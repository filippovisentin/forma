//! MCP server: lets AI agents (Claude Desktop, Claude Code…) build and edit Forma
//! models. JSON-RPC 2.0 over stdio, one message per line (ADR 0007).
//!
//! Tools: `forma_run` (command lines), `forma_inspect` (model summary),
//! `forma_commands` (command list and syntax), `forma_render` (a view as PNG).
//!
//! Two back ends:
//! - local: an engine of its own, on files (`forma-cli mcp [--open f.3dm]`);
//! - live: the model open in the desktop app, over the app's agent connection
//!   (`forma-cli mcp --live`, after Tools ▸ Agent Connection in the app).
//!
//! `--root <folder>` keeps every file path (Open, Save, Import, Export) inside one
//! folder; relative paths are taken from it.

use forma_engine::agent;
use forma_engine::Engine;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

/// Protocol version answered when the client does not ask for one.
const PROTOCOL: &str = "2025-06-18";

#[derive(Debug, Default, Clone)]
pub struct Options {
    /// Folder that file paths must stay in.
    pub root: Option<PathBuf>,
    /// Model to open at start (local back end).
    pub open: Option<String>,
    /// Drive the running desktop app instead of an engine of our own.
    pub live: bool,
}

/// Where commands go.
enum Backend {
    Local(Box<Engine>),
    Live,
}

pub struct Server {
    backend: Backend,
    root: Option<PathBuf>,
}

impl Server {
    pub fn new(opts: &Options) -> Result<Server, String> {
        let root = match &opts.root {
            Some(r) => {
                Some(std::fs::canonicalize(r).map_err(|e| format!("--root {}: {e}", r.display()))?)
            }
            None => None,
        };
        let backend = if opts.live {
            Backend::Live
        } else {
            let mut engine = Engine::new();
            // Centimetres, like the app (Engine::new is mm for tests).
            let _ = engine.run_line("New cm");
            Backend::Local(Box::new(engine))
        };
        let mut s = Server { backend, root };
        if let Some(f) = &opts.open {
            let (ok, out) = s.run(&format!("Open {f}"));
            if !ok {
                return Err(out);
            }
        }
        Ok(s)
    }

