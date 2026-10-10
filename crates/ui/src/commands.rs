//! Command dispatch: engine command lines, interactive tools, typed input,
//! Enter / Esc, tool options and command-name completion.

use crate::tools::{parse_typed_point, OptAction, Step, Tool, ToolKind, Want};
use crate::viewport::to_d;
use crate::{gumball, pick, Act, FormaApp, Guarded, LogKind, SidePanel};
use eframe::egui;
use forma_geom::{Point3, Vec3};
use forma_render::{DisplayMode, StandardView};

/// Words handled by the UI itself (views, display modes, toggles).
const UI_WORDS: [&str; 30] = [
    "pointson",
    "pointsoff",
    "zw",
    "zoomwindow",
    "bottom",
    "back",
    "left",
    "ze",
    "zea",
    "zoomextents",
    "zs",
    "zoomselected",
    "top",
    "front",
    "right",
    "perspective",
    "persp",
    "help",
    "?",
    "ortho",
    "snap",
    "gridsnap",
    "planar",
    "osnap",
    "gumball",
    "wireframe",
    "setdisplaymode-wireframe",
    "shaded",
    "maxviewport",
    "maximize",
];

/// Typed words that never complete to another command (Enter must not turn
/// `u` into `Ungroup`).
const NEVER_COMPLETE: [&str; 3] = ["u", "undo", "redo"];

impl FormaApp {
    /// Run an engine command line and log the result.
    pub(crate) fn run_engine(&mut self, line: &str) {
        self.run_engine_as(line, true);
    }

    /// `remember`: the line becomes the command to repeat and to recall (not for
    /// lines emitted by interactive tools, which repeat the tool instead).
    fn run_engine_as(&mut self, line: &str, remember: bool) {
        let line = line.trim();
        if line.is_empty() {
            return;
        }
        let first = line.split_whitespace().next().unwrap_or("").to_lowercase();
        if matches!(first.as_str(), "new" | "open") && self.guard(Guarded::Line(line.to_string())) {
            return;
        }
        self.log(LogKind::Command, format!("Command: {line}"));
        match self.engine.run_line(line) {
            Ok(msg) => {
                self.log(LogKind::Normal, msg);
                let saved = matches!(first.as_str(), "save" | "saveas");
                if first == "open" || first == "new" {
                    self.reset_document_view();
                    self.saved_by_forma = false;
                }
                if saved {
                    self.saved_by_forma = true;
                }
                if saved || first == "open" || first == "new" {
                    self.saved_at = std::time::Instant::now();
                    self.saved_version = self.engine.doc().version();
                    if let Some(p) = self.engine.doc().path.clone().filter(|_| first != "new") {
                        self.remember_file(&p);
                    }
                }
                if remember
                    && !matches!(
                        first.as_str(),
                        "undo" | "redo" | "u" | "open" | "save" | "saveas"
                    )
                {
                    self.set_last(line);
                }
            }
            Err(e) => {
                self.log(LogKind::Error, e.to_string());
                if first == "open" {
                    // A recent file that is gone leaves the list.
                    let path = line.split_once(' ').map_or("", |(_, p)| p.trim());
                    if !path.is_empty() && !std::path::Path::new(path).exists() {
                        self.settings.remove_recent(path);
                    }
                }
            }
        }
    }

    /// Remember a command for Enter / right-click repeat and Up-arrow recall.
    pub(crate) fn set_last(&mut self, cmd: &str) {
        self.last_command = Some(cmd.to_string());
        self.history.push(cmd);
    }

