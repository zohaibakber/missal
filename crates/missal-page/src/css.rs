use missal_text::Rgba;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Length {
    Px(f32),
    Em(f32),
    Rem(f32),
    Percent(f32),
    Auto,
}

impl Length {
    pub fn resolve(self, em: f32, percent_base: f32) -> f32 {
        match self {
            Length::Px(v) => v,
            Length::Em(v) => v * em,
            Length::Rem(v) => v * 16.0,
            Length::Percent(v) => v * percent_base / 100.0,
            Length::Auto => 0.0,
        }
    }

    pub fn resolve_absolute(self, em: f32) -> Option<f32> {
        match self {
            Length::Percent(_) | Length::Auto => None,
            other => Some(other.resolve(em, 0.0)),
        }
    }
}

pub fn clean(value: &str) -> &str {
    let value = value.trim();
    value
        .strip_suffix("!important")
        .map(str::trim_end)
        .unwrap_or(value)
}

pub fn unit_factor(unit: &str) -> Option<f32> {
    Some(match unit {
        "px" => 1.0,
        "pt" => 96.0 / 72.0,
        "pc" => 16.0,
        "in" => 96.0,
        "cm" => 96.0 / 2.54,
        "mm" => 96.0 / 25.4,
        "q" => 96.0 / 101.6,
        _ => return None,
    })
}

pub fn length(value: &str) -> Option<Length> {
    let value = clean(value).to_ascii_lowercase();
    if value == "auto" {
        return Some(Length::Auto);
    }
    let split = value
        .find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-' || c == '+'))
        .unwrap_or(value.len());
    let (number, unit) = value.split_at(split);
    let n: f32 = number.parse().ok()?;
    if !n.is_finite() {
        return None;
    }
    Some(match unit {
        "" if n == 0.0 => Length::Px(0.0),
        "" => return None,
        "%" => Length::Percent(n),
        "em" => Length::Em(n),
        "rem" => Length::Rem(n),
        "ex" | "ch" => Length::Em(n * 0.5),
        unit => Length::Px(n * unit_factor(unit)?),
    })
}

pub fn number(value: &str) -> Option<f32> {
    clean(value).parse().ok().filter(|n: &f32| n.is_finite())
}

pub fn split_values(value: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0usize;
    let mut start = None;
    for (i, c) in value.char_indices() {
        match c {
            '(' => {
                depth += 1;
                start.get_or_insert(i);
            }
            ')' => depth = depth.saturating_sub(1),
            c if c.is_whitespace() && depth == 0 => {
                if let Some(s) = start.take() {
                    parts.push(&value[s..i]);
                }
            }
            _ => {
                start.get_or_insert(i);
            }
        }
    }
    if let Some(s) = start {
        parts.push(&value[s..]);
    }
    parts
}

pub fn box_values<T: Copy>(value: &str, parse: impl Fn(&str) -> Option<T>) -> Option<[T; 4]> {
    let parts: Vec<T> = split_values(clean(value))
        .into_iter()
        .map(&parse)
        .collect::<Option<Vec<_>>>()?;
    Some(match parts.as_slice() {
        [a] => [*a, *a, *a, *a],
        [a, b] => [*a, *b, *a, *b],
        [a, b, c] => [*a, *b, *c, *b],
        [a, b, c, d] => [*a, *b, *c, *d],
        _ => return None,
    })
}

fn hex_digit(c: u8) -> Option<u32> {
    (c as char).to_digit(16)
}

fn channel(value: &str, scale: f32) -> Option<f32> {
    let value = value.trim();
    if let Some(p) = value.strip_suffix('%') {
        Some(p.trim().parse::<f32>().ok()? / 100.0)
    } else {
        Some(value.parse::<f32>().ok()? / scale)
    }
}

