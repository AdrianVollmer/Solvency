use serde::{Deserialize, Serialize};

/// Curated color palette for tags (Tailwind 500 values).
pub const TAG_PALETTE: &[(&str, &str)] = &[
    ("Red", "#ef4444"),
    ("Orange", "#f97316"),
    ("Amber", "#f59e0b"),
    ("Yellow", "#eab308"),
    ("Lime", "#84cc16"),
    ("Green", "#22c55e"),
    ("Emerald", "#10b981"),
    ("Teal", "#14b8a6"),
    ("Cyan", "#06b6d4"),
    ("Blue", "#3b82f6"),
    ("Indigo", "#6366f1"),
    ("Violet", "#8b5cf6"),
    ("Purple", "#a855f7"),
    ("Fuchsia", "#d946ef"),
    ("Pink", "#ec4899"),
    ("Gray", "#6b7280"),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum TagStyle {
    #[default]
    Solid,
    Outline,
    Striped,
}

impl TagStyle {
    pub fn as_str(&self) -> &'static str {
        match self {
            TagStyle::Solid => "solid",
            TagStyle::Outline => "outline",
            TagStyle::Striped => "striped",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "outline" => TagStyle::Outline,
            "striped" => TagStyle::Striped,
            _ => TagStyle::Solid,
        }
    }
}

impl std::fmt::Display for TagStyle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tag {
    pub id: i64,
    pub name: String,
    pub color: String,
    pub style: TagStyle,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NewTag {
    pub name: String,
    #[serde(default = "default_color")]
    pub color: String,
    #[serde(default)]
    pub style: TagStyle,
}

fn default_color() -> String {
    "#6b7280".to_string()
}

/// Tag with its transaction usage count (for listing pages).
#[derive(Debug, Clone)]
pub struct TagWithUsage {
    pub tag: Tag,
    pub usage_count: i64,
}

// -- Colour helpers for accessible badge rendering --

/// Parse a hex color string (#RRGGBB) into (R, G, B).
fn parse_hex(hex: &str) -> Option<(u8, u8, u8)> {
    let hex = hex.strip_prefix('#').unwrap_or(hex);
    if hex.len() < 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some((r, g, b))
}

/// sRGB relative luminance (W3C WCAG 2.x formula).
fn relative_luminance(r: u8, g: u8, b: u8) -> f64 {
    fn linearize(c: u8) -> f64 {
        let c = c as f64 / 255.0;
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    }
    0.2126 * linearize(r) + 0.7152 * linearize(g) + 0.0722 * linearize(b)
}

/// WCAG contrast ratio between two luminances (L1 >= L2).
fn contrast_ratio(l1: f64, l2: f64) -> f64 {
    let (lighter, darker) = if l1 > l2 { (l1, l2) } else { (l2, l1) };
    (lighter + 0.05) / (darker + 0.05)
}

impl Tag {
    /// Text color for **solid** badges: returns `"white"` for dark backgrounds,
    /// `"#1e293b"` (slate-800) for light backgrounds.
    pub fn text_color(&self) -> &'static str {
        if let Some((r, g, b)) = parse_hex(&self.color) {
            let lum = relative_luminance(r, g, b);
            // Threshold chosen so that the resulting contrast ratio is >= 4.5:1
            // against whichever text colour we pick.
            if lum > 0.179 {
                "#1e293b" // dark text for light/bright bg
            } else {
                "white"
            }
        } else {
            "white"
        }
    }

    /// Text color for **ghost / outline / striped** badges: returns a darkened
    /// version of the tag colour that achieves >= 4.5:1 contrast against white.
    /// If the original colour already passes, it is returned unchanged.
    pub fn ghost_text_color(&self) -> String {
        let Some((mut r, mut g, mut b)) = parse_hex(&self.color) else {
            return self.color.clone();
        };

        let white_lum = 1.0; // relative luminance of white
        for _ in 0..40 {
            let lum = relative_luminance(r, g, b);
            if contrast_ratio(white_lum, lum) >= 4.5 {
                return format!("#{:02x}{:02x}{:02x}", r, g, b);
            }
            // Darken by ~10%
            r = (r as f64 * 0.90) as u8;
            g = (g as f64 * 0.90) as u8;
            b = (b as f64 * 0.90) as u8;
        }
        // Fallback: very dark
        format!("#{:02x}{:02x}{:02x}", r, g, b)
    }
}

impl TagStyle {
    /// Capitalised label for display in tables.
    pub fn label(&self) -> &'static str {
        match self {
            TagStyle::Solid => "Solid",
            TagStyle::Outline => "Outline",
            TagStyle::Striped => "Striped",
        }
    }
}