    /// Serve until stdin closes.
    pub fn serve(&mut self, input: impl BufRead, mut output: impl Write) -> std::io::Result<()> {
        for line in input.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            if let Some(reply) = self.handle_message(&line) {
                writeln!(output, "{reply}")?;
                output.flush()?;
            }
        }
        Ok(())
    }

    /// One JSON-RPC message in, the reply out (`None` for notifications).
    pub fn handle_message(&mut self, line: &str) -> Option<Value> {
        let msg: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(e) => return Some(rpc_error(Value::Null, -32700, &format!("parse error: {e}"))),
        };
        let id = msg.get("id").cloned();
        let method = msg.get("method").and_then(Value::as_str).unwrap_or("");
        let params = msg.get("params").cloned().unwrap_or(Value::Null);
        let id = id?; // notifications get no reply
        let result = match method {
            "initialize" => Ok(self.initialize(&params)),
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({ "tools": tools() })),
            "tools/call" => self.call(&params),
            _ => Err((-32601, format!("unknown method {method}"))),
        };
        Some(match result {
            Ok(r) => json!({ "jsonrpc": "2.0", "id": id, "result": r }),
            Err((code, m)) => rpc_error(id, code, &m),
        })
    }

    fn initialize(&self, params: &Value) -> Value {
        let version = params
            .get("protocolVersion")
            .and_then(Value::as_str)
            .unwrap_or(PROTOCOL);
        let mode = match self.backend {
            Backend::Local(_) => "Commands act on a model of this server (open or save files with Open / Save).",
            Backend::Live => "Commands act on the model open in the Forma desktop app; the user sees every change and can undo it.",
        };
        json!({
            "protocolVersion": version,
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "forma", "version": env!("CARGO_PKG_VERSION") },
            "instructions": format!(
                "Forma is a Rhino-style NURBS modeller. {mode} Use forma_inspect to see the model, \
                 forma_commands to look up syntax, forma_run to run command lines and forma_render \
                 to look at the result. Coordinates are in the model units shown by forma_inspect."
            )
        })
    }

    fn call(&mut self, params: &Value) -> Result<Value, (i64, String)> {
        let name = params
            .get("name")
            .and_then(Value::as_str)
            .ok_or((-32602, "missing tool name".to_string()))?;
        let args = params.get("arguments").cloned().unwrap_or(json!({}));
        let text = |ok: bool, t: String| json!({ "content": [{ "type": "text", "text": t }], "isError": !ok });
        Ok(match name {
            "forma_run" => {
                let script = args
                    .get("commands")
                    .and_then(Value::as_str)
                    .ok_or((-32602, "forma_run needs `commands`".to_string()))?;
                let (ok, out) = self.run(script);
                text(ok, out)
            }
            "forma_inspect" => {
                let limit = args
                    .get("limit")
                    .and_then(Value::as_u64)
                    .map_or(200, |l| l.clamp(1, 5000) as usize);
                match self.inspect(limit) {
                    Ok(s) => text(true, s),
                    Err(e) => text(false, e),
                }
            }
            "forma_commands" => {
                let filter = args
                    .get("filter")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_lowercase();
                text(true, command_list(&filter))
            }
            "forma_python" => {
                let code = args
                    .get("code")
                    .and_then(Value::as_str)
                    .ok_or((-32602, "forma_python needs `code`".to_string()))?;
                let (ok, out) = self.python(code);
                text(ok, out)
            }
            "forma_render" => match self.render(&args) {
                Ok(png) => json!({
                    "content": [{ "type": "image", "data": base64(&png), "mimeType": "image/png" }],
                    "isError": false
                }),
                Err(e) => text(false, e),
            },
            _ => return Err((-32602, format!("unknown tool {name}"))),
        })
    }

    /// Run command lines (checked against `--root` first).
    pub fn run(&mut self, script: &str) -> (bool, String) {
        let mut lines = Vec::new();
        for line in script.split(['\n', ';']) {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            match self.check_paths(line) {
                Ok(l) => lines.push(l),
                Err(e) => return (false, format!("> {line}\nerror: {e}\n(nothing was run)")),
            }
        }
        let script = lines.join("\n");
        match &mut self.backend {
            Backend::Local(engine) => agent::run_script(engine, &script),
            Backend::Live => live(agent::RUN, &script).unwrap_or_else(|e| (false, e)),
        }
    }

    /// Run Python (`import forma`) on the model; what it printed, or the traceback.
    pub fn python(&mut self, code: &str) -> (bool, String) {
        if self.root.is_some() {
            // A script could open any file: the folder limit cannot hold.
            return (
                false,
                "Python is not available with --root (scripts can reach any file)".into(),
            );
        }
        match &mut self.backend {
            Backend::Local(engine) => {
                let mut host = forma_script::EngineHost::new(engine);
                let r = host.run_script(code, "<agent>");
                let mut out = std::mem::take(&mut host.out);
                match r {
                    Ok(()) => (true, out),
                    Err(e) => {
                        out.push_str(&e);
                        (false, out)
                    }
                }
            }
            Backend::Live => live(agent::PYTHON, code).unwrap_or_else(|e| (false, e)),
        }
    }

    fn inspect(&mut self, limit: usize) -> Result<String, String> {
        match &self.backend {
            Backend::Local(engine) => Ok(agent::inspect(engine, limit)),
            Backend::Live => match live(agent::INSPECT, &limit.to_string()) {
                Ok((true, s)) => Ok(s),
                Ok((false, s)) | Err(s) => Err(s),
            },
        }
    }

    fn render(&mut self, args: &Value) -> Result<Vec<u8>, String> {
        use forma_render::StandardView;
        let view = match args
            .get("view")
            .and_then(Value::as_str)
            .unwrap_or("perspective")
        {
            "perspective" => StandardView::Perspective,
            "top" => StandardView::Top,
            "front" => StandardView::Front,
            "right" => StandardView::Right,
            v => return Err(format!("unknown view {v} (perspective, top, front, right)")),
        };
        let dim = |k: &str, d: u64| {
            args.get(k)
                .and_then(Value::as_u64)
                .unwrap_or(d)
                .clamp(64, 2400) as u32
        };
        let size = (dim("width", 1200), dim("height", 800));
        let pixels = match &self.backend {
            Backend::Local(engine) => forma_render::screenshot(engine.doc(), view, size),
            Backend::Live => {
                // The app writes a copy of its model; render that.
                let (ok, path) = live(agent::SNAPSHOT, "")?;
                if !ok {
                    return Err(path);
                }
                let mut e = Engine::new();
                e.run_line(&format!("Open {path}"))
                    .map_err(|e| e.to_string())?;
                let _ = std::fs::remove_file(&path);
                forma_render::screenshot(e.doc(), view, size)
            }
        }
        .ok_or("no GPU or software renderer available for offscreen rendering")?;
        encode_png(&pixels, size)
    }

    /// Lines that read or write files must name a path inside `--root`; relative
    /// paths are made absolute inside it. Other lines pass unchanged.
    fn check_paths(&self, line: &str) -> Result<String, String> {
        let mut words = line.splitn(2, char::is_whitespace);
        let cmd = words.next().unwrap_or("");
        let rest = words.next().unwrap_or("").trim().trim_matches('"');
        let takes_path = matches!(
            cmd.to_ascii_lowercase().as_str(),
            "open" | "save" | "saveas" | "import" | "export" | "exportselected"
        );
        let Some(root) = &self.root else {
            return Ok(line.to_string());
        };
        if !takes_path {
            return Ok(line.to_string());
        }
        if rest.is_empty() {
            return Ok(line.to_string()); // Save to the current file (already inside)
        }
        let p = Path::new(rest);
        let full = if p.is_absolute() {
            p.to_path_buf()
        } else {
            root.join(p)
        };
        let parent = full.parent().ok_or("invalid path")?;
        let parent =
            std::fs::canonicalize(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        if !parent.starts_with(root) {
            return Err(format!(
                "{rest} is outside the allowed folder {}",
                root.display()
            ));
        }
        let name = full.file_name().ok_or("invalid path")?;
        Ok(format!("{cmd} {}", parent.join(name).display()))
    }
}

