use std::path::Path;

use missal_core::FirStatus;
use serde_json::{Map, Value, json};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemeChoice {
    #[default]
    System,
    Light,
    Dark,
}

impl ThemeChoice {
    pub const ALL: [ThemeChoice; 3] = [ThemeChoice::System, ThemeChoice::Light, ThemeChoice::Dark];

    pub const fn as_str(self) -> &'static str {
        match self {
            ThemeChoice::System => "system",
            ThemeChoice::Light => "light",
            ThemeChoice::Dark => "dark",
        }
    }

    pub const fn title(self) -> &'static str {
        match self {
            ThemeChoice::System => "System",
            ThemeChoice::Light => "Light",
            ThemeChoice::Dark => "Dark",
        }
    }

    pub fn parse(value: &str) -> Option<ThemeChoice> {
        ThemeChoice::ALL
            .into_iter()
            .find(|choice| choice.as_str() == value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Column {
    FirNo,
    Offence,
    Status,
    Date,
    IncidentDate,
    ArrestDate,
}

impl Column {
    pub const ALL: [Column; 6] = [
        Column::FirNo,
        Column::Offence,
        Column::Status,
        Column::Date,
        Column::IncidentDate,
        Column::ArrestDate,
    ];

    pub const fn key(self) -> &'static str {
        match self {
            Column::FirNo => "fir_no",
            Column::Offence => "offence",
            Column::Status => "status",
            Column::Date => "date",
            Column::IncidentDate => "incident_date",
            Column::ArrestDate => "arrest_date",
        }
    }

    pub const fn title(self) -> &'static str {
        match self {
            Column::FirNo => "ایف آئی آر نمبر",
            Column::Offence => "جرم",
            Column::Status => "حالت",
            Column::Date => "تاریخ ایف آئی آر",
            Column::IncidentDate => "تاریخ وقوعہ",
            Column::ArrestDate => "تاریخ گرفتاری",
        }
    }

    pub fn parse(value: &str) -> Option<Column> {
        Column::ALL.into_iter().find(|column| column.key() == value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sort {
    pub column: Column,
    pub descending: bool,
}

impl Default for Sort {
    fn default() -> Sort {
        Sort {
            column: Column::FirNo,
            descending: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TablePrefs {
    pub sort: Sort,
    pub statuses: Vec<FirStatus>,
    pub hidden: Vec<Column>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WindowPrefs {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub maximized: bool,
}

pub const MIN_WINDOW: (f32, f32) = (800.0, 600.0);
pub const DEFAULT_WINDOW: (f32, f32) = (1280.0, 800.0);

#[derive(Debug, Clone, PartialEq)]
pub struct Preferences {
    pub theme: ThemeChoice,
    pub sidebar_open: bool,
    pub window: Option<WindowPrefs>,
    pub table: TablePrefs,
}

impl Default for Preferences {
    fn default() -> Preferences {
        Preferences {
            theme: ThemeChoice::System,
            sidebar_open: true,
            window: None,
            table: TablePrefs::default(),
        }
    }
}

fn number(value: &Value, key: &str) -> Option<f32> {
    value
        .get(key)?
        .as_f64()
        .filter(|number| number.is_finite())
        .map(|number| number as f32)
}

impl Preferences {
    pub fn from_json(value: &Value) -> Preferences {
        let mut preferences = Preferences::default();
        if let Some(theme) = value
            .get("theme")
            .and_then(Value::as_str)
            .and_then(ThemeChoice::parse)
        {
            preferences.theme = theme;
        }
        if let Some(open) = value.get("sidebarOpen").and_then(Value::as_bool) {
            preferences.sidebar_open = open;
        }
        if let Some(window) = value.get("window") {
            if let (Some(x), Some(y), Some(width), Some(height)) = (
                number(window, "x"),
                number(window, "y"),
                number(window, "width"),
                number(window, "height"),
            ) && width >= MIN_WINDOW.0
                && height >= MIN_WINDOW.1
            {
                preferences.window = Some(WindowPrefs {
                    x,
                    y,
                    width,
                    height,
                    maximized: window
                        .get("maximized")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                });
            }
        }
        if let Some(table) = value.get("firTable") {
            if let Some(column) = table
                .get("sortColumn")
                .and_then(Value::as_str)
                .and_then(Column::parse)
            {
                preferences.table.sort.column = column;
                preferences.table.sort.descending = table
                    .get("sortDescending")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
            }
            if let Some(statuses) = table.get("statuses").and_then(Value::as_array) {
                preferences.table.statuses = statuses
                    .iter()
                    .filter_map(Value::as_str)
                    .filter_map(FirStatus::parse)
                    .collect();
            }
            if let Some(hidden) = table.get("hiddenColumns").and_then(Value::as_array) {
                preferences.table.hidden = hidden
                    .iter()
                    .filter_map(Value::as_str)
                    .filter_map(Column::parse)
                    .collect();
            }
        }
        preferences
    }

    pub fn merge_into(&self, object: &mut Map<String, Value>) {
        object.insert("theme".into(), json!(self.theme.as_str()));
        object.insert("sidebarOpen".into(), json!(self.sidebar_open));
        match self.window {
            Some(window) => {
                object.insert(
                    "window".into(),
                    json!({
                        "x": window.x,
                        "y": window.y,
                        "width": window.width,
                        "height": window.height,
                        "maximized": window.maximized,
                    }),
                );
            }
            None => {
                object.remove("window");
            }
        }
        object.insert(
            "firTable".into(),
            json!({
                "sortColumn": self.table.sort.column.key(),
                "sortDescending": self.table.sort.descending,
                "statuses": self.table.statuses.iter().map(|status| status.as_str()).collect::<Vec<_>>(),
                "hiddenColumns": self.table.hidden.iter().map(|column| column.key()).collect::<Vec<_>>(),
            }),
        );
    }

    pub fn load(path: &Path) -> Preferences {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|text| serde_json::from_str::<Value>(&text).ok())
            .map(|value| Preferences::from_json(&value))
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let mut object = std::fs::read_to_string(path)
            .ok()
            .and_then(|text| serde_json::from_str::<Value>(&text).ok())
            .and_then(|value| match value {
                Value::Object(object) => Some(object),
                _ => None,
            })
            .unwrap_or_default();
        self.merge_into(&mut object);
        let text = serde_json::to_string_pretty(&Value::Object(object))
            .map_err(|error| std::io::Error::other(error.to_string()))?;
        let temporary = path.with_extension("json.tmp");
        std::fs::write(&temporary, text)?;
        std::fs::rename(&temporary, path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_electron_preferences_file() {
        let preferences = Preferences::from_json(&json!({"theme": "dark"}));
        assert_eq!(preferences.theme, ThemeChoice::Dark);
        assert!(preferences.sidebar_open);
        assert_eq!(preferences.table, TablePrefs::default());
    }

    #[test]
    fn round_trips_and_keeps_unknown_keys() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("preferences.json");
        std::fs::write(&path, r#"{"theme":"light","other":1}"#).unwrap();
        let mut preferences = Preferences::load(&path);
        preferences.theme = ThemeChoice::Dark;
        preferences.sidebar_open = false;
        preferences.table.sort = Sort {
            column: Column::Date,
            descending: false,
        };
        preferences.table.statuses = vec![FirStatus::Closed];
        preferences.table.hidden = vec![Column::ArrestDate];
        preferences.window = Some(WindowPrefs {
            x: 10.0,
            y: 20.0,
            width: 1000.0,
            height: 700.0,
            maximized: true,
        });
        preferences.save(&path).unwrap();
        assert_eq!(Preferences::load(&path), preferences);
        let value: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(value["other"], 1);
        assert_eq!(value["theme"], "dark");
    }

    #[test]
    fn rejects_unusable_windows_and_garbage() {
        let preferences = Preferences::from_json(&json!({
            "theme": "purple",
            "window": {"x": 0, "y": 0, "width": 10, "height": 10},
            "firTable": {"sortColumn": "nope", "statuses": ["Closed", "x"]}
        }));
        assert_eq!(preferences.theme, ThemeChoice::System);
        assert_eq!(preferences.window, None);
        assert_eq!(preferences.table.sort, Sort::default());
        assert_eq!(preferences.table.statuses, vec![FirStatus::Closed]);
    }
}