    pub(crate) fn start_tool(&mut self, kind: ToolKind) {
        self.tool = None;
        self.option_edit = None;
        self.gumball_typed = None;
        let plane = self.viewports[self.active].cplane();
        if kind.instant() && self.has_selection() {
            let line = Tool::new(kind, plane, true, Point3::ORIGIN, Vec::new()).instant_line();
            self.set_last(kind.name());
            self.run_engine_from_tool(&line);
            return;
        }
        let doc = self.engine.doc();
        let sel = &self.engine.ctx.selection;
        let anchor = pick::selection_box(doc, sel).map_or(Point3::ORIGIN, |b| b.center());
        let skeleton = if kind.needs_selection() {
            pick::selection_skeleton(doc, sel)
        } else {
            Vec::new()
        };
        let mut tool = Tool::new(kind, plane, self.has_selection(), anchor, skeleton);
        if kind == ToolKind::Extrude {
            tool.anchor = self.extrude_anchor().unwrap_or(anchor);
        }
        tool.distance = match kind {
            ToolKind::Offset => self.offset_distance,
            ToolKind::Fillet => self.fillet_radius,
            ToolKind::Chamfer => self.chamfer_distance,
            _ => tool.distance,
        };
        tool.curves = self.selected_curves();
        self.log(LogKind::Command, format!("Command: {}", kind.name()));
        self.set_last(kind.name());
        // A tool that starts by asking for one object takes the preselected
        // one (FilletEdge, Shell… on the selected solid), like Rhino.
        let first_is_object = !tool.selecting
            && matches!(tool.seq_step(), Some((_, crate::tools::seq::In::Object(_))));
        let preselected = (self.engine.ctx.selection.len() == 1)
            .then(|| self.engine.ctx.selection.iter().next().map(|i| i.0))
            .flatten();
        self.tool = Some(tool);
        if let (true, Some(id)) = (first_is_object, preselected) {
            if let Some(t) = self.tool.as_mut() {
                let step = t.feed_object(id);
                self.handle_step(step);
            }
        }
    }

    /// Selected curves with their own plane normals (Offset preview).
    fn selected_curves(&self) -> Vec<(forma_geom::Chain, Option<Vec3>)> {
        let doc = self.engine.doc();
        self.engine
            .ctx
            .selection
            .iter()
            .filter_map(|id| doc.object(*id))
            .filter_map(|o| {
                o.geometry
                    .to_chain()
                    .map(|c| (c, o.geometry.curve_normal()))
            })
            .collect()
    }

    /// First point of the first selected curve (Extrude height line).
    fn extrude_anchor(&self) -> Option<Point3> {
        let doc = self.engine.doc();
        self.engine
            .ctx
            .selection
            .iter()
            .filter_map(|id| doc.object(*id))
            .find_map(|o| o.geometry.curve_points().first().copied())
    }

    fn refresh_tool_selection(&mut self) {
        let doc = self.engine.doc();
        let sel = &self.engine.ctx.selection;
        let skeleton = pick::selection_skeleton(doc, sel);
        let anchor = pick::selection_box(doc, sel).map(|b| b.center());
        let ext = self.extrude_anchor();
        let curves = self.selected_curves();
        if let Some(t) = self.tool.as_mut() {
            t.skeleton = skeleton;
            t.curves = curves;
            if let Some(a) = anchor {
                t.anchor = a;
            }
            if matches!(t.kind, ToolKind::Extrude) {
                if let Some(a) = ext {
                    t.anchor = a;
                }
            }
        }
    }

    pub(crate) fn handle_step(&mut self, step: Step) {
        if matches!(step, Step::Done(_) | Step::Cancel(_)) {
            self.track_points.clear();
            self.option_edit = None;
        }
        if let Some(t) = &self.tool {
            match t.kind {
                ToolKind::Offset => self.offset_distance = t.distance,
                ToolKind::Fillet => self.fillet_radius = t.distance,
                ToolKind::Chamfer => self.chamfer_distance = t.distance,
                _ => {}
            }
        }
        match step {
            Step::Continue => {}
            Step::Done(cmds) => {
                self.tool = None;
                for c in cmds {
                    self.run_engine_from_tool(&c);
                }
            }
            Step::Emit(cmds) => {
                for c in cmds {
                    self.run_engine_from_tool(&c);
                }
            }
            Step::Cancel(msg) => {
                self.tool = None;
                self.log(LogKind::Normal, msg);
            }
        }
    }

    /// Repeating should restart the tool, not replay its coordinates.
    fn run_engine_from_tool(&mut self, line: &str) {
        self.run_engine_as(line, false);
    }