pub fn color(value: &str) -> Option<Rgba> {
    let value = clean(value).to_ascii_lowercase();
    if let Some(hex) = value.strip_prefix('#') {
        let bytes = hex.as_bytes();
        let digits: Option<Vec<u32>> = bytes.iter().map(|b| hex_digit(*b)).collect();
        let d = digits?;
        let (r, g, b, a) = match d.len() {
            3 => (d[0] * 17, d[1] * 17, d[2] * 17, 255),
            4 => (d[0] * 17, d[1] * 17, d[2] * 17, d[3] * 17),
            6 => (d[0] * 16 + d[1], d[2] * 16 + d[3], d[4] * 16 + d[5], 255),
            8 => (
                d[0] * 16 + d[1],
                d[2] * 16 + d[3],
                d[4] * 16 + d[5],
                d[6] * 16 + d[7],
            ),
            _ => return None,
        };
        return Some(Rgba((r << 24) | (g << 16) | (b << 8) | a));
    }
    if let Some(args) = value
        .strip_prefix("rgba(")
        .or_else(|| value.strip_prefix("rgb("))
        .and_then(|rest| rest.strip_suffix(')'))
    {
        let (rgb, alpha) = match args.split_once('/') {
            Some((rgb, alpha)) => (rgb, Some(alpha)),
            None => (args, None),
        };
        let parts: Vec<&str> = if rgb.contains(',') {
            rgb.split(',').collect()
        } else {
            rgb.split_whitespace().collect()
        };
        let (rgb_parts, alpha) = match (parts.len(), alpha) {
            (3, alpha) => (&parts[..3], alpha),
            (4, None) => (&parts[..3], Some(parts[3])),
            _ => return None,
        };
        let r = channel(rgb_parts[0], 255.0)?;
        let g = channel(rgb_parts[1], 255.0)?;
        let b = channel(rgb_parts[2], 255.0)?;
        let a = match alpha {
            Some(a) => channel(a, 1.0)?,
            None => 1.0,
        };
        return Some(Rgba::from_f32(r, g, b, a));
    }
    Some(match value.as_str() {
        "transparent" => Rgba::TRANSPARENT,
        "black" | "windowtext" => Rgba::rgb(0x000000),
        "white" | "window" => Rgba::rgb(0xffffff),
        "red" => Rgba::rgb(0xff0000),
        "green" => Rgba::rgb(0x008000),
        "blue" => Rgba::rgb(0x0000ff),
        "yellow" => Rgba::rgb(0xffff00),
        "gray" | "grey" => Rgba::rgb(0x808080),
        "silver" => Rgba::rgb(0xc0c0c0),
        "maroon" => Rgba::rgb(0x800000),
        "navy" => Rgba::rgb(0x000080),
        "purple" => Rgba::rgb(0x800080),
        "teal" => Rgba::rgb(0x008080),
        "olive" => Rgba::rgb(0x808000),
        "lime" => Rgba::rgb(0x00ff00),
        "aqua" | "cyan" => Rgba::rgb(0x00ffff),
        "fuchsia" | "magenta" => Rgba::rgb(0xff00ff),
        "orange" => Rgba::rgb(0xffa500),
        "darkgray" | "darkgrey" => Rgba::rgb(0xa9a9a9),
        "lightgray" | "lightgrey" => Rgba::rgb(0xd3d3d3),
        "darkblue" => Rgba::rgb(0x00008b),
        "darkred" => Rgba::rgb(0x8b0000),
        "darkgreen" => Rgba::rgb(0x006400),
        _ => return None,
    })
}

pub fn first_family(value: &str) -> Option<String> {
    let first = clean(value).split(',').next()?.trim();
    let unquoted = first.trim_matches(|c| c == '"' || c == '\'').trim();
    (!unquoted.is_empty()).then(|| unquoted.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lengths() {
        assert_eq!(length("16px"), Some(Length::Px(16.0)));
        assert_eq!(length("12pt"), Some(Length::Px(16.0)));
        assert_eq!(length("-6px"), Some(Length::Px(-6.0)));
        assert_eq!(length("2em"), Some(Length::Em(2.0)));
        assert_eq!(length("1.5rem"), Some(Length::Rem(1.5)));
        assert_eq!(length("50%"), Some(Length::Percent(50.0)));
        assert_eq!(length("0"), Some(Length::Px(0.0)));
        assert_eq!(length("auto"), Some(Length::Auto));
        assert_eq!(length("1ex"), Some(Length::Em(0.5)));
        assert!(length("12").is_none());
        let mm = length("25.4mm").unwrap().resolve(0.0, 0.0);
        assert!((mm - 96.0).abs() < 1e-3);
    }

    #[test]
    fn colors() {
        assert_eq!(color("rgb(47, 84, 150)"), Some(Rgba::rgb(0x2f5496)));
        assert_eq!(color("#fff"), Some(Rgba::rgb(0xffffff)));
        assert_eq!(color("#2F5496"), Some(Rgba::rgb(0x2f5496)));
        assert_eq!(color("rgba(0, 0, 0, 0)"), Some(Rgba(0)));
        assert_eq!(color("rgb(0 0 0 / 50%)").map(|c| c.alpha()), Some(128));
        assert_eq!(color("black"), Some(Rgba::BLACK));
    }

    #[test]
    fn boxes() {
        assert_eq!(
            box_values("0px -22.2px 0px -42px", length),
            Some([
                Length::Px(0.0),
                Length::Px(-22.2),
                Length::Px(0.0),
                Length::Px(-42.0)
            ])
        );
        assert_eq!(
            box_values("5px 0px 10px", length).map(|b| b[3]),
            Some(Length::Px(0.0))
        );
        assert_eq!(
            split_values("1px solid rgb(0, 0, 0)"),
            vec!["1px", "solid", "rgb(0, 0, 0)"]
        );
    }
}
