//! `Ask`: a language model writes a Forma Python script from a request in plain
//! words (ADR 0008). The model runs locally — Ollama, LM Studio, llama.cpp or any
//! server with an OpenAI-style API — reached by its URL, so nothing leaves the
//! computer unless the user points it elsewhere. The reply is only a proposal:
//! the front end shows the code and runs it when the user says so.
//!
//! The HTTP client is deliberately tiny (plain `http://`, one request per
//! connection): local model servers need nothing more, and it keeps TLS stacks
//! out of the build.

use serde_json::{json, Value};
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

/// Where the model is and which one to use.
#[derive(Debug, Clone, PartialEq)]
pub struct AiConfig {
    /// `http://localhost:11434` (Ollama), `http://localhost:1234/v1` (LM Studio
    /// and other OpenAI-style servers), or a full endpoint URL.
    pub url: String,
    pub model: String,
    /// Sent as `Authorization: Bearer …` when set (some servers want one).
    pub api_key: Option<String>,
    /// How long to wait for the reply (local models can be slow).
    pub timeout: Duration,
}

impl Default for AiConfig {
    fn default() -> Self {
        AiConfig {
            url: DEFAULT_URL.into(),
            model: DEFAULT_MODEL.into(),
            api_key: None,
            timeout: Duration::from_secs(600),
        }
    }
}

pub const DEFAULT_URL: &str = "http://localhost:11434";
pub const DEFAULT_MODEL: &str = "qwen2.5-coder:7b";

/// One turn of the conversation.
#[derive(Debug, Clone, PartialEq)]
pub struct Message {
    /// "system", "user" or "assistant".
    pub role: String,
    pub content: String,
}

impl Message {
    pub fn new(role: &str, content: impl Into<String>) -> Message {
        Message {
            role: role.into(),
            content: content.into(),
        }
    }
}

/// The two API shapes local servers speak.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Api {
    /// Ollama's `/api/chat`.
    Ollama,
    /// `/v1/chat/completions`.
    OpenAi,
}

/// A parsed `http://host[:port]/path` URL.
#[derive(Debug, PartialEq)]
struct Endpoint {
    host: String,
    port: u16,
    path: String,
    api: Api,
}

fn endpoint(url: &str) -> Result<Endpoint, String> {
    let url = url.trim();
    let rest = if let Some(r) = url.strip_prefix("http://") {
        r
    } else if url.starts_with("https://") {
        return Err(
            "only http:// model servers are supported (a local model, or a local proxy to a remote one)"
                .into(),
        );
    } else {
        url
    };
    let (authority, path) = match rest.find('/') {
        Some(i) => (&rest[..i], rest[i..].trim_end_matches('/')),
        None => (rest, ""),
    };
    if authority.is_empty() {
        return Err(format!("no host in the model URL {url:?}"));
    }
    let (host, port) = match authority.rsplit_once(':') {
        Some((h, p)) => (
            h,
            p.parse::<u16>()
                .map_err(|_| format!("bad port in the model URL {url:?}"))?,
        ),
        None => (authority, 80),
    };
    let (path, api) = if path.ends_with("/chat/completions") {
        (path.to_string(), Api::OpenAi)
    } else if path.ends_with("/v1") {
        (format!("{path}/chat/completions"), Api::OpenAi)
    } else if path.ends_with("/api/chat") {
        (path.to_string(), Api::Ollama)
    } else {
        (format!("{path}/api/chat"), Api::Ollama)
    };
    Ok(Endpoint {
        host: host.to_string(),
        port,
        path,
        api,
    })
}

