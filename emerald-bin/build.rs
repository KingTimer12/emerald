fn main() {
    // Icon for the .exe, the taskbar and every window (gpui and webview).
    // `embed-resource` is a build dependency of Windows hosts only.
    #[cfg(windows)]
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        println!("cargo:rerun-if-changed=assets/emerald.rc");
        println!("cargo:rerun-if-changed=assets/emerald.ico");
        embed_resource::compile("assets/emerald.rc", embed_resource::NONE)
            .manifest_optional()
            .unwrap();
    }
}
