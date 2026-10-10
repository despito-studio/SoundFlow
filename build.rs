fn main() {
    slint_build::compile("ui/soundflow.slint").expect("Compile the native SoundFlow interface");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        winresource::WindowsResource::new()
            .set_icon("resources/icon.ico")
            .set("ProductName", "SoundFlow")
            .set("FileDescription", "SoundFlow Music")
            .set("CompanyName", "SoundFlow")
            .set("OriginalFilename", "SoundFlow.exe")
            .set_manifest_file("resources/windows.manifest")
            .compile()
            .expect("Embed SoundFlow Windows identity and icons");
    }
    println!("cargo:rerun-if-changed=ui");
    println!("cargo:rerun-if-changed=resources/icon.ico");
    println!("cargo:rerun-if-changed=resources/windows.manifest");
    println!("cargo:rerun-if-env-changed=RC_PATH");
}
