const CLIENT_ID_VARIABLE: &str = "WORKOS_CLIENT_ID";
const BASE_URL_VARIABLE: &str = "WORKOS_BASE_URL";

#[cfg(not(debug_assertions))]
const BUILT_CLIENT_ID: Option<&str> = Some(env!(
    "WORKOS_CLIENT_ID",
    "release builds need WORKOS_CLIENT_ID set to the WorkOS client ID"
));

#[cfg(not(debug_assertions))]
const _: () = assert!(
    !env!("WORKOS_CLIENT_ID").is_empty(),
    "release builds need a non-empty WORKOS_CLIENT_ID"
);

#[cfg(debug_assertions)]
const BUILT_CLIENT_ID: Option<&str> = option_env!("WORKOS_CLIENT_ID");

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub client_id: String,
    pub base_url: String,
}

fn non_empty(value: String) -> Option<String> {
    let value = value.trim().to_owned();
    (!value.is_empty()).then_some(value)
}

impl Config {
    pub fn resolve() -> Option<Config> {
        let client_id = std::env::var(CLIENT_ID_VARIABLE)
            .ok()
            .and_then(non_empty)
            .or_else(|| BUILT_CLIENT_ID.map(str::to_owned).and_then(non_empty))?;
        let base_url = std::env::var(BASE_URL_VARIABLE)
            .ok()
            .and_then(non_empty)
            .unwrap_or_else(|| workos::DEFAULT_BASE_URL.to_owned());
        Some(Config {
            client_id,
            base_url,
        })
    }
}
