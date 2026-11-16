use std::path::PathBuf;

const FORWARDED: [&str; 2] = ["MISSAL_API_URL", "MISSAL_PURCHASE_URL"];

fn main() {
    let manifest = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap_or_default());
    let env_file = manifest.join("../../.env");
    println!("cargo:rerun-if-changed={}", env_file.display());
    for variable in FORWARDED {
        println!("cargo:rerun-if-env-changed={variable}");
    }
    let missing: Vec<&str> = FORWARDED
        .into_iter()
        .filter(|variable| std::env::var_os(variable).is_none_or(|value| value.is_empty()))
        .collect();
    if missing.is_empty() {
        return;
    }
    let Ok(entries) = dotenvy::from_path_iter(&env_file) else {
        return;
    };
    for (key, value) in entries.flatten() {
        if missing.contains(&key.as_str()) {
            println!("cargo:rustc-env={key}={value}");
        }
    }
}
