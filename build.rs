fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");

    // Embed the icon in the Windows executable (shown in Explorer and the taskbar).
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let result = winresource::WindowsResource::new()
            .set_icon("assets/icon.ico")
            .compile();
        if let Err(e) = result {
            // Native Windows builds must have the icon; cross-builds without
            // the resource compiler (windres) still work, just without it.
            if std::env::var("HOST").is_ok_and(|h| h.contains("windows")) {
                panic!("failed to embed the Windows icon: {e}");
            }
            println!("cargo:warning=Windows icon not embedded: {e}");
        }
    }
}