    /// Enter, Space or right click.
    pub(crate) fn enter_action(&mut self) {
        if self.option_edit.take().is_some() {
            return; // keep the current value
        }
        let has_sel = self.has_selection();
        // Split: first the objects to split, then the cutting objects.
        if let Some(t) = self.tool.as_mut().filter(|t| t.kind == ToolKind::Split) {
            if t.phase == 0 {
                if !has_sel {
                    self.tool = None;
                    self.log(LogKind::Normal, "nothing selected");
                    return;
                }
                t.stash = self.engine.ctx.selection.iter().map(|i| i.0).collect();
                t.phase = 1;
                self.engine.ctx.selection.clear();
                return;
            }
            let ids: Vec<String> = t.stash.iter().map(|i| format!("#{i}")).collect();
            let cutters: Vec<String> = self
                .engine
                .ctx
                .selection
                .iter()
                .map(|i| format!("#{}", i.0))
                .collect();
            let cmds = vec![
                "SelNone".to_string(),
                format!("Select {}", ids.join(" ")),
                format!("Split {}", cutters.join(" ")).trim().to_string(),
            ];
            self.handle_step(Step::Done(cmds));
            return;
        }
        if let Some(t) = self.tool.as_mut() {
            let was_selecting = t.selecting;
            let step = t.enter(has_sel);
            if was_selecting && matches!(step, Step::Continue) {
                self.refresh_tool_selection();
            }
            self.handle_step(step);
            return;
        }
        if let Some(last) = self.last_command.clone() {
            self.submit(&last);
        }
    }

    /// Esc, Rhino style: close a menu, leave an option, cancel the running
    /// command (and its typed text), clear typed text, drop the picked face,
    /// and finally deselect.
    pub(crate) fn cancel(&mut self) {
        self.ac_choice = None;
        self.history.reset();
        if self.title_menu.take().is_some() {
            return;
        }
        if self.option_edit.take().is_some() {
            self.command.clear();
            return;
        }
        let busy = self.gumball_typed.take().is_some()
            | self.gumball_drag.take().is_some()
            | self.pending.take().is_some()
            | self.face_drag.take().is_some()
            | std::mem::take(&mut self.face_typed)
            | self.tool.take().is_some();
        if busy {
            self.command.clear();
            self.track_points.clear();
            self.log(LogKind::Normal, "cancelled");
        } else if !self.command.is_empty() {
            self.command.clear();
        } else if self.face_sel.take().is_some() {
        } else if !self.grips.sel.is_empty() {
            self.grips.sel.clear();
            self.dirty_all();
        } else if self.grips.active() {
            self.points_off();
        } else {
            self.engine.ctx.selection.clear();
        }
    }

    /// Nothing is being asked: the command line starts a new command.
    pub(crate) fn at_command_prompt(&self) -> bool {
        self.tool.is_none()
            && self.pending.is_none()
            && self.gumball_typed.is_none()
            && !self.face_typed
            && self.option_edit.is_none()
    }

    /// Is `word` a command or alias as typed (no completion needed)?
    fn is_known(&self, word: &str) -> bool {
        let w = word.to_lowercase();
        UI_WORDS.contains(&w.as_str())
            || NEVER_COMPLETE.contains(&w.as_str())
            || self.engine.resolve(&w).is_some()
            || ToolKind::from_name(&w).is_some()
            || ToolKind::selection_command(&w).is_some()
            || self.completer.exact(&w).is_some()
    }

    /// A partly typed command name completed on Enter / Space (`Polyl` →
    /// `Polyline`); anything else comes back unchanged.
    fn complete_typed(&self, text: &str) -> String {
        let t = text.trim();
        if !self.at_command_prompt() || t.chars().count() < 2 {
            return text.to_string();
        }
        self.completer
            .resolve(t, |w| self.is_known(w))
            .unwrap_or_else(|| text.to_string())
    }

    /// Text ended by a space (Rhino: space works like Enter, except that a
    /// command that needs arguments keeps collecting them until Enter).
    pub(crate) fn submit_space(&mut self, text: &str) {
        let text = self.complete_typed(text);
        let toks: Vec<&str> = text.split_whitespace().collect();
        if toks.is_empty() {
            self.submit(&text);
            return;
        }
        if let Some((line, _)) = self.pending.as_mut() {
            line.push(' ');
            line.push_str(&toks.join(" "));
            return;
        }
        if self.at_command_prompt() {
            let first = toks[0];
            let is_tool = ToolKind::from_name(first).is_some()
                || ToolKind::selection_command(first).is_some();
            if !is_tool {
                // Arguments to come (required or optional): collect them until Enter.
                let what = self
                    .engine
                    .missing_input(first)
                    .or_else(|| self.engine.optional_args(first));
                if let Some(what) = what {
                    self.pending = Some((toks.join(" "), what));
                    return;
                }
            }
        }
        self.submit(&text);
    }

