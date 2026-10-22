fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("resources/icon.ico")
            .set("ProductName", "SoundFlow")
            .set("FileDescription", "SoundFlow Audio Core")
            .set("CompanyName", "SoundFlow")
            .set("OriginalFilename", "soundflow_lib.dll")
            .set_manifest_file("resources/windows.manifest");
        if let Err(e) = res.compile() {
            println!("cargo:warning=Embed Windows identity failed: {e}");
        }
    }
    println!("cargo:rerun-if-changed=resources/icon.ico");
    println!("cargo:rerun-if-changed=resources/windows.manifest");
    println!("cargo:rerun-if-env-changed=RC_PATH");
}
