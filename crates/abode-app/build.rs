//! On Windows, embed the app icon and version info in abode.exe.

fn main() {
    println!("cargo:rerun-if-changed=assets/icon/icon.ico");
    #[cfg(windows)]
    {
        if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
            let mut res = winresource::WindowsResource::new();
            res.set_icon("assets/icon/icon.ico");
            res.set("ProductName", "Abode");
            res.set("FileDescription", "Abode");
            if let Err(e) = res.compile() {
                println!("cargo:warning=couldn't embed the Windows icon: {e}");
            }
        }
    }
}