    /// Text from the command line (one or more tokens), on Enter.
    pub(crate) fn submit(&mut self, text: &str) {
        self.ac_choice = None;
        self.history.reset();
        let text = self.complete_typed(text);
        let text = text.as_str();
        let toks: Vec<String> = text.split_whitespace().map(String::from).collect();
        // A tool option waiting for its new value.
        if let Some(name) = self.option_edit.take() {
            if let Some(tok) = toks.first() {
                self.set_tool_option(name, tok);
            }
            return;
        }
        // A command waiting for arguments: collect them until Enter.
        if let Some((mut line, _)) = self.pending.take() {
            if !toks.is_empty() {
                line.push(' ');
                line.push_str(&toks.join(" "));
            }
            self.run_engine(&line);
            return;
        }
        if toks.is_empty() {
            if self.gumball_typed.take().is_some() {
                return;
            }
            self.enter_action();
            return;
        }
        if std::mem::take(&mut self.face_typed) {
            match (toks[0].parse::<f64>(), self.face_sel.as_ref()) {
                (Ok(v), Some(f)) => {
                    let line = crate::input::face_command(f, v);
                    self.run_engine(&line);
                    self.refresh_face_sel();
                }
                _ => self.log(LogKind::Error, format!("a number is expected: {}", toks[0])),
            }
            return;
        }
        if let Some((h, center)) = self.gumball_typed.take() {
            match toks[0].parse::<f64>() {
                Ok(v) => {
                    for line in self.gumball_lines(gumball::typed_motion(h, v), center) {
                        self.run_engine(&line);
                    }
                }
                Err(_) => self.log(LogKind::Error, format!("a number is expected: {}", toks[0])),
            }
            return;
        }
        if let Some(t) = self
            .tool
            .as_mut()
            .filter(|t| t.want() == crate::tools::Want::Text)
        {
            let step = t.feed_text(text.trim());
            self.handle_step(step);
            return;
        }
        if self.tool.is_some() {
            for t in &toks {
                self.feed_token(t);
                if self.tool.is_none() {
                    break;
                }
            }
            return;
        }
        let first = toks[0].to_lowercase();
        if toks.len() == 1 && !self.has_selection() {
            if let Some(kind) = ToolKind::selection_command(&first) {
                self.start_tool(kind);
                return;
            }
        }
        if toks.len() == 1 && first == "projecttocplane" {
            self.start_tool(ToolKind::OnSel("ProjectToCPlane"));
            return;
        }
        if let Some(kind) = ToolKind::from_name(&first) {
            // A sequence command typed with its arguments runs as written.
            if toks.len() > 1 && matches!(kind, ToolKind::Seq(_)) {
                self.run_engine(text);
                return;
            }
            self.start_tool(kind);
            for t in &toks[1..] {
                self.feed_token(t);
                if self.tool.is_none() {
                    break;
                }
            }
            return;
        }
        match first.as_str() {
            "ze" | "zoomextents" | "zea" => {
                let which = (first == "ze").then_some(self.active);
                self.fit(which);
            }
            "top" => self.set_active_view(StandardView::Top),
            "front" => self.set_active_view(StandardView::Front),
            "right" => self.set_active_view(StandardView::Right),
            "bottom" => self.set_active_view(StandardView::Bottom),
            "back" => self.set_active_view(StandardView::Back),
            "left" => self.set_active_view(StandardView::Left),
            "perspective" | "persp" => self.set_active_view(StandardView::Perspective),
            "help" | "?" => {
                self.show_help();
                return;
            }
            "ortho" => self.snap.ortho = !self.snap.ortho,
            "snap" | "gridsnap" => self.snap.grid = !self.snap.grid,
            "planar" => self.snap.planar = !self.snap.planar,
            "osnap" => self.snap.disabled = !self.snap.disabled,
            "gumball" => self.gumball_on = !self.gumball_on,
            "zs" | "zoomselected" => self.zoom_selected(),
            "pointson" => self.points_on(),
            "pointsoff" => self.points_off(),
            "zw" | "zoomwindow" => {
                self.zoom_window = true;
                self.log(LogKind::Normal, "Drag a window to zoom into");
            }
            "wireframe" | "setdisplaymode-wireframe" => self.set_mode(DisplayMode::Wireframe),
            "shaded" => self.set_mode(DisplayMode::Shaded),
            "maxviewport" | "maximize" => self.toggle_maximize(),
            _ => {
                // A bare engine command that needs arguments waits for them.
                if toks.len() == 1 {
                    if let Some(what) = self.engine.missing_input(text.trim()) {
                        self.pending = Some((text.trim().to_string(), what));
                        return;
                    }
                }
                self.run_engine(text);
                return;
            }
        }
        self.history.push(text);
        self.log(LogKind::Command, format!("Command: {text}"));
    }

