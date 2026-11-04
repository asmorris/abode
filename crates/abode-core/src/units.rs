//! Units. Everything is stored in metres; this module formats and parses
//! human input in either metric or imperial.

use serde::{Deserialize, Serialize};

pub const M_PER_FT: f64 = 0.3048;
pub const M_PER_IN: f64 = 0.0254;
pub const M2_PER_FT2: f64 = 0.09290304;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnitSystem {
    #[default]
    Metric,
    Imperial,
}

impl UnitSystem {
    pub const ALL: [UnitSystem; 2] = [UnitSystem::Metric, UnitSystem::Imperial];

    pub fn label(self) -> &'static str {
        match self {
            UnitSystem::Metric => "Metric (m)",
            UnitSystem::Imperial => "Imperial (ft/in)",
        }
    }

    /// Candidate grid spacings, smallest first.
    pub fn grid_steps(self) -> &'static [f64] {
        match self {
            UnitSystem::Metric => &[0.01, 0.05, 0.1, 0.5, 1.0, 5.0, 10.0, 50.0],
            UnitSystem::Imperial => &[
                M_PER_IN,
                3.0 * M_PER_IN,
                6.0 * M_PER_IN,
                M_PER_FT,
                5.0 * M_PER_FT,
                10.0 * M_PER_FT,
                50.0 * M_PER_FT,
            ],
        }
    }

    pub fn area_unit(self) -> &'static str {
        match self {
            UnitSystem::Metric => "m²",
            UnitSystem::Imperial => "ft²",
        }
    }

    pub fn area_to_display(self, m2: f64) -> f64 {
        match self {
            UnitSystem::Metric => m2,
            UnitSystem::Imperial => m2 / M2_PER_FT2,
        }
    }

    pub fn area_from_display(self, v: f64) -> f64 {
        match self {
            UnitSystem::Metric => v,
            UnitSystem::Imperial => v * M2_PER_FT2,
        }
    }
}

pub fn format_length(m: f64, units: UnitSystem) -> String {
    match units {
        UnitSystem::Metric => {
            if m.abs() < 0.995 {
                format!("{:.0} cm", m * 100.0)
            } else {
                format!("{:.2} m", m)
            }
        }
        UnitSystem::Imperial => {
            let quarters = (m.abs() / M_PER_IN * 4.0).round() as i64;
            let feet = quarters / 48;
            let rem = quarters % 48;
            let inches = rem / 4;
            let frac = ["", "¼", "½", "¾"][(rem % 4) as usize];
            let s = if feet > 0 {
                format!("{feet}' {inches}{frac}\"")
            } else {
                format!("{inches}{frac}\"")
            };
            if m < 0.0 && quarters > 0 {
                format!("-{s}")
            } else {
                s
            }
        }
    }
}

