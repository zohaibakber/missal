fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../../assets/icon.ico");
    #[cfg(windows)]
    windows::embed();
}

#[cfg(windows)]
mod windows {
    pub fn embed() {
        if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
            return;
        }
        let version = std::env::var("CARGO_PKG_VERSION").unwrap_or_default();
        let mut resource = winresource::WindowsResource::new();
        resource
            .set_icon("../../assets/icon.ico")
            .set("ProductName", "Missal")
            .set("FileDescription", "Missal")
            .set("InternalName", "missal")
            .set("OriginalFilename", "missal.exe")
            .set("CompanyName", "Zohaib Akber")
            .set("LegalCopyright", "Copyright (c) Zohaib Akber")
            .set("ProductVersion", &version)
            .set("FileVersion", &version);
        if let Err(error) = resource.compile() {
            println!("cargo:warning=could not embed Windows resources: {error}");
            std::process::exit(1);
        }
    }
}