    /// Set a numeric tool option from typed text.
    fn set_tool_option(&mut self, name: &str, tok: &str) {
        let Some(t) = self.tool.as_mut() else { return };
        match tok.parse::<f64>() {
            Ok(x) => {
                if let Err(e) = t.set_option(name, x) {
                    self.log(LogKind::Error, e);
                }
            }
            Err(_) => self.log(LogKind::Error, format!("a number is expected: {tok}")),
        }
    }

    /// A clicked prompt option.
    pub(crate) fn click_option(&mut self, a: OptAction) {
        match a {
            OptAction::Word(w) => self.feed_token(w),
            OptAction::Value(n) => {
                self.option_edit = Some(n);
                self.command.clear();
            }
        }
        self.focus_command = true;
    }

    pub(crate) fn feed_token(&mut self, tok: &str) {
        let Some(t) = self.tool.as_mut() else { return };
        if let Some(step) = t.option(tok) {
            self.handle_step(step);
            return;
        }
        // Numeric options: `Distance=12` in one go, or `Distance` then a number.
        if let Some((name, value)) = tok.split_once('=') {
            if let Some(opt) = t.options().into_iter().find_map(|o| match o.action {
                OptAction::Value(n) if n.eq_ignore_ascii_case(name) => Some(n),
                _ => None,
            }) {
                self.set_tool_option(opt, value);
                return;
            }
        }
        if let Some(opt) = t.options().into_iter().find_map(|o| match o.action {
            OptAction::Value(n) if n.eq_ignore_ascii_case(tok) => Some(n),
            _ => None,
        }) {
            self.option_edit = Some(opt);
            return;
        }
        let want = t.want();
        let toward = self.hover.as_ref().map(|h| h.point);
        let step = match want {
            Want::Selection => {
                self.log(
                    LogKind::Normal,
                    "select objects in a view, then press Enter",
                );
                return;
            }
            Want::Height { .. } => match tok.parse::<f64>() {
                Ok(x) => t.feed_number(x),
                Err(_) => {
                    self.log(LogKind::Error, format!("a number is expected: {tok}"));
                    return;
                }
            },
            Want::Number | Want::Pick => match tok.parse::<f64>() {
                Ok(x) => t.feed_number(x),
                Err(_) => {
                    let msg = if want == Want::Pick {
                        "click on a curve in a view (or type a number to change the value)"
                    } else {
                        "a number is expected"
                    };
                    self.log(LogKind::Error, format!("{msg}: {tok}"));
                    return;
                }
            },
            Want::Text => t.feed_text(tok),
            Want::Choice => {
                let words: Vec<String> = t.options().into_iter().map(|o| o.label).collect();
                self.log(
                    LogKind::Error,
                    format!("choose one of: {} — got {tok}", words.join(", ")),
                );
                return;
            }
            Want::PickObject => match tok.trim_start_matches('#').parse::<u64>() {
                Ok(id) => t.feed_object(id),
                Err(_) => {
                    self.log(LogKind::Error, "click on an object in a view (or type #id)");
                    return;
                }
            },
            Want::PointOrNumber if tok.parse::<f64>().is_ok() => {
                t.feed_number(tok.parse().expect("checked"))
            }
            Want::Point | Want::PointOrNumber | Want::SurfacePoint => {
                let plane = t.plane;
                match parse_typed_point(tok, &plane, t.base(), toward) {
                    Some(p) => t.feed_point(p),
                    None => {
                        self.log(LogKind::Error, format!("cannot read a point from: {tok}"));
                        return;
                    }
                }
            }
        };
        self.handle_step(step);
    }

    // ----------------------------------------------------------------- views

    pub(crate) fn set_mode(&mut self, m: DisplayMode) {
        let vp = &mut self.viewports[self.active];
        vp.mode = m;
        vp.dirty = true;
    }