/// Send the conversation to the model; its reply text.
pub fn chat(cfg: &AiConfig, messages: &[Message]) -> Result<String, String> {
    let ep = endpoint(&cfg.url)?;
    let msgs: Vec<Value> = messages
        .iter()
        .map(|m| json!({ "role": m.role, "content": m.content }))
        .collect();
    let body = match ep.api {
        // A large context: the prompt carries the command reference.
        Api::Ollama => json!({
            "model": cfg.model,
            "messages": msgs,
            "stream": false,
            "options": { "temperature": 0.2, "num_ctx": 16384 }
        }),
        Api::OpenAi => json!({
            "model": cfg.model,
            "messages": msgs,
            "temperature": 0.2,
            "stream": false
        }),
    };
    let (status, text) = post(&ep, &body.to_string(), cfg.api_key.as_deref(), cfg.timeout)?;
    let v: Value = serde_json::from_str(&text).map_err(|_| {
        format!(
            "the model server answered {status} with: {}",
            text.chars().take(300).collect::<String>()
        )
    })?;
    if let Some(e) = v.get("error") {
        let msg = e
            .get("message")
            .and_then(Value::as_str)
            .or(e.as_str())
            .map_or_else(|| e.to_string(), String::from);
        return Err(format!("model server: {msg}"));
    }
    if !(200..300).contains(&status) {
        return Err(format!("the model server answered {status}"));
    }
    let content = match ep.api {
        Api::Ollama => v.pointer("/message/content"),
        Api::OpenAi => v.pointer("/choices/0/message/content"),
    };
    content
        .and_then(Value::as_str)
        .map(String::from)
        .ok_or_else(|| "the model's reply has no text".into())
}

/// POST `body` as JSON; the status and the response body.
fn post(
    ep: &Endpoint,
    body: &str,
    key: Option<&str>,
    timeout: Duration,
) -> Result<(u16, String), String> {
    let place = format!("{}:{}", ep.host, ep.port);
    let addrs: Vec<_> = place
        .to_socket_addrs()
        .map_err(|e| format!("{place}: {e}"))?
        .collect();
    let mut stream = addrs
        .iter()
        .find_map(|a| TcpStream::connect_timeout(a, Duration::from_secs(5)).ok())
        .ok_or_else(|| {
            format!("cannot reach the model server at {place} (is it running? see Ask ▸ Model)")
        })?;
    stream
        .set_read_timeout(Some(timeout))
        .and_then(|()| stream.set_write_timeout(Some(Duration::from_secs(30))))
        .map_err(|e| e.to_string())?;
    let auth = key
        .filter(|k| !k.is_empty())
        .map_or(String::new(), |k| format!("Authorization: Bearer {k}\r\n"));
    let head = format!(
        "POST {} HTTP/1.1\r\nHost: {place}\r\nContent-Type: application/json\r\nAccept: application/json\r\n{auth}Content-Length: {}\r\nConnection: close\r\n\r\n",
        ep.path,
        body.len()
    );
    stream
        .write_all(head.as_bytes())
        .and_then(|()| stream.write_all(body.as_bytes()))
        .map_err(|e| format!("{place}: {e}"))?;
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).map_err(|e| {
        if matches!(
            e.kind(),
            std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
        ) {
            "the model took too long to answer".to_string()
        } else {
            format!("{place}: {e}")
        }
    })?;
    parse_response(&raw)
}

fn parse_response(raw: &[u8]) -> Result<(u16, String), String> {
    let split = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or("incomplete answer from the model server")?;
    let head = String::from_utf8_lossy(&raw[..split]);
    let mut body = &raw[split + 4..];
    let mut lines = head.lines();
    let status = lines
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse::<u16>().ok())
        .ok_or("not an HTTP answer")?;
    let mut chunked = false;
    for l in lines {
        if let Some((k, v)) = l.split_once(':') {
            let (k, v) = (k.trim().to_ascii_lowercase(), v.trim());
            if k == "transfer-encoding" && v.eq_ignore_ascii_case("chunked") {
                chunked = true;
            } else if k == "content-length" {
                if let Ok(n) = v.parse::<usize>() {
                    body = &body[..n.min(body.len())];
                }
            }
        }
    }
    let bytes = if chunked {
        dechunk(body)?
    } else {
        body.to_vec()
    };
    Ok((status, String::from_utf8_lossy(&bytes).into_owned()))
}

fn dechunk(mut b: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    loop {
        let eol = b
            .windows(2)
            .position(|w| w == b"\r\n")
            .ok_or("bad chunked answer")?;
        let size_text = String::from_utf8_lossy(&b[..eol]);
        let size_text = size_text.split(';').next().unwrap_or("").trim();
        let n = usize::from_str_radix(size_text, 16).map_err(|_| "bad chunk size")?;
        b = &b[eol + 2..];
        if n == 0 {
            return Ok(out);
        }
        if b.len() < n {
            return Err("truncated answer".into());
        }
        out.extend_from_slice(&b[..n]);
        b = b.get(n + 2..).unwrap_or(&[]);
    }
}

