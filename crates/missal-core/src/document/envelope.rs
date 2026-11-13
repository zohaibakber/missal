use std::collections::HashMap;
use std::fmt;

use serde_json::{Map, Value};

pub const DOCUMENT_FORMAT: &str = "missal-lexical";
pub const DOCUMENT_FORMAT_VERSION: u64 = 2;

const STYLE_KEYS: [&str; 3] = ["style", "textStyle", "importedStyle"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DocumentError {
    Json(String),
    Invalid(&'static str),
}

impl fmt::Display for DocumentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DocumentError::Json(message) => write!(f, "document is not valid JSON: {message}"),
            DocumentError::Invalid(reason) => write!(f, "document envelope is invalid: {reason}"),
        }
    }
}

impl std::error::Error for DocumentError {}

#[derive(Debug, Clone, PartialEq)]
pub struct PageLayout {
    pub width_mm: f64,
    pub height_mm: f64,
    pub margin_top_mm: f64,
    pub margin_right_mm: f64,
    pub margin_bottom_mm: f64,
    pub margin_left_mm: f64,
    pub(crate) extra: Map<String, Value>,
}

impl PageLayout {
    pub fn new(width_mm: f64, height_mm: f64, margins_mm: [f64; 4]) -> Option<PageLayout> {
        let [top, right, bottom, left] = margins_mm;
        let layout = PageLayout {
            width_mm,
            height_mm,
            margin_top_mm: top,
            margin_right_mm: right,
            margin_bottom_mm: bottom,
            margin_left_mm: left,
            extra: Map::new(),
        };
        layout.is_valid().then_some(layout)
    }

    pub fn a4() -> PageLayout {
        PageLayout::new(210.0, 297.0, [18.0; 4]).expect("A4 is a valid layout")
    }

    fn is_valid(&self) -> bool {
        self.width_mm > 0.0
            && self.height_mm > 0.0
            && [
                self.margin_top_mm,
                self.margin_right_mm,
                self.margin_bottom_mm,
                self.margin_left_mm,
            ]
            .iter()
            .all(|margin| *margin >= 0.0)
    }

    const KEYS: [&'static str; 6] = [
        "widthMm",
        "heightMm",
        "marginTopMm",
        "marginRightMm",
        "marginBottomMm",
        "marginLeftMm",
    ];

    fn from_json(value: &Value) -> Option<PageLayout> {
        let mut extra = value.as_object()?.clone();
        let mut numbers = [0.0; 6];
        for (slot, key) in numbers.iter_mut().zip(PageLayout::KEYS) {
            *slot = extra.remove(key)?.as_f64()?;
        }
        let [width_mm, height_mm, top, right, bottom, left] = numbers;
        let layout = PageLayout {
            width_mm,
            height_mm,
            margin_top_mm: top,
            margin_right_mm: right,
            margin_bottom_mm: bottom,
            margin_left_mm: left,
            extra,
        };
        layout.is_valid().then_some(layout)
    }

    fn to_json(&self) -> Value {
        let mut object = Map::new();
        let numbers = [
            self.width_mm,
            self.height_mm,
            self.margin_top_mm,
            self.margin_right_mm,
            self.margin_bottom_mm,
            self.margin_left_mm,
        ];
        for (key, number) in PageLayout::KEYS.into_iter().zip(numbers) {
            object.insert(key.to_owned(), number_value(number));
        }
        for (key, value) in &self.extra {
            object.entry(key.clone()).or_insert_with(|| value.clone());
        }
        Value::Object(object)
    }
}