    /// Fit the selection in the active view (Rhino's ZS).
    fn zoom_selected(&mut self) {
        let Some(bb) = pick::selection_box(self.engine.doc(), &self.engine.ctx.selection) else {
            self.log(LogKind::Normal, "nothing selected");
            return;
        };
        let o = self.origin();
        let vp = &mut self.viewports[self.active];
        let aspect = if vp.rect.is_positive() {
            vp.aspect()
        } else {
            1.5
        };
        vp.camera.fit(to_d(bb.min) - o, to_d(bb.max) - o, aspect);
        vp.dirty = true;
    }

    fn set_active_view(&mut self, v: StandardView) {
        let vp = &mut self.viewports[self.active];
        let target = vp.camera.target;
        vp.camera = forma_render::Camera::view(v);
        vp.camera.target = target;
        vp.kind = v;
        self.fit(Some(self.active));
    }

    pub(crate) fn toggle_maximize(&mut self) {
        self.maximized = if self.maximized.is_some() {
            None
        } else {
            Some(self.active)
        };
        self.dirty_all();
    }

    fn show_help(&mut self) {
        self.log(LogKind::Command, "Command: Help");
        let mut lines: Vec<String> = Vec::new();
        lines.push(
            "Drawing: click points in a view, or type x,y · @dx,dy · @dist<angle · a length:"
                .into(),
        );
        for k in ToolKind::CURVES
            .iter()
            .chain(&ToolKind::SOLIDS)
            .chain(&ToolKind::TRANSFORMS)
        {
            lines.push(format!("  {}", k.tooltip()));
        }
        lines.push("Curve tools / edit:".into());
        for k in ToolKind::CURVE_TOOLS.iter().chain(&ToolKind::EDIT) {
            lines.push(format!("  {}", k.tooltip()));
        }
        let more: Vec<&str> = crate::tools::seq::ALL.iter().map(|s| s.name).collect();
        lines.push(format!(
            "More tools (menus Curve, Surface, Solid, Mesh, Dimension, Transform, Analyze): {}",
            more.join(" · ")
        ));
        lines.push("Attributes: SetObjectColor <r,g,b|#hex|rosso…|ByLayer> · LayerColor <colour> [layer] · LayerVisible on|off [layer] · LayerLock on|off [layer] · Layer <name> · ChangeLayer <name>".into());
        lines.push("Other: Array nx ny nz dx,dy,dz · Delete · SelAll · SelNone · Undo · Redo · Save · Open · New [mm|cm|m] · ZE · ZEA".into());
        lines.push("Command line: while typing, a list of matching commands appears (↑/↓ choose, Enter/Tab/Space accept) · ↑/↓ on an empty line recalls recent commands · options in ( ) can be clicked".into());
        lines.push("Gumball: drag an arrow to move along an axis, a square to move in a plane, an arc to rotate; click a handle to type an exact value".into());
        lines.push("Keys: Enter/Space/right click = confirm or repeat · Esc = cancel · Del = delete · F3 = properties · F8 = Ortho (or hold Shift) · F9 = grid snap · F7 = grid · Ctrl+Z/Y/A/S/O/N".into());
        lines.push("Mouse: left = pick/select (drag: window →, crossing ←) · right drag = rotate (pan in Top/Front/Right) · Shift+right / middle = pan · wheel = zoom · double-click a view title = maximize".into());
        for l in lines {
            self.log(LogKind::Normal, l);
        }
    }

    // ----------------------------------------------------------------- actions

    pub(crate) fn act(&mut self, ctx: &egui::Context, a: Act) {
        match a {
            Act::Tool(k) => self.start_tool(k),
            Act::Cmd(c) => self.run_engine(c),
            Act::Submit(c) => self.submit(c),
            Act::Prefill(text, usage) => {
                self.tool = None;
                self.command = text.to_string();
                self.focus_command = true;
                self.log(LogKind::Normal, usage);
            }
            Act::Open => self.open_dialog(),
            Act::Save => {
                self.save(false);
            }
            Act::SaveAs => {
                self.save(true);
            }
            Act::Exit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            Act::Help => {
                self.side_panel = SidePanel::Help;
                self.show_help();
            }
            Act::Maximize => self.toggle_maximize(),
            Act::Panel(p) => self.side_panel = p,
            Act::Mode(m) => self.set_mode(m),
            Act::Clip(c) => match c {
                "Paste" => self.run_engine("Paste"),
                other => {
                    if self.has_selection() {
                        self.run_engine(other);
                    } else {
                        self.start_tool(ToolKind::OnSel(other));
                    }
                }
            },
        }
    }
}
