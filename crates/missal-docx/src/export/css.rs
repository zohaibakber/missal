use missal_core::document::Style;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color(pub u8, pub u8, pub u8);

impl Color {
    pub const BLACK: Color = Color(0, 0, 0);

    pub fn hex(self) -> String {
        format!("{:02X}{:02X}{:02X}", self.0, self.1, self.2)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Side {
    pub width_px: f64,
    pub style: &'static str,
    pub color: Color,
}

impl Side {
    pub fn solid(width_px: f64) -> Side {
        Side {
            width_px,
            style: "solid",
            color: Color::BLACK,
        }
    }
}

#[derive(Clone, Default)]
pub struct Css<'s> {
    layers: Vec<&'s Style>,
}

impl<'s> Css<'s> {
    pub fn of(style: Option<&'s Style>) -> Css<'s> {
        Css::default().with(style)
    }

    pub fn with(mut self, style: Option<&'s Style>) -> Css<'s> {
        if let Some(style) = style.filter(|style| !style.is_empty()) {
            self.layers.push(style);
        }
        self
    }

    pub fn get(&self, property: &str) -> Option<&'s str> {
        self.layers
            .iter()
            .rev()
            .find_map(|style| style.get(property))
            .map(clean)
            .filter(|value| !value.is_empty())
    }

    pub fn length(&self, property: &str, em: f64) -> Option<f64> {
        length(self.get(property)?, em)
    }

    pub fn color(&self, property: &str) -> Option<Color> {
        color(self.get(property)?)
    }

    pub fn font_size(&self) -> Option<f64> {
        self.length("font-size", 20.0).filter(|size| *size > 0.0)
    }

    pub fn family(&self) -> Option<String> {
        family(self.get("font-family")?)
    }

    pub fn bold(&self) -> Option<bool> {
        let value = self.get("font-weight")?.to_ascii_lowercase();
        match value.as_str() {
            "bold" | "bolder" => Some(true),
            "normal" | "lighter" => Some(false),
            other => other.parse::<f64>().ok().map(|weight| weight >= 600.0),
        }
    }

    pub fn italic(&self) -> Option<bool> {
        let value = self.get("font-style")?.to_ascii_lowercase();
        Some(matches!(value.as_str(), "italic" | "oblique"))
    }

    pub fn decoration(&self) -> Option<(bool, bool)> {
        let value = self
            .get("text-decoration-line")
            .or_else(|| self.get("text-decoration"))?
            .to_ascii_lowercase();
        Some((value.contains("underline"), value.contains("line-through")))
    }

    pub fn box_lengths(&self, property: &str) -> Option<[Option<f64>; 4]> {
        let values = four(self.get(property)?)?;
        Some(values.map(|value| length(value, 20.0)))
    }

    pub fn borders(&self) -> Option<[Option<Side>; 4]> {
        let widths = self.get("border-width").and_then(four);
        let styles = self.get("border-style").and_then(four);
        let colors = self.get("border-color").and_then(four);
        if widths.is_none() && styles.is_none() && colors.is_none() {
            return self.get("border").map(shorthand);
        }
        let mut sides = [None; 4];
        for (index, side) in sides.iter_mut().enumerate() {
            let style = styles.map_or("solid", |styles| styles[index]);
            let width = widths
                .and_then(|widths| length(widths[index], 20.0))
                .unwrap_or(1.0);
            let color = colors
                .and_then(|colors| color(colors[index]))
                .unwrap_or(Color::BLACK);
            *side = make_side(width, style, color);
        }
        Some(sides)
    }
}

fn make_side(width: f64, style: &str, color: Color) -> Option<Side> {
    let style = border_style(style)?;
    (width > 0.0).then_some(Side {
        width_px: width,
        style,
        color,
    })
}

fn shorthand(value: &str) -> [Option<Side>; 4] {
    let mut width = 1.0;
    let mut style = None;
    let mut chosen = Color::BLACK;
    for part in split_values(value) {
        if let Some(found) = length(part, 20.0) {
            width = found;
        } else if let Some(found) = color(part) {
            chosen = found;
        } else {
            style = Some(part);
        }
    }
    let side = style.and_then(|style| make_side(width, style, chosen));
    [side; 4]
}

fn border_style(value: &str) -> Option<&'static str> {
    Some(match value.to_ascii_lowercase().as_str() {
        "none" | "hidden" => return None,
        "double" => "double",
        "dotted" => "dotted",
        "dashed" => "dashed",
        "inset" => "inset",
        "outset" => "outset",
        "ridge" => "ridge",
        "groove" => "groove",
        _ => "solid",
    })
}

fn clean(value: &str) -> &str {
    let value = value.trim();
    value
        .strip_suffix("!important")
        .map_or(value, str::trim_end)
}

pub fn length(value: &str, em: f64) -> Option<f64> {
    let value = clean(value).to_ascii_lowercase();
    let split = value
        .find(|c: char| !(c.is_ascii_digit() || matches!(c, '.' | '-' | '+')))
        .unwrap_or(value.len());
    let (digits, unit) = value.split_at(split);
    let number: f64 = digits
        .parse()
        .ok()
        .filter(|number: &f64| number.is_finite())?;
    let factor = match unit {
        "px" => 1.0,
        "pt" => 96.0 / 72.0,
        "pc" => 16.0,
        "in" => 96.0,
        "cm" => 96.0 / 2.54,
        "mm" => 96.0 / 25.4,
        "em" => em,
        "rem" => 16.0,
        "" if number == 0.0 => 1.0,
        _ => return None,
    };
    Some(number * factor)
}

pub fn split_values(value: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0usize;
    let mut start = None;
    for (index, c) in value.char_indices() {
        match c {
            '(' => {
                depth += 1;
                start.get_or_insert(index);
            }
            ')' => depth = depth.saturating_sub(1),
            c if c.is_whitespace() && depth == 0 => {
                if let Some(from) = start.take() {
                    parts.push(&value[from..index]);
                }
            }
            _ => {
                start.get_or_insert(index);
            }
        }
    }
    if let Some(from) = start {
        parts.push(&value[from..]);
    }
    parts
}

pub fn four(value: &str) -> Option<[&str; 4]> {
    Some(match split_values(value).as_slice() {
        [a] => [a, a, a, a],
        [a, b] => [a, b, a, b],
        [a, b, c] => [a, b, c, b],
        [a, b, c, d] => [a, b, c, d],
        _ => return None,
    })
}

pub fn color(value: &str) -> Option<Color> {
    let value = clean(value).to_ascii_lowercase();
    if let Some(hex) = value.strip_prefix('#') {
        let channel = |text: &str| u8::from_str_radix(text, 16).ok();
        return match hex.len() {
            3 | 4 => {
                let digit = |at: usize| channel(&hex[at..at + 1]).map(|value| value * 17);
                Some(Color(digit(0)?, digit(1)?, digit(2)?))
            }
            6 | 8 => Some(Color(
                channel(&hex[0..2])?,
                channel(&hex[2..4])?,
                channel(&hex[4..6])?,
            )),
            _ => None,
        };
    }
    if let Some(inner) = value
        .strip_prefix("rgba(")
        .or_else(|| value.strip_prefix("rgb("))
        .and_then(|rest| rest.strip_suffix(')'))
    {
        let parts: Vec<&str> = inner
            .split([',', ' ', '/'])
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .collect();
        if parts.len() < 3 {
            return None;
        }
        if let Some(alpha) = parts.get(3) {
            let alpha = alpha.strip_suffix('%').map_or_else(
                || alpha.parse::<f64>().ok(),
                |percent| percent.parse::<f64>().ok().map(|value| value / 100.0),
            )?;
            if alpha <= 0.0 {
                return None;
            }
        }
        let channel = |part: &str| -> Option<u8> {
            let value = match part.strip_suffix('%') {
                Some(percent) => percent.parse::<f64>().ok()? * 2.55,
                None => part.parse::<f64>().ok()?,
            };
            Some(value.round().clamp(0.0, 255.0) as u8)
        };
        return Some(Color(
            channel(parts[0])?,
            channel(parts[1])?,
            channel(parts[2])?,
        ));
    }
    Some(match value.as_str() {
        "black" => Color(0, 0, 0),
        "white" => Color(255, 255, 255),
        "red" => Color(255, 0, 0),
        "green" => Color(0, 128, 0),
        "blue" => Color(0, 0, 255),
        "yellow" => Color(255, 255, 0),
        "gray" | "grey" => Color(128, 128, 128),
        "silver" => Color(192, 192, 192),
        "maroon" => Color(128, 0, 0),
        "navy" => Color(0, 0, 128),
        "orange" => Color(255, 165, 0),
        "purple" => Color(128, 0, 128),
        _ => return None,
    })
}

pub fn family(value: &str) -> Option<String> {
    let mut first = String::new();
    let mut quote = None;
    let mut escaped = false;
    for c in value.chars() {
        if escaped {
            first.push(c);
            escaped = false;
            continue;
        }
        match (c, quote) {
            ('\\', _) => escaped = true,
            ('"' | '\'', None) => quote = Some(c),
            (c, Some(open)) if c == open => quote = None,
            (',', None) => break,
            (c, _) => first.push(c),
        }
    }
    let name = first.split_whitespace().collect::<Vec<_>>().join(" ");
    let lower = name.to_ascii_lowercase();
    if name.is_empty()
        || matches!(
            lower.as_str(),
            "serif"
                | "sans-serif"
                | "monospace"
                | "cursive"
                | "fantasy"
                | "system-ui"
                | "ui-sans-serif"
        )
    {
        return None;
    }
    if lower == "missal jameel noori nastaleeq" {
        return Some("Jameel Noori Nastaleeq".to_owned());
    }
    Some(name)
}
