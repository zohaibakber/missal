use std::path::PathBuf;

const API_URL: &str = "MISSAL_API_URL";

fn main() {
    let manifest = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap_or_default());
    let env_file = manifest.join("../../.env");
    println!("cargo:rerun-if-changed={}", env_file.display());
    println!("cargo:rerun-if-env-changed={API_URL}");
    if std::env::var_os(API_URL).is_some_and(|value| !value.is_empty()) {
        return;
    }
    let Ok(entries) = dotenvy::from_path_iter(&env_file) else {
        return;
    };
    if let Some((_, value)) = entries.flatten().find(|(key, _)| key == API_URL) {
        println!("cargo:rustc-env={API_URL}={value}");
    }
}
