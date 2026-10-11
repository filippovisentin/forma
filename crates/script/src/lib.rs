//! Python scripting inside Forma (ADR 0008): an embedded interpreter (RustPython,
//! no Python installation needed) with a `forma` module whose calls go through the
//! command line, like everything else (ground rule 1).
//!
//! ```python
//! import forma
//! forma.run("Layer Walls")
//! for i in range(5):
//!     forma.run(f"Box {i*120},0,0 {i*120+100},20,0 270")
//! for o in forma.objects():
//!     print(o["id"], o["kind"], o["layer"], o["min"], o["max"])
//! ```
//!
//! `forma.run(line)` runs one command line and returns its message, raising
//! `RuntimeError` when the command fails; `objects()`, `selection()`, `select()`
//! and `units()` read the model; `print` goes to the command history.
//!
//! The front end decides where commands go by implementing [`Host`]: the desktop
//! app runs them like typed commands, [`EngineHost`] runs them on an engine.

pub mod ai;

use forma_engine::Engine;
use std::cell::Cell;

/// What a script sees of one object.
#[derive(Debug, Clone, PartialEq)]
pub struct ObjectInfo {
    pub id: u64,
    pub kind: String,
    pub layer: String,
    pub name: Option<String>,
    pub min: [f64; 3],
    pub max: [f64; 3],
}

/// Where a script's calls go.
pub trait Host {
    /// Run one command line; the message on success, the error text otherwise.
    fn run(&mut self, line: &str) -> Result<String, String>;
    fn objects(&mut self) -> Vec<ObjectInfo>;
    fn selection(&mut self) -> Vec<u64>;
    fn units(&mut self) -> String;
    /// Text the script printed.
    fn output(&mut self, text: &str);
}

/// The objects of an engine's document, for hosts.
pub fn objects_of(engine: &Engine) -> Vec<ObjectInfo> {
    let doc = engine.doc();
    doc.objects()
        .map(|o| {
            let b = o.geometry.bounding_box();
            ObjectInfo {
                id: o.id.0,
                kind: o.geometry.kind().to_string(),
                layer: doc.layer(o.layer).name.clone(),
                name: o.name.clone(),
                min: [b.min.x, b.min.y, b.min.z],
                max: [b.max.x, b.max.y, b.max.z],
            }
        })
        .collect()
}

/// A host running commands straight on an engine (CLI, MCP, tests). Printed text
/// is collected in `out`.
pub struct EngineHost<'a> {
    pub engine: &'a mut Engine,
    pub out: String,
}

impl<'a> EngineHost<'a> {
    pub fn new(engine: &'a mut Engine) -> Self {
        Self {
            engine,
            out: String::new(),
        }
    }

    /// Run a script; everything it changed is one undo step.
    pub fn run_script(&mut self, source: &str, name: &str) -> Result<(), String> {
        let depth = self.engine.doc().undo_depth();
        let r = run_python(self, source, name);
        self.engine.ctx.doc.group_undo_since(depth);
        r
    }
}

impl Host for EngineHost<'_> {
    fn run(&mut self, line: &str) -> Result<String, String> {
        self.engine.run_line(line).map_err(|e| e.to_string())
    }
    fn objects(&mut self) -> Vec<ObjectInfo> {
        objects_of(self.engine)
    }
    fn selection(&mut self) -> Vec<u64> {
        self.engine.ctx.selection.iter().map(|id| id.0).collect()
    }
    fn units(&mut self) -> String {
        self.engine.doc().units.abbreviation().to_string()
    }
    fn output(&mut self, text: &str) {
        self.out.push_str(text);
    }
}

