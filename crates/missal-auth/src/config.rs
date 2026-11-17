const URL_VARIABLE: &str = "API_URL";
const PURCHASE_VARIABLE: &str = "PURCHASE_URL";
const BUILT_PURCHASE_URL: Option<&str> = option_env!("PURCHASE_URL");

#[cfg(not(debug_assertions))]
const BUILT_URL: Option<&str> = Some(env!(
    "API_URL",
    "release builds need API_URL set to the Missal API URL"
));

#[cfg(not(debug_assertions))]
const _: () = assert!(
    !env!("API_URL").is_empty(),
    "release builds need a non-empty API_URL"
);

#[cfg(debug_assertions)]
const BUILT_URL: Option<&str> = option_env!("API_URL");

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub base_url: String,
    pub purchase_url: Option<String>,
}

fn https(value: &str) -> Option<String> {
    let value = value.trim();
    (value.starts_with("https://") && value.len() > "https://".len()).then(|| value.to_owned())
}

fn usable(value: &str) -> Option<String> {
    let value = value.trim().trim_end_matches('/');
    let local = ["http://127.0.0.1", "http://localhost"]
        .iter()
        .any(|prefix| value.starts_with(prefix));
    (value.starts_with("https://") || local).then(|| value.to_owned())
}

impl Config {
    pub fn resolve() -> Option<Config> {
        std::env::var(URL_VARIABLE)
            .ok()
            .as_deref()
            .and_then(usable)
            .or_else(|| BUILT_URL.and_then(usable))
            .map(|base_url| Config {
                base_url,
                purchase_url: std::env::var(PURCHASE_VARIABLE)
                    .ok()
                    .as_deref()
                    .and_then(https)
                    .or_else(|| BUILT_PURCHASE_URL.and_then(https)),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::{https, usable};

    #[test]
    fn only_https_or_a_local_server_is_accepted() {
        assert_eq!(
            usable(" https://api.example.com/ ").as_deref(),
            Some("https://api.example.com")
        );
        assert!(usable("http://127.0.0.1:1337").is_some());
        assert!(usable("http://api.example.com").is_none());
        assert!(usable("").is_none());
    }

    #[test]
    fn the_purchase_link_must_be_https() {
        assert_eq!(
            https(" https://example.com/buy ").as_deref(),
            Some("https://example.com/buy")
        );
        assert!(https("http://example.com/buy").is_none());
        assert!(https("https://").is_none());
        assert!(https("").is_none());
    }
}
