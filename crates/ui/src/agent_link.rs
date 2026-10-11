//! Agent connection: lets an AI assistant drive the open model through
//! `forma-cli mcp --live` (ADR 0007). Off by default; Tools ▸ Agent Connection
//! (or `AgentConnection On|Off`) turns it on for this session.
//!
//! A thread listens on 127.0.0.1 (random port) and writes the port and a random
//! token to `agent.txt` in the settings folder (readable only by the user). Each
//! request carries the token; accepted requests are handed to the frame loop,
//! which runs them like typed commands (history, undo) and sends back the output.

use crate::{FormaApp, LogKind};
use eframe::egui;
use forma_engine::agent;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::time::Duration;

/// A request waiting for the frame loop, with the way back.
struct Request {
    verb: String,
    text: String,
    reply: Sender<(bool, String)>,
}

pub(crate) struct AgentLink {
    requests: Receiver<Request>,
    stop: Arc<AtomicBool>,
    pub(crate) port: u16,
}

impl AgentLink {
    /// Start listening and write `agent.txt`.
    fn start(ctx: egui::Context) -> Result<AgentLink, String> {
        let file = agent::link_file().ok_or("no settings folder")?;
        let listener = TcpListener::bind(("127.0.0.1", 0)).map_err(|e| e.to_string())?;
        let port = listener.local_addr().map_err(|e| e.to_string())?.port();
        listener.set_nonblocking(true).map_err(|e| e.to_string())?;
        let token = agent::new_token();
        write_private(&file, &format!("{port}\n{token}\n"))
            .map_err(|e| format!("{}: {e}", file.display()))?;
        let (tx, rx) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let stop2 = Arc::clone(&stop);
        std::thread::spawn(move || {
            while !stop2.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, addr)) if addr.ip().is_loopback() => {
                        let (tx, token, ctx) = (tx.clone(), token.clone(), ctx.clone());
                        std::thread::spawn(move || serve(stream, &token, &tx, &ctx));
                    }
                    Ok(_) => {} // never happens on 127.0.0.1; dropped
                    Err(_) => std::thread::sleep(Duration::from_millis(50)),
                }
            }
        });
        Ok(AgentLink {
            requests: rx,
            stop,
            port,
        })
    }

    fn shut(&self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(f) = agent::link_file() {
            let _ = std::fs::remove_file(f);
        }
    }
}

/// `agent.txt` with owner-only permissions where the platform has them.
fn write_private(path: &std::path::Path, text: &str) -> std::io::Result<()> {
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d)?;
    }
    let _ = std::fs::remove_file(path);
    let mut o = std::fs::OpenOptions::new();
    o.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        o.mode(0o600);
    }
    o.open(path)?.write_all(text.as_bytes())
}

/// One connection: one request line, one reply line.
fn serve(stream: TcpStream, token: &str, tx: &Sender<Request>, ctx: &egui::Context) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
    let Ok(mut writer) = stream.try_clone() else {
        return;
    };
    let mut line = String::new();
    // At most 1 MB per request.
    if BufReader::new(stream.take(1 << 20))
        .read_line(&mut line)
        .is_err()
    {
        return;
    }
    let reply = match agent::parse_request(&line) {
        Some((t, verb, text)) if agent::same_token(t, token) => {
            let (rtx, rrx) = mpsc::channel();
            let sent = tx.send(Request {
                verb: verb.to_string(),
                text,
                reply: rtx,
            });
            ctx.request_repaint();
            match sent
                .ok()
                .and_then(|()| rrx.recv_timeout(Duration::from_secs(120)).ok())
            {
                Some((ok, text)) => agent::reply_line(ok, &text),
                None => agent::reply_line(false, "the Forma app did not answer in time"),
            }
        }
        _ => agent::reply_line(false, "refused: wrong or missing token"),
    };
    let _ = writeln!(writer, "{reply}");
}

