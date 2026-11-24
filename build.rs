fn main() {
    println!("cargo:rerun-if-changed=assets/keel.ico");
    #[cfg(windows)]
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/keel.ico");
        res.set("ProductName", "Keel");
        res.set("InternalName", "keel");
        res.set("OriginalFilename", "keel.exe");
        res.compile().expect("failed to embed Windows icon resource");
    }
}
