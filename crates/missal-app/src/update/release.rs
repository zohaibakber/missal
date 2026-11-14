use semver::Version;
use serde_json::Value;

pub const REPOSITORY: &str = "zohaibakber/missal";
pub const TAG_PREFIX: &str = "native-v";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    pub tag: String,
    pub version: Version,
    pub assets: Vec<String>,
}

pub fn tag_version(tag: &str) -> Option<Version> {
    Version::parse(tag.strip_prefix(TAG_PREFIX)?).ok()
}

pub fn feed_name(channel: &str) -> String {
    format!("releases.{channel}.json")
}

pub fn releases_url(repository: &str) -> String {
    format!("https://api.github.com/repos/{repository}/releases?per_page=30")
}

pub fn download_base(repository: &str, tag: &str) -> String {
    format!("https://github.com/{repository}/releases/download/{tag}")
}

pub fn parse_releases(json: &str) -> Result<Vec<Release>, String> {
    let value: Value = serde_json::from_str(json).map_err(|error| format!("invalid release list: {error}"))?;
    let items = value.as_array().ok_or("release list is not an array")?;
    let mut releases = Vec::new();
    for item in items {
        let flag = |key: &str| item.get(key).and_then(Value::as_bool).unwrap_or(false);
        if flag("draft") || flag("prerelease") {
            continue;
        }
        let Some(tag) = item.get("tag_name").and_then(Value::as_str) else {
            continue;
        };
        let Some(version) = tag_version(tag) else {
            continue;
        };
        let assets = item
            .get("assets")
            .and_then(Value::as_array)
            .map(|assets| {
                assets
                    .iter()
                    .filter_map(|asset| asset.get("name").and_then(Value::as_str))
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default();
        releases.push(Release {
            tag: tag.to_owned(),
            version,
            assets,
        });
    }
    Ok(releases)
}

pub fn newest_update<'a>(releases: &'a [Release], current: &Version, channel: &str) -> Option<&'a Release> {
    let feed = feed_name(channel);
    releases
        .iter()
        .filter(|release| release.version > *current)
        .filter(|release| release.assets.iter().any(|asset| asset.eq_ignore_ascii_case(&feed)))
        .max_by(|a, b| a.version.cmp(&b.version))
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIST: &str = r#"[
        {"tag_name":"v1.9.0","draft":false,"prerelease":false,"assets":[{"name":"Missal-1.9.0-Setup.exe"},{"name":"latest.yml"}]},
        {"tag_name":"native-v0.3.0","draft":true,"prerelease":false,"assets":[{"name":"releases.win.json"}]},
        {"tag_name":"native-v0.3.0-beta.1","draft":false,"prerelease":true,"assets":[{"name":"releases.win.json"}]},
        {"tag_name":"native-v0.2.0","draft":false,"prerelease":false,"assets":[{"name":"releases.win.json"},{"name":"releases.linux.json"},{"name":"Missal-0.2.0-full.nupkg"}]},
        {"tag_name":"native-v0.10.0","draft":false,"prerelease":false,"assets":[{"name":"releases.linux.json"}]},
        {"tag_name":"native-v0.1.0","draft":false,"prerelease":false,"assets":[{"name":"releases.win.json"}]},
        {"tag_name":"native-vnext","draft":false,"prerelease":false,"assets":[]}
    ]"#;

    #[test]
    fn electron_tags_never_match() {
        assert_eq!(tag_version("v1.9.0"), None);
        assert_eq!(tag_version("native-v1.2.3"), Some(Version::new(1, 2, 3)));
        assert_eq!(tag_version("native-vnext"), None);
    }

    #[test]
    fn drafts_prereleases_and_foreign_tags_are_dropped() {
        let releases = parse_releases(LIST).unwrap();
        let tags: Vec<_> = releases.iter().map(|release| release.tag.as_str()).collect();
        assert_eq!(tags, ["native-v0.2.0", "native-v0.10.0", "native-v0.1.0"]);
    }

    #[test]
    fn newest_is_chosen_by_version_not_list_order() {
        let releases = parse_releases(LIST).unwrap();
        let linux = newest_update(&releases, &Version::new(0, 1, 0), "linux").unwrap();
        assert_eq!(linux.tag, "native-v0.10.0");
        let windows = newest_update(&releases, &Version::new(0, 1, 0), "win").unwrap();
        assert_eq!(windows.tag, "native-v0.2.0");
    }

    #[test]
    fn nothing_newer_means_no_update() {
        let releases = parse_releases(LIST).unwrap();
        assert!(newest_update(&releases, &Version::new(0, 2, 0), "win").is_none());
        assert!(newest_update(&releases, &Version::new(0, 10, 0), "linux").is_none());
        assert!(newest_update(&releases, &Version::new(0, 1, 0), "osx").is_none());
    }

    #[test]
    fn malformed_lists_are_errors() {
        assert!(parse_releases("{}").is_err());
        assert!(parse_releases("nope").is_err());
        assert_eq!(parse_releases("[]").unwrap(), vec![]);
    }

    #[test]
    fn urls_are_built_from_the_repository_and_tag() {
        assert_eq!(
            download_base(REPOSITORY, "native-v0.2.0"),
            "https://github.com/zohaibakber/missal/releases/download/native-v0.2.0"
        );
        assert!(releases_url(REPOSITORY).starts_with("https://api.github.com/repos/zohaibakber/missal/releases"));
        assert_eq!(feed_name("win"), "releases.win.json");
    }
}
