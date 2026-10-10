//! Opt-in timing log: set `FORMA_PROFILE=1` to print how long the expensive
//! steps take (scene rebuild, snap data, highlight, frame) to stderr.

use std::sync::OnceLock;
use std::time::Instant;

pub fn enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("FORMA_PROFILE").is_some_and(|v| v != "0"))
}

/// Logs the elapsed time when dropped (only when profiling is enabled).
pub struct Span {
    what: &'static str,
    start: Option<Instant>,
    /// Do not log spans shorter than this (milliseconds).
    min_ms: f64,
}

pub fn span(what: &'static str) -> Span {
    span_min(what, 0.0)
}

pub fn span_min(what: &'static str, min_ms: f64) -> Span {
    Span {
        what,
        start: enabled().then(Instant::now),
        min_ms,
    }
}

impl Drop for Span {
    fn drop(&mut self) {
        if let Some(s) = self.start {
            let ms = s.elapsed().as_secs_f64() * 1000.0;
            if ms >= self.min_ms {
                eprintln!("[forma] {:<22} {ms:8.2} ms", self.what);
            }
        }
    }
}