impl FormaApp {
    /// `AgentConnection On|Off` (and the Tools menu).
    pub(crate) fn agent_command(&mut self, ctx: &egui::Context, line: &str) {
        let arg = line.split_whitespace().nth(1).map(str::to_ascii_lowercase);
        let on = match arg.as_deref() {
            Some("on") => true,
            Some("off") => false,
            None => self.agent.is_none(),
            Some(a) => {
                self.log(LogKind::Error, format!("AgentConnection On|Off, not {a}"));
                return;
            }
        };
        if on == self.agent.is_some() {
            self.log(LogKind::Normal, self.agent_status());
            return;
        }
        if on {
            match AgentLink::start(ctx.clone()) {
                Ok(link) => {
                    self.agent = Some(link);
                    self.log(LogKind::Normal, self.agent_status());
                }
                Err(e) => self.log(LogKind::Error, format!("agent connection: {e}")),
            }
        } else {
            self.agent_stop();
            self.log(LogKind::Normal, "agent connection off");
        }
    }

    fn agent_status(&self) -> String {
        match &self.agent {
            Some(l) => format!(
                "agent connection on (127.0.0.1:{}): AI assistants connected with `forma-cli mcp --live` can now edit this model; Tools ▸ Agent Connection turns it off",
                l.port
            ),
            None => "agent connection off".into(),
        }
    }

    /// Turn the connection off (also on exit).
    pub(crate) fn agent_stop(&mut self) {
        if let Some(l) = self.agent.take() {
            l.shut();
        }
    }

    /// Called every frame: answer waiting requests.
    pub(crate) fn agent_tick(&mut self) {
        let Some(link) = &self.agent else {
            return;
        };
        let pending: Vec<Request> = link.requests.try_iter().collect();
        for r in pending {
            let answer = self.agent_handle(&r.verb, &r.text);
            let _ = r.reply.send(answer);
        }
    }

    fn agent_handle(&mut self, verb: &str, text: &str) -> (bool, String) {
        if self.confirm.is_some() || self.autosave.offer.is_some() || self.new_model.is_some() {
            return (
                false,
                "the Forma app is waiting for the user to answer a dialog; try again after it is closed"
                    .into(),
            );
        }
        match verb {
            agent::INSPECT => {
                let limit = text.trim().parse().unwrap_or(200);
                (true, agent::inspect(&self.engine, limit))
            }
            agent::SNAPSHOT => {
                let path =
                    std::env::temp_dir().join(format!("forma-snapshot-{}.3dm", agent::new_token()));
                match forma_engine::write_copy(&self.engine.doc().snapshot(), &path) {
                    Ok(()) => (true, path.display().to_string()),
                    Err(e) => (false, e.to_string()),
                }
            }
            agent::RUN => self.agent_run(text),
            agent::PYTHON => {
                let before = self.log_count;
                let _ = self.run_python_source(text, "<agent>");
                let new = (self.log_count - before).min(self.log.len());
                let mut ok = true;
                let mut out = String::new();
                for (kind, msg) in &self.log[self.log.len() - new..] {
                    if *kind == LogKind::Command {
                        continue;
                    }
                    ok &= *kind != LogKind::Error;
                    out.push_str(msg);
                    out.push('\n');
                }
                (ok, out)
            }
            v => (false, format!("unknown request {v}")),
        }
    }

    /// Run lines like typed commands; stop at the first error.
    fn agent_run(&mut self, script: &str) -> (bool, String) {
        let mut out = String::new();
        // A tool the user left half-way would take the agent's input.
        self.tool = None;
        self.pending = None;
        let was_empty = self.engine.doc().is_empty();
        for line in script.split(['\n', ';']) {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            out.push_str("> ");
            out.push_str(line);
            out.push('\n');
            let (ok, text) = self.run_captured(line);
            out.push_str(&text);
            let failed = !ok;
            if self.confirm.is_some() {
                out.push_str(
                    "(the app asks the user to save changes first; nothing more was run)\n",
                );
                return (false, out);
            }
            if failed {
                out.push_str("(stopped; the lines after this one did not run)\n");
                return (false, out);
            }
        }
        if was_empty && !self.engine.doc().is_empty() {
            self.pending_fit = true; // the first objects: show them in every view
        }
        (true, out)
    }
}

/// View commands an agent may use in the live app.
pub(crate) const VIEW_WORDS: [&str; 16] = [
    "ze",
    "zea",
    "zoomextents",
    "zs",
    "zoomselected",
    "top",
    "front",
    "right",
    "bottom",
    "back",
    "left",
    "perspective",
    "persp",
    "wireframe",
    "shaded",
    "maximize",
];