pub fn format_area(m2: f64, units: UnitSystem) -> String {
    match units {
        UnitSystem::Metric => format!("{:.1} m²", m2),
        UnitSystem::Imperial => format!("{:.0} ft²", m2 / M2_PER_FT2),
    }
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ParseLengthError {
    #[error("enter a length, e.g. 3.5 m or 12' 6\"")]
    Empty,
    #[error("couldn't understand “{0}”")]
    Invalid(String),
    #[error("unknown unit “{0}”")]
    UnknownUnit(String),
}

enum Tok {
    Num(f64, bool), // value, is_fraction
    Unit(String),
}

fn unit_factor(u: &str) -> Option<f64> {
    Some(match u {
        "m" | "meter" | "meters" | "metre" | "metres" => 1.0,
        "cm" => 0.01,
        "mm" => 0.001,
        "'" | "ft" | "foot" | "feet" => M_PER_FT,
        "\"" | "in" | "inch" | "inches" => M_PER_IN,
        _ => return None,
    })
}

/// Parse a length typed by a person. Accepts things like `3.5`, `3.5m`,
/// `350 cm`, `12'`, `12' 6"`, `12ft 6in`, `12'6`, `6 1/2"`, `6½"`.
///
/// A bare number uses `default_units` (metres or feet). A bare number after
/// feet is read as inches, so `12 6` in imperial is 12' 6".
pub fn parse_length(input: &str, default_units: UnitSystem) -> Result<f64, ParseLengthError> {
    let s = input
        .trim()
        .to_lowercase()
        .replace('½', " 1/2")
        .replace('¼', " 1/4")
        .replace('¾', " 3/4")
        .replace(['’', '′', '‘'], "'")
        .replace(['”', '″', '“'], "\"")
        .replace("''", "\"");
    if s.is_empty() {
        return Err(ParseLengthError::Empty);
    }
    let invalid = || ParseLengthError::Invalid(input.trim().to_string());

    let chars: Vec<char> = s.chars().collect();
    let mut toks = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() || c == ',' || c == '-' {
            i += 1;
        } else if c.is_ascii_digit() || c == '.' {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.' || chars[i] == '/') {
                i += 1;
            }
            let t: String = chars[start..i].iter().collect();
            if let Some((a, b)) = t.split_once('/') {
                let a: f64 = a.parse().map_err(|_| invalid())?;
                let b: f64 = b.parse().map_err(|_| invalid())?;
                if b == 0.0 {
                    return Err(invalid());
                }
                toks.push(Tok::Num(a / b, true));
            } else {
                toks.push(Tok::Num(t.parse().map_err(|_| invalid())?, false));
            }
        } else if c.is_alphabetic() {
            let start = i;
            while i < chars.len() && chars[i].is_alphabetic() {
                i += 1;
            }
            toks.push(Tok::Unit(chars[start..i].iter().collect()));
        } else if c == '\'' || c == '"' {
            toks.push(Tok::Unit(c.to_string()));
            i += 1;
        } else {
            return Err(invalid());
        }
    }

    let mut total = 0.0;
    let mut saw_feet = false;
    let mut groups = 0;
    let mut idx = 0;
    while idx < toks.len() {
        let Tok::Num(mut v, _) = toks[idx] else {
            return Err(invalid());
        };
        idx += 1;
        if let Some(Tok::Num(f, true)) = toks.get(idx) {
            v += *f;
            idx += 1;
        }
        let factor = match toks.get(idx) {
            Some(Tok::Unit(u)) => {
                idx += 1;
                unit_factor(u).ok_or_else(|| ParseLengthError::UnknownUnit(u.clone()))?
            }
            _ if saw_feet => M_PER_IN,
            _ => match default_units {
                UnitSystem::Metric => 1.0,
                UnitSystem::Imperial => M_PER_FT,
            },
        };
        if factor == M_PER_FT {
            saw_feet = true;
        }
        total += v * factor;
        groups += 1;
    }
    if groups == 0 {
        return Err(ParseLengthError::Empty);
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use UnitSystem::*;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn parses_metric() {
        assert!(close(parse_length("3.5", Metric).unwrap(), 3.5));
        assert!(close(parse_length("3.5m", Metric).unwrap(), 3.5));
        assert!(close(parse_length("350 cm", Metric).unwrap(), 3.5));
        assert!(close(parse_length("3500mm", Imperial).unwrap(), 3.5));
    }

    #[test]
    fn parses_imperial() {
        let ft12_6 = 12.0 * M_PER_FT + 6.0 * M_PER_IN;
        assert!(close(parse_length("12'", Metric).unwrap(), 12.0 * M_PER_FT));
        assert!(close(parse_length("12' 6\"", Metric).unwrap(), ft12_6));
        assert!(close(parse_length("12ft 6in", Metric).unwrap(), ft12_6));
        assert!(close(parse_length("12'6", Metric).unwrap(), ft12_6));
        assert!(close(parse_length("12 6", Imperial).unwrap(), ft12_6));
        assert!(close(parse_length("12", Imperial).unwrap(), 12.0 * M_PER_FT));
        assert!(close(parse_length("6 1/2\"", Metric).unwrap(), 6.5 * M_PER_IN));
        assert!(close(parse_length("6½\"", Metric).unwrap(), 6.5 * M_PER_IN));
        assert!(close(parse_length("12’ 6”", Metric).unwrap(), ft12_6));
    }

    #[test]
    fn rejects_garbage() {
        assert!(parse_length("", Metric).is_err());
        assert!(parse_length("abc", Metric).is_err());
        assert!(parse_length("3 furlongs", Metric).is_err());
        assert!(parse_length("1/0", Metric).is_err());
    }

    #[test]
    fn formats_round_trip() {
        for m in [0.5, 1.0, 2.37, 3.81, 10.0] {
            for u in UnitSystem::ALL {
                let s = format_length(m, u);
                let back = parse_length(&s, u).unwrap();
                let tol = if u == Imperial { M_PER_IN / 8.0 } else { 0.006 };
                assert!((back - m).abs() < tol, "{m} -> {s} -> {back}");
            }
        }
        assert_eq!(format_length(12.0 * M_PER_FT + 6.5 * M_PER_IN, Imperial), "12' 6½\"");
    }
}
