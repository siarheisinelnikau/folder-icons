fn main() {
    println!("cargo:rerun-if-changed=assets/FolderIcons.ico");
    println!("cargo:rerun-if-changed=Cargo.toml");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let version = std::env::var("CARGO_PKG_VERSION").expect("Package version");
        winresource::WindowsResource::new()
            .set_icon("assets/FolderIcons.ico")
            .set_language(0x0409)
            .set("ProductName", "FolderIcons")
            .set(
                "FileDescription",
                "FolderIcons - original folder appearance",
            )
            .set("InternalName", "FolderIcons")
            .set("OriginalFilename", &format!("FolderIcons-v{version}.exe"))
            .compile()
            .expect("Could not embed application icon and version; Windows SDK rc.exe is required");
    }
}