pub(crate) fn number_value(number: f64) -> Value {
    if number.fract() == 0.0 && number.abs() < 9_007_199_254_740_992.0 {
        Value::from(number as i64)
    } else {
        serde_json::Number::from_f64(number).map_or(Value::Null, Value::Number)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Envelope {
    pub styles: Vec<String>,
    pub state: Value,
    pub page_layout: Option<PageLayout>,
    pub(crate) extra: Map<String, Value>,
}

impl Envelope {
    pub fn parse(json: &str) -> Result<Envelope, DocumentError> {
        let value: Value =
            serde_json::from_str(json).map_err(|error| DocumentError::Json(error.to_string()))?;
        Envelope::from_value(value)
    }

    pub fn from_value(value: Value) -> Result<Envelope, DocumentError> {
        let Value::Object(mut object) = value else {
            return Err(DocumentError::Invalid("not an object"));
        };
        if object.remove("format").as_ref().and_then(Value::as_str) != Some(DOCUMENT_FORMAT) {
            return Err(DocumentError::Invalid("unknown format"));
        }
        let version = object.remove("version").and_then(|version| version.as_f64());
        let page_layout = match object.remove("pageLayout") {
            None => None,
            Some(layout) => Some(
                PageLayout::from_json(&layout).ok_or(DocumentError::Invalid("invalid page layout"))?,
            ),
        };
        let state = object
            .remove("state")
            .ok_or(DocumentError::Invalid("missing state"))?;
        match version {
            Some(2.0) => {
                let styles = match object.remove("styles") {
                    Some(Value::Array(styles)) => styles
                        .into_iter()
                        .map(|style| match style {
                            Value::String(style) => Ok(style),
                            _ => Err(DocumentError::Invalid("style is not a string")),
                        })
                        .collect::<Result<Vec<_>, _>>()?,
                    _ => return Err(DocumentError::Invalid("missing styles")),
                };
                Ok(Envelope {
                    styles,
                    state,
                    page_layout,
                    extra: object,
                })
            }
            Some(1.0) => {
                let (styles, state) = normalize_styles(state);
                Ok(Envelope {
                    styles,
                    state,
                    page_layout,
                    extra: object,
                })
            }
            _ => Err(DocumentError::Invalid("unsupported version")),
        }
    }

    pub fn to_value(&self) -> Value {
        let mut object = Map::new();
        object.insert("format".into(), Value::from(DOCUMENT_FORMAT));
        object.insert("version".into(), Value::from(DOCUMENT_FORMAT_VERSION));
        object.insert(
            "styles".into(),
            Value::Array(self.styles.iter().cloned().map(Value::String).collect()),
        );
        object.insert("state".into(), self.state.clone());
        if let Some(layout) = &self.page_layout {
            object.insert("pageLayout".into(), layout.to_json());
        }
        for (key, value) in &self.extra {
            object.entry(key.clone()).or_insert_with(|| value.clone());
        }
        Value::Object(object)
    }

    pub fn to_json(&self) -> String {
        self.to_value().to_string()
    }

    pub fn expanded_state(&self) -> Value {
        let mut state = self.state.clone();
        expand_styles(&mut state, &self.styles);
        state
    }
}

fn map_styles(value: &mut Value, map: &mut dyn FnMut(&mut Value)) {
    match value {
        Value::Array(items) => items.iter_mut().for_each(|item| map_styles(item, map)),
        Value::Object(object) => {
            for (key, child) in object.iter_mut() {
                if STYLE_KEYS.contains(&key.as_str()) && (child.is_string() || child.is_number()) {
                    map(child);
                } else {
                    map_styles(child, map);
                }
            }
        }
        _ => {}
    }
}

pub fn normalize_styles(mut state: Value) -> (Vec<String>, Value) {
    let mut styles: Vec<String> = Vec::new();
    let mut indexes: HashMap<String, usize> = HashMap::new();
    map_styles(&mut state, &mut |style| {
        let Value::String(text) = style else { return };
        if text.is_empty() {
            return;
        }
        let index = *indexes.entry(text.clone()).or_insert_with(|| {
            styles.push(text.clone());
            styles.len() - 1
        });
        *style = Value::from(index);
    });
    (styles, state)
}

pub fn expand_styles(state: &mut Value, styles: &[String]) {
    map_styles(state, &mut |style| {
        if let Value::Number(number) = style {
            let text = number
                .as_u64()
                .and_then(|index| styles.get(usize::try_from(index).ok()?))
                .cloned()
                .unwrap_or_default();
            *style = Value::String(text);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn styles_normalise_and_expand() {
        let state = json!({"root": {"children": [
            {"type": "text", "style": "color: red", "text": "a"},
            {"type": "text", "style": "", "text": "b"},
            {"type": "paragraph", "textStyle": "color: red", "$": {"importedStyle": "margin: 0"}, "children": []}
        ]}});
        let (styles, normalised) = normalize_styles(state.clone());
        assert_eq!(styles, ["color: red", "margin: 0"]);
        assert_eq!(normalised["root"]["children"][0]["style"], 0);
        assert_eq!(normalised["root"]["children"][1]["style"], "");
        assert_eq!(normalised["root"]["children"][2]["$"]["importedStyle"], 1);
        let mut expanded = normalised;
        expand_styles(&mut expanded, &styles);
        assert_eq!(expanded, state);
        let mut out_of_range = json!({"style": 9});
        expand_styles(&mut out_of_range, &styles);
        assert_eq!(out_of_range, json!({"style": ""}));
    }

    #[test]
    fn version_one_is_upgraded() {
        let envelope = Envelope::parse(
            r#"{"format":"missal-lexical","version":1,"state":{"root":{"type":"root","children":[{"type":"text","text":"x","style":"color: red"}]}},"pageLayout":{"widthMm":210,"heightMm":297,"marginTopMm":18,"marginRightMm":18,"marginBottomMm":18,"marginLeftMm":18}}"#,
        )
        .unwrap();
        assert_eq!(envelope.styles, ["color: red"]);
        assert_eq!(envelope.state["root"]["children"][0]["style"], 0);
        let value = envelope.to_value();
        assert_eq!(value["version"], 2);
        assert_eq!(value["pageLayout"]["widthMm"], 210);
    }

    #[test]
    fn invalid_envelopes_are_rejected() {
        for json in [
            r#"{"format":"other","version":2,"styles":[],"state":{}}"#,
            r#"{"format":"missal-lexical","version":3,"styles":[],"state":{}}"#,
            r#"{"format":"missal-lexical","version":2,"state":{}}"#,
            r#"{"format":"missal-lexical","version":2,"styles":[],"state":{},"pageLayout":{"widthMm":0}}"#,
            "[",
        ] {
            assert!(Envelope::parse(json).is_err(), "{json}");
        }
    }
}
