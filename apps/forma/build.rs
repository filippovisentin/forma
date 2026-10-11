//! Windows: put the application icon and version details into forma.exe
//! (ADR 0009). Other platforms: nothing to do.

fn main() {
    println!("cargo:rerun-if-changed=../../packaging/icon/forma.ico");
    #[cfg(windows)]
    windows_resources();
}

#[cfg(windows)]
fn windows_resources() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut res = winresource::WindowsResource::new();
    res.set_icon("../../packaging/icon/forma.ico");
    res.set("FileDescription", "Forma — 3D NURBS modeller");
    res.set("ProductName", "Forma");
    if let Err(e) = res.compile() {
        println!("cargo:warning=forma.exe built without its icon: {e}");
    }
}
