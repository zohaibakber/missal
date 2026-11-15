use std::path::PathBuf;

const CLIENT_ID: &str = "WORKOS_CLIENT_ID";

fn main() {
    let manifest = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap_or_default());
    let env_file = manifest.join("../../.env");
    println!("cargo:rerun-if-changed={}", env_file.display());
    println!("cargo:rerun-if-env-changed={CLIENT_ID}");
    if std::env::var_os(CLIENT_ID).is_some_and(|value| !value.is_empty()) {
        return;
    }
    let Ok(entries) = dotenvy::from_path_iter(&env_file) else {
        return;
    };
    if let Some((_, value)) = entries.flatten().find(|(key, _)| key == CLIENT_ID) {
        println!("cargo:rustc-env={CLIENT_ID}={value}");
    }
}
