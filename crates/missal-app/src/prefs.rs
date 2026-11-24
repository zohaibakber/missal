use std::path::Path;

use missal_core::{FirId, FirStatus};
use serde_json::{Value, json};

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
    Accused,
}

impl Column {
    pub const ALL: [Column; 7] = [
        Column::FirNo,
        Column::Offence,
        Column::Accused,
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
            Column::Accused => "accused",
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
            Column::Accused => "ملزمان",
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TablePrefs {
    pub sort: Sort,
    pub status: Option<FirStatus>,
    pub hidden: Vec<Column>,
}

impl Default for TablePrefs {
    fn default() -> TablePrefs {
        TablePrefs {
            sort: Sort::default(),
            status: None,
            hidden: vec![Column::IncidentDate, Column::ArrestDate],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WindowPrefs {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub maximized: bool,
}

pub const MIN_WINDOW: (f32, f32) = (1024.0, 700.0);
pub const RECENT_FIRS: usize = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecentFir {
    pub id: FirId,
    pub opened_at: i64,
}
pub const DEFAULT_WINDOW: (f32, f32) = (1280.0, 800.0);

#[derive(Debug, Clone, PartialEq)]
pub struct Preferences {
    pub theme: ThemeChoice,
    pub window: Option<WindowPrefs>,
    pub table: TablePrefs,
    pub recent_firs: Vec<RecentFir>,
    pub sidebar_hidden: bool,
}

impl Default for Preferences {
    fn default() -> Preferences {
        Preferences {
            theme: ThemeChoice::System,
            window: None,
            table: TablePrefs::default(),
            recent_firs: Vec::new(),
            sidebar_hidden: false,
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

pub fn write_text(path: &Path, text: &str) -> std::io::Result<()> {
    let temporary = path.with_extension("json.tmp");
    std::fs::write(&temporary, text)?;
    std::fs::rename(&temporary, path)
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
        preferences.sidebar_hidden = value
            .get("sidebar_hidden")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if let Some(recent) = value.get("recent_firs").and_then(Value::as_array) {
            preferences.recent_firs = recent
                .iter()
                .filter_map(|entry| {
                    let id = FirId::new(entry.get("id")?.as_i64()?)?;
                    let opened_at = entry.get("opened_at")?.as_i64()?;
                    Some(RecentFir { id, opened_at })
                })
                .take(RECENT_FIRS)
                .collect();
        }
        if let Some(window) = value.get("window")
            && let (Some(x), Some(y), Some(width), Some(height)) = (
                number(window, "x"),
                number(window, "y"),
                number(window, "width"),
                number(window, "height"),
            )
            && width >= MIN_WINDOW.0
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
        if let Some(table) = value.get("fir_table") {
            if let Some(column) = table
                .get("sort_column")
                .and_then(Value::as_str)
                .and_then(Column::parse)
            {
                preferences.table.sort.column = column;
                preferences.table.sort.descending = table
                    .get("sort_descending")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
            }
            preferences.table.status = table
                .get("status")
                .and_then(Value::as_str)
                .and_then(FirStatus::parse);
            if let Some(hidden) = table.get("hidden_columns").and_then(Value::as_array) {
                preferences.table.hidden = hidden
                    .iter()
                    .filter_map(Value::as_str)
                    .filter_map(Column::parse)
                    .collect();
            }
        }
        preferences
    }

    pub fn to_json(&self) -> Value {
        let mut object = serde_json::Map::new();
        object.insert("theme".into(), json!(self.theme.as_str()));
        object.insert("sidebar_hidden".into(), json!(self.sidebar_hidden));
        object.insert(
            "recent_firs".into(),
            json!(
                self.recent_firs
                    .iter()
                    .map(|recent| json!({"id": recent.id.get(), "opened_at": recent.opened_at}))
                    .collect::<Vec<_>>()
            ),
        );
        if let Some(window) = self.window {
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
        object.insert(
            "fir_table".into(),
            json!({
                "sort_column": self.table.sort.column.key(),
                "sort_descending": self.table.sort.descending,
                "status": self.table.status.map(|status| status.as_str()),
                "hidden_columns": self.table.hidden.iter().map(|column| column.key()).collect::<Vec<_>>(),
            }),
        );
        Value::Object(object)
    }

    pub fn record_recent(&mut self, id: FirId, opened_at: i64) {
        self.recent_firs.retain(|recent| recent.id != id);
        self.recent_firs.insert(0, RecentFir { id, opened_at });
        self.recent_firs.truncate(RECENT_FIRS);
    }

    pub fn load(path: &Path) -> Preferences {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|text| serde_json::from_str::<Value>(&text).ok())
            .map_or_default(|value| Preferences::from_json(&value))
    }

    pub fn serialize(&self) -> std::io::Result<String> {
        serde_json::to_string_pretty(&self.to_json())
            .map_err(|error| std::io::Error::other(error.to_string()))
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        write_text(path, &self.serialize()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unusable_windows_and_garbage() {
        let preferences = Preferences::from_json(&json!({
            "theme": "purple",
            "window": {"x": 0, "y": 0, "width": 10, "height": 10},
            "fir_table": {"sort_column": "nope", "status": "Closed"}
        }));
        assert_eq!(preferences.theme, ThemeChoice::System);
        assert_eq!(preferences.window, None);
        assert_eq!(preferences.table.sort, Sort::default());
        assert_eq!(preferences.table.status, Some(FirStatus::Closed));
    }
}