/// The instructions given to the model before the request: what the `forma`
/// module offers and every command's syntax (`commands`: name and help).
pub fn system_prompt(commands: &[(&str, &str)], units: &str) -> String {
    let mut s = String::from(
        "You write Python scripts for Forma, a Rhino-style 3D NURBS modeller. \
The script runs in Forma's embedded Python 3 interpreter (standard library only: math, random, itertools…; no numpy, no pip packages).\n\
\n\
The `forma` module:\n\
- forma.run(line) runs one Forma command line, exactly as typed in the command line, and returns its message; it raises RuntimeError when the command fails.\n\
- forma.objects() -> list of dicts: id, kind, layer, name, min (x,y,z), max (x,y,z).\n\
- forma.selection() -> list of selected ids; forma.select(ids) selects them.\n\
- forma.units() -> model units (\"mm\", \"cm\", \"m\", \"in\", \"ft\").\n\
- print(...) writes to Forma's command history.\n\
\n\
Command lines: words and points separated by spaces; a point is x,y,z with no spaces (e.g. Box 0,0,0 100,50,0 30). \
Objects are referred to as #id (e.g. Move #3 0,0,0 0,0,10). Commands that take objects use the selection when no ids are given. \
Every forma.run call changes the model; the whole script is undone with one Undo.\n\
\n\
Rules: answer with ONE ```python code block, then at most two short sentences. Use f-strings to build command lines. \
Use only the commands listed below, with their exact syntax. Do not invent commands or keyword arguments. \
Put new geometry on a layer named after what it is (forma.run(\"Layer Walls\") creates and activates a layer).\n",
    );
    s.push_str(&format!(
        "\nThe model units are {units}.\n\nCommands (syntax — what it does):\n"
    ));
    for (name, help) in commands {
        if help.is_empty() {
            s.push_str(name);
        } else {
            s.push_str(help);
        }
        s.push('\n');
    }
    s
}

