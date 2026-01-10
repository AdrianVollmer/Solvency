use serde::{Deserialize, Serialize};

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