thread_local! {
    /// The host of the script running on this thread (set only during
    /// [`run_python`], which restores it before returning).
    static HOST: Cell<Option<*mut (dyn Host + 'static)>> = const { Cell::new(None) };
}

/// Call `f` with the current host.
fn with_host<R>(f: impl FnOnce(&mut dyn Host) -> R) -> Option<R> {
    let p = HOST.with(Cell::get)?;
    // SAFETY: the pointer is set by `run_python` from a `&mut dyn Host` that
    // outlives the interpreter run, only on this thread, and cleared before
    // `run_python` returns; module functions run synchronously inside it and do
    // not keep the reference.
    Some(f(unsafe { &mut *p }))
}

#[rustpython_vm::pymodule]
mod forma {
    use super::with_host;
    use rustpython_vm::builtins::{PyListRef, PyStrRef};
    use rustpython_vm::{PyObjectRef, PyResult, VirtualMachine};

    fn no_host(vm: &VirtualMachine) -> rustpython_vm::builtins::PyBaseExceptionRef {
        vm.new_runtime_error("no Forma model to work on".to_owned())
    }

    /// Run one command line, e.g. `forma.run("Box 0,0,0 10,10,0 10")`.
    #[pyfunction]
    fn run(line: PyStrRef, vm: &VirtualMachine) -> PyResult<String> {
        match with_host(|h| h.run(line.as_str())) {
            Some(Ok(msg)) => Ok(msg),
            Some(Err(e)) => Err(vm.new_runtime_error(format!("{}: {e}", line.as_str()))),
            None => Err(no_host(vm)),
        }
    }

    /// Every object: dicts with id, kind, layer, name, min, max.
    #[pyfunction]
    fn objects(vm: &VirtualMachine) -> PyResult<PyListRef> {
        let objs = with_host(|h| h.objects()).ok_or_else(|| no_host(vm))?;
        let mut items: Vec<PyObjectRef> = Vec::with_capacity(objs.len());
        for o in objs {
            let d = vm.ctx.new_dict();
            d.set_item("id", vm.new_pyobj(o.id), vm)?;
            d.set_item("kind", vm.new_pyobj(o.kind), vm)?;
            d.set_item("layer", vm.new_pyobj(o.layer), vm)?;
            let name = match o.name {
                Some(n) => vm.new_pyobj(n),
                None => vm.ctx.none(),
            };
            d.set_item("name", name, vm)?;
            let [a, b, c] = o.min;
            d.set_item("min", vm.new_tuple((a, b, c)).into(), vm)?;
            let [a, b, c] = o.max;
            d.set_item("max", vm.new_tuple((a, b, c)).into(), vm)?;
            items.push(d.into());
        }
        Ok(vm.ctx.new_list(items))
    }

    /// Ids of the selected objects.
    #[pyfunction]
    fn selection(vm: &VirtualMachine) -> PyResult<PyListRef> {
        let ids = with_host(|h| h.selection()).ok_or_else(|| no_host(vm))?;
        Ok(vm
            .ctx
            .new_list(ids.into_iter().map(|i| vm.new_pyobj(i)).collect()))
    }

    /// Model units: "mm", "cm", "m", "in" or "ft".
    #[pyfunction]
    fn units(vm: &VirtualMachine) -> PyResult<String> {
        with_host(|h| h.units()).ok_or_else(|| no_host(vm))
    }

    #[pyfunction]
    fn _out(text: PyStrRef) {
        with_host(|h| h.output(text.as_str()));
    }
}

/// Makes `print` write to the host, and adds `forma.select`.
const PRELUDE: &str = r##"
import builtins as _builtins
import forma as _forma

def _print(*args, sep=" ", end="\n", file=None, flush=False):
    _forma._out(sep.join(str(a) for a in args) + end)

_builtins.print = _print

def _select(*ids):
    """Select these objects (ids or a list of ids) instead of the current selection."""
    if len(ids) == 1 and isinstance(ids[0], (list, tuple)):
        ids = ids[0]
    _forma.run("SelNone")
    if ids:
        _forma.run("Select " + " ".join("#%d" % int(i) for i in ids))

_forma.select = _select
"##;

/// Run Python `source` (named `name` in tracebacks) against `host`. On failure the
/// error is the Python traceback.
pub fn run_python(host: &mut dyn Host, source: &str, name: &str) -> Result<(), String> {
    if HOST.with(Cell::get).is_some() {
        return Err("a script cannot run another script".into());
    }
    // SAFETY: only the lifetime is erased; see `with_host`. The guard clears the
    // pointer on every exit path, panics included.
    let ptr: *mut (dyn Host + '_) = host;
    let ptr: *mut (dyn Host + 'static) = unsafe { std::mem::transmute(ptr) };
    HOST.with(|h| h.set(Some(ptr)));
    struct Clear;
    impl Drop for Clear {
        fn drop(&mut self) {
            HOST.with(|h| h.set(None));
        }
    }
    let _clear = Clear;

    let interp = rustpython::InterpreterConfig::new()
        .init_stdlib()
        .add_native_module("forma".to_owned(), forma::make_module)
        .interpreter();
    interp.enter(|vm| {
        let scope = vm.new_scope_with_builtins();
        let run = |src: &str, file: &str| {
            vm.run_code_string(scope.clone(), src, file.to_owned())
                .map(|_| ())
                .map_err(|e| {
                    let mut s = String::new();
                    let _ = vm.write_exception(&mut s, &e);
                    s.trim_end().to_string()
                })
        };
        run(PRELUDE, "<forma prelude>")?;
        run(source, name)
    })
}

/// The command name of a script file: `pannelli_muro.py` → `PannelliMuro`.
pub fn command_name(file_stem: &str) -> String {
    file_stem
        .split(['_', '-', ' '])
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut c = w.chars();
            c.next()
                .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
                .unwrap_or_default()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_script_builds_reads_and_undoes_in_one_step() {
        let mut e = Engine::new();
        e.run_line("New cm").unwrap();
        let mut h = EngineHost::new(&mut e);
        h.run_script(
            r#"
import forma, math
forma.run("Layer Walls")
for i in range(4):
    forma.run(f"Box {i*120},0,0 {i*120+100},20,0 270")
objs = forma.objects()
print(len(objs), objs[0]["kind"], objs[0]["layer"], forma.units())
print(round(math.hypot(3, 4)))
forma.select([o["id"] for o in objs[:2]])
print(forma.selection())
"#,
            "test.py",
        )
        .unwrap();
        assert_eq!(h.out, "4 polysurface Walls cm\n5\n[1, 2]\n");
        assert_eq!(e.doc().len(), 4);
        e.run_line("Undo").unwrap();
        assert_eq!(e.doc().len(), 0, "one Undo removes the whole script");
    }

    #[test]
    fn command_errors_are_python_exceptions() {
        let mut e = Engine::new();
        let mut h = EngineHost::new(&mut e);
        h.run_script(
            r#"
import forma
try:
    forma.run("Frobnicate 1 2")
except RuntimeError as err:
    print("caught:", err)
"#,
            "t.py",
        )
        .unwrap();
        assert!(
            h.out.starts_with("caught: Frobnicate 1 2: unknown command"),
            "{}",
            h.out
        );
        // An uncaught error stops the script with a traceback naming the line.
        let err = h
            .run_script("import forma\nforma.run('Box 0,0')\n", "bad.py")
            .unwrap_err();
        assert!(err.contains("bad.py") && err.contains("line 2"), "{err}");
        assert!(err.contains("RuntimeError"), "{err}");
        // Syntax errors too.
        assert!(h.run_script("for i in\n", "syntax.py").is_err());
    }

    #[test]
    fn command_names_from_files() {
        assert_eq!(command_name("pannelli_muro"), "PannelliMuro");
        assert_eq!(command_name("stanza"), "Stanza");
        assert_eq!(command_name("my-stair tool"), "MyStairTool");
    }
}