/// The Python code of a reply: its first ```python block (or first fenced block),
/// or the whole reply when it has no fences and looks like code.
pub fn extract_code(reply: &str) -> Option<String> {
    let mut rest = reply;
    let mut first_any: Option<String> = None;
    while let Some(i) = rest.find("```") {
        let after = &rest[i + 3..];
        let (lang, body) = after.split_once('\n').unwrap_or((after, ""));
        let end = body.find("```").unwrap_or(body.len());
        let code = body[..end].trim_end().to_string();
        let lang = lang.trim().to_ascii_lowercase();
        if lang == "python" || lang == "py" {
            return Some(code);
        }
        if first_any.is_none() && !code.trim().is_empty() {
            first_any = Some(code);
        }
        rest = body.get(end + 3..).unwrap_or("");
    }
    first_any.or_else(|| {
        let t = reply.trim();
        (t.contains("forma.") || t.starts_with("import ")).then(|| t.to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    #[test]
    fn urls() {
        let e = endpoint("http://localhost:11434").unwrap();
        assert_eq!(
            (e.host.as_str(), e.port, e.path.as_str()),
            ("localhost", 11434, "/api/chat")
        );
        assert_eq!(e.api, Api::Ollama);
        let e = endpoint("http://127.0.0.1:1234/v1/").unwrap();
        assert_eq!(
            (e.path.as_str(), e.api),
            ("/v1/chat/completions", Api::OpenAi)
        );
        let e = endpoint("192.168.1.20:8080/v1/chat/completions").unwrap();
        assert_eq!(
            (e.host.as_str(), e.port, e.api),
            ("192.168.1.20", 8080, Api::OpenAi)
        );
        assert!(endpoint("https://api.example.com/v1").is_err());
        assert!(endpoint("http://host:notaport").is_err());
    }

    #[test]
    fn code_blocks() {
        let r = "Here:\n```python\nimport forma\nforma.run(\"Line 0,0,0 1,0,0\")\n```\nDone.";
        assert_eq!(
            extract_code(r).unwrap(),
            "import forma\nforma.run(\"Line 0,0,0 1,0,0\")"
        );
        assert_eq!(extract_code("```\nprint(1)\n```").unwrap(), "print(1)");
        assert_eq!(
            extract_code("```text\nnotes\n```\n```py\nx = 1\n```").unwrap(),
            "x = 1"
        );
        assert!(extract_code("I cannot help with that.").is_none());
        assert_eq!(
            extract_code("import forma\nforma.run('Box')").unwrap(),
            "import forma\nforma.run('Box')"
        );
    }

    #[test]
    fn chunked_and_plain_answers() {
        let raw = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nhello\r\n6\r\n world\r\n0\r\n\r\n";
        assert_eq!(parse_response(raw).unwrap(), (200, "hello world".into()));
        let raw = b"HTTP/1.1 404 Not Found\r\nContent-Length: 2\r\n\r\nnoXX";
        assert_eq!(parse_response(raw).unwrap(), (404, "no".into()));
    }

    /// A one-shot fake model server: answers `reply` and returns the request.
    fn serve(reply: String) -> (String, std::thread::JoinHandle<String>) {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", l.local_addr().unwrap());
        let h = std::thread::spawn(move || {
            let (mut s, _) = l.accept().unwrap();
            let mut req = Vec::new();
            let mut buf = [0u8; 4096];
            // Read the head, then the body by its Content-Length.
            loop {
                let n = s.read(&mut buf).unwrap();
                req.extend_from_slice(&buf[..n]);
                if let Some(i) = req.windows(4).position(|w| w == b"\r\n\r\n") {
                    let head = String::from_utf8_lossy(&req[..i]).to_lowercase();
                    let len: usize = head
                        .lines()
                        .find_map(|l| l.strip_prefix("content-length:"))
                        .map_or(0, |v| v.trim().parse().unwrap());
                    while req.len() < i + 4 + len {
                        let n = s.read(&mut buf).unwrap();
                        req.extend_from_slice(&buf[..n]);
                    }
                    break;
                }
            }
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{reply}",
                reply.len()
            );
            s.write_all(resp.as_bytes()).unwrap();
            String::from_utf8_lossy(&req).into_owned()
        });
        (url, h)
    }

    #[test]
    fn talks_to_ollama_and_openai_style_servers() {
        let msgs = [
            Message::new("system", "sys"),
            Message::new("user", "four boxes"),
        ];
        let (url, h) = serve(
            json!({ "message": { "role": "assistant", "content": "```python\nprint(1)\n```" } })
                .to_string(),
        );
        let cfg = AiConfig {
            url,
            model: "m1".into(),
            ..AiConfig::default()
        };
        assert_eq!(chat(&cfg, &msgs).unwrap(), "```python\nprint(1)\n```");
        let req = h.join().unwrap();
        assert!(req.starts_with("POST /api/chat HTTP/1.1"), "{req}");
        assert!(
            req.contains("\"model\":\"m1\"") && req.contains("four boxes"),
            "{req}"
        );

        let (url, h) = serve(
            json!({ "choices": [ { "message": { "role": "assistant", "content": "ok" } } ] })
                .to_string(),
        );
        let cfg = AiConfig {
            url: format!("{url}/v1"),
            model: "m2".into(),
            api_key: Some("k".into()),
            ..AiConfig::default()
        };
        assert_eq!(chat(&cfg, &msgs).unwrap(), "ok");
        let req = h.join().unwrap();
        assert!(
            req.starts_with("POST /v1/chat/completions HTTP/1.1"),
            "{req}"
        );
        assert!(req.contains("Authorization: Bearer k"), "{req}");

        let (url, h) = serve(json!({ "error": "model \"x\" not found" }).to_string());
        let cfg = AiConfig {
            url,
            ..AiConfig::default()
        };
        let err = chat(&cfg, &msgs).unwrap_err();
        assert!(err.contains("not found"), "{err}");
        h.join().unwrap();
    }

    #[test]
    fn unreachable_server_is_a_clear_error() {
        // Bind then drop: the port is (almost certainly) closed.
        let port = TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let cfg = AiConfig {
            url: format!("http://127.0.0.1:{port}"),
            ..AiConfig::default()
        };
        let err = chat(&cfg, &[Message::new("user", "hi")]).unwrap_err();
        assert!(err.contains("cannot reach"), "{err}");
    }

    #[test]
    fn the_prompt_lists_the_commands() {
        let e = forma_engine::Engine::new();
        let list = e.command_list();
        let p = system_prompt(&list, "cm");
        assert!(p.contains("forma.run(line)") && p.contains("units are cm"));
        assert!(
            p.lines().any(|l| l.starts_with("Box ")),
            "Box syntax missing"
        );
        // Small enough for a local model's context (16k tokens ≈ 64 kB).
        assert!(p.len() < 40_000, "prompt is {} bytes", p.len());
    }
}
