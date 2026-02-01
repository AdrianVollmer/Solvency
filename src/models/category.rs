use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Category {
    pub id: i64,
    pub name: String,
    pub parent_id: Option<i64>,
    pub color: String,
    pub icon: String,
    pub built_in: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryWithPath {
    #[serde(flatten)]
    pub category: Category,
    pub path: String,
    pub depth: i64,
}

impl CategoryWithPath {
    pub fn indent(&self) -> String {
        "  ".repeat(self.depth as usize)
    }

    pub fn display_name(&self) -> String {
        if self.depth > 0 {
            format!("{}└ {}", self.indent(), self.category.name)
        } else {
            self.category.name.clone()
        }
    }

    pub fn is_child_of(&self, parent_id: &i64) -> bool {
        self.category.parent_id == Some(*parent_id)
    }

    pub fn root_name(&self) -> &str {
        self.path.split(" > ").next().unwrap_or(&self.category.name)
    }

    pub fn indent_px(&self) -> i64 {
        self.depth * 20 + 12
    }

    pub fn parent_id_or_empty(&self) -> String {
        self.category
            .parent_id
            .map(|id| id.to_string())
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct NewCategory {
    pub name: String,
    pub parent_id: Option<i64>,
    #[serde(default = "default_color")]
    pub color: String,
    #[serde(default = "default_icon")]
    pub icon: String,
}

fn default_color() -> String {
    "#6b7280".to_string()
}

fn default_icon() -> String {
    "folder".to_string()
}
