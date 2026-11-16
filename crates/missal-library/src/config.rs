const URL_VARIABLE: &str = "MISSAL_LIBRARY_URL";
const BUILT_URL: Option<&str> = option_env!("MISSAL_LIBRARY_URL");

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryConfig {
    pub base_url: String,
}

fn usable(value: &str) -> Option<String> {
    let value = value.trim().trim_end_matches('/');
    (value.starts_with("https://") || value.starts_with("http://127.0.0.1"))
        .then(|| value.to_owned())
}

impl LibraryConfig {
    pub fn resolve() -> Option<LibraryConfig> {
        std::env::var(URL_VARIABLE)
            .ok()
            .as_deref()
            .and_then(usable)
            .or_else(|| BUILT_URL.and_then(usable))
            .map(|base_url| LibraryConfig { base_url })
    }
}