fn rpc_error(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

fn tools() -> Value {
    json!([
        {
            "name": "forma_run",
            "description": "Run Forma command lines, Rhino-style: one command per line (or separated by ';'). \
    Points are x,y,z (z may be omitted), @dx,dy,dz is relative to the last point, @d<angle polar; objects are \
    referred to as #id (see forma_inspect). Most edit commands act on the selection: SelNone, Select #3 #4, \
    SelAll, SelLast (objects made by the previous command). Examples: 'Box 0,0,0 100,50,0 30', \
    'Circle 0,0 25', 'SelLast', 'Extrude 40', 'Move 0,0 0,100', 'Undo', 'Layer Walls', 'Save C:/models/a.3dm'. \
    Stops at the first error and reports which line failed. Use forma_commands for the syntax of a command. \
    In the live desktop app, ZEA zooms every view to the model, ZS to the selection, and Top / Front / Right / \
    Perspective / Shaded / Wireframe set the active view.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "commands": { "type": "string", "description": "Command lines to run, in order." }
                },
                "required": ["commands"]
            }
        },
        {
            "name": "forma_inspect",
            "description": "Describe the model: file, units, layers, objects (id, kind, layer, name, bounding box) and the selection.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "limit": { "type": "integer", "description": "Most objects to list (default 200)." }
                }
            }
        },
        {
            "name": "forma_commands",
            "description": "List Forma commands with their syntax. Pass a filter (part of a name or description) to narrow it, e.g. 'extrude', 'mesh', 'fillet'.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "filter": { "type": "string", "description": "Text to look for in names and descriptions." }
                }
            }
        },
        {
            "name": "forma_python",
            "description": "Run a Python script on the model, for loops and arithmetic that plain command lines cannot do. \
    `import forma`, then forma.run(\"<command line>\") runs a command (raises RuntimeError on failure), \
    forma.objects() lists objects as dicts (id, kind, layer, name, min, max), forma.selection(), \
    forma.select(ids), forma.units(); print() output is returned. The whole script is one undo step. \
    Example: for i in range(5): forma.run(f\"Box {i*120},0,0 {i*120+100},20,0 270\")",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "code": { "type": "string", "description": "Python source." }
                },
                "required": ["code"]
            }
        },
        {
            "name": "forma_render",
            "description": "Render the model to a PNG image (shaded, fitted to the whole model).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "view": { "type": "string", "enum": ["perspective", "top", "front", "right"] },
                    "width": { "type": "integer" },
                    "height": { "type": "integer" }
                }
            }
        }
    ])
}

