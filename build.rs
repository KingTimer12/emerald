use std::path::Path;

fn main() {
    embed_locales();

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

/// Lists every `locales/*.toml` for `i18n`, English first: dropping a file
/// there is all it takes to ship a language.
fn embed_locales() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("locales");
    println!("cargo:rerun-if-changed={}", dir.display());
    let mut codes: Vec<String> = std::fs::read_dir(&dir)
        .expect("locales/")
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            println!("cargo:rerun-if-changed={}", path.display());
            (path.extension()? == "toml").then(|| path.file_stem()?.to_str().map(str::to_owned))?
        })
        .collect();
    codes.sort_by_key(|code| (code != "en-US", code.clone()));
    assert_eq!(codes.first().map(String::as_str), Some("en-US"), "locales/en-US.toml is required");
    let entries: String = codes
        .iter()
        .map(|code| format!("    ({code:?}, include_str!({:?})),\n", dir.join(format!("{code}.toml"))))
        .collect();
    let out = Path::new(&std::env::var("OUT_DIR").unwrap()).join("locales.rs");
    std::fs::write(out, format!("&[\n{entries}]\n")).unwrap();
}