fn command_list(filter: &str) -> String {
    let engine = Engine::new();
    let mut out = String::new();
    for (name, help) in engine.command_list() {
        if filter.is_empty()
            || name.to_lowercase().contains(filter)
            || help.to_lowercase().contains(filter)
        {
            out.push_str(help);
            out.push('\n');
        }
    }
    if out.is_empty() {
        out = format!("no command matches '{filter}'");
    }
    out
}

/// One request to the desktop app's agent connection.
fn live(verb: &str, text: &str) -> Result<(bool, String), String> {
    let file = agent::link_file().ok_or("no settings folder")?;
    let info = std::fs::read_to_string(&file).map_err(|_| {
        "the Forma app is not accepting agent connections: in the app choose Tools ▸ Agent Connection".to_string()
    })?;
    let mut lines = info.lines();
    let port: u16 = lines
        .next()
        .and_then(|p| p.trim().parse().ok())
        .ok_or("damaged agent.txt")?;
    let token = lines.next().ok_or("damaged agent.txt")?.trim().to_string();
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    let stream = std::net::TcpStream::connect_timeout(&addr, std::time::Duration::from_secs(3))
        .map_err(|e| format!("cannot reach the Forma app ({e}); is it still running?"))?;
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(120)))
        .ok();
    let mut writer = stream.try_clone().map_err(|e| e.to_string())?;
    writeln!(writer, "{}", agent::request_line(&token, verb, text)).map_err(|e| e.to_string())?;
    let mut reply = String::new();
    BufReader::new(stream)
        .read_line(&mut reply)
        .map_err(|e| format!("no answer from the Forma app: {e}"))?;
    agent::parse_reply(&reply).ok_or_else(|| "unreadable answer from the Forma app".to_string())
}

fn encode_png(rgba: &[u8], (w, h): (u32, u32)) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    let mut enc = png::Encoder::new(&mut out, w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header()
        .and_then(|mut wr| wr.write_image_data(rgba))
        .map_err(|e| format!("png: {e}"))?;
    Ok(out)
}

fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let n = (u32::from(c[0]) << 16)
            | (u32::from(*c.get(1).unwrap_or(&0)) << 8)
            | u32::from(*c.get(2).unwrap_or(&0));
        s.push(T[(n >> 18) as usize & 63] as char);
        s.push(T[(n >> 12) as usize & 63] as char);
        s.push(if c.len() > 1 {
            T[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        s.push(if c.len() > 2 {
            T[n as usize & 63] as char
        } else {
            '='
        });
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(s: &mut Server, id: u64, method: &str, params: Value) -> Value {
        let msg = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });
        s.handle_message(&msg.to_string()).expect("a reply")
    }

    fn tool_text(v: &Value) -> (bool, String) {
        let r = &v["result"];
        (
            !r["isError"].as_bool().unwrap(),
            r["content"][0]["text"].as_str().unwrap_or("").to_string(),
        )
    }

    #[test]
    fn handshake_and_tool_list() {
        let mut s = Server::new(&Options::default()).unwrap();
        let r = call(
            &mut s,
            1,
            "initialize",
            json!({ "protocolVersion": "2025-03-26" }),
        );
        assert_eq!(r["result"]["protocolVersion"], "2025-03-26");
        assert_eq!(r["result"]["serverInfo"]["name"], "forma");
        let note = json!({ "jsonrpc": "2.0", "method": "notifications/initialized" });
        assert!(s.handle_message(&note.to_string()).is_none());
        let r = call(&mut s, 2, "tools/list", json!({}));
        let names: Vec<&str> = r["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap())
            .collect();
        assert_eq!(
            names,
            [
                "forma_run",
                "forma_inspect",
                "forma_commands",
                "forma_python",
                "forma_render"
            ]
        );
        assert_eq!(call(&mut s, 3, "nope", json!({}))["error"]["code"], -32601);
        let bad = s.handle_message("{not json").unwrap();
        assert_eq!(bad["error"]["code"], -32700);
    }

    #[test]
    fn agent_builds_edits_and_saves_a_model() {
        let dir = std::env::temp_dir().join(format!("forma-mcp-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut s = Server::new(&Options {
            root: Some(dir.clone()),
            ..Options::default()
        })
        .unwrap();
        let (ok, out) = tool_text(&call(
            &mut s,
            1,
            "tools/call",
            json!({ "name": "forma_run", "arguments": { "commands":
                "Box 0,0,0 100,50,0 30\nCircle 200,0 20; SelLast; Extrude 40\nSelNone\nSelect #1\nMove 0,0 0,100\nSave stand.3dm" } }),
        ));
        assert!(ok, "{out}");
        assert!(dir.join("stand.3dm").exists());
        let (ok, summary) = tool_text(&call(
            &mut s,
            2,
            "tools/call",
            json!({ "name": "forma_inspect", "arguments": {} }),
        ));
        assert!(ok);
        assert!(summary.contains("units: cm"), "{summary}");
        assert!(
            summary.contains("#1 polysurface Default 0,100,0 .. 100,150,30"),
            "{summary}"
        );
        // Delete, undo: through the same command line.
        let (ok, _) = tool_text(&call(
            &mut s,
            3,
            "tools/call",
            json!({ "name": "forma_run", "arguments": { "commands": "Delete #1\nUndo" } }),
        ));
        assert!(ok);
        // Errors are reported, not thrown.
        let (ok, out) = tool_text(&call(
            &mut s,
            4,
            "tools/call",
            json!({ "name": "forma_run", "arguments": { "commands": "Box 0,0" } }),
        ));
        assert!(!ok && out.contains("error"), "{out}");
        let (_, help) = tool_text(&call(
            &mut s,
            5,
            "tools/call",
            json!({ "name": "forma_commands", "arguments": { "filter": "extrude" } }),
        ));
        assert!(help.contains("ExtrudeCrvAlongCrv"), "{help}");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn python_tool_builds_with_a_loop() {
        let mut s = Server::new(&Options::default()).unwrap();
        let (ok, out) = tool_text(&call(
            &mut s,
            1,
            "tools/call",
            json!({ "name": "forma_python", "arguments": { "code":
                "import forma\nfor i in range(3):\n    forma.run(f'Box {i*20},0,0 {i*20+10},10,0 10')\nprint(len(forma.objects()))" } }),
        ));
        assert!(ok, "{out}");
        assert_eq!(out.trim(), "3");
        let (ok, out) = tool_text(&call(
            &mut s,
            2,
            "tools/call",
            json!({ "name": "forma_python", "arguments": { "code": "import forma\nforma.run('Nope')" } }),
        ));
        assert!(!ok && out.contains("RuntimeError"), "{out}");
    }

    #[test]
    fn root_keeps_files_inside_one_folder() {
        let dir = std::env::temp_dir().join(format!("forma-mcp-root-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut s = Server::new(&Options {
            root: Some(dir.clone()),
            ..Options::default()
        })
        .unwrap();
        for bad in [
            "Save ../escape.3dm",
            "Save /etc/forma.3dm",
            "Open ../../secret.3dm",
        ] {
            let (ok, out) = s.run(&format!("Line 0,0 1,0\n{bad}"));
            assert!(!ok, "{bad}: {out}");
            assert!(out.contains("nothing was run"), "{out}");
        }
        assert_eq!(
            match &s.backend {
                Backend::Local(e) => e.doc().len(),
                Backend::Live => 99,
            },
            0,
            "a refused script runs no line at all"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn base64_matches_known_values() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }
}
