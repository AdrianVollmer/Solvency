use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleActionType {
    AssignCategory,
    AssignTag,
}

impl RuleActionType {
    pub fn as_str(&self) -> &'static str {
        match self {
            RuleActionType::AssignCategory => "assign_category",
            RuleActionType::AssignTag => "assign_tag",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "assign_category" => Some(RuleActionType::AssignCategory),
            "assign_tag" => Some(RuleActionType::AssignTag),
            _ => None,
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            RuleActionType::AssignCategory => "Assign Category",
            RuleActionType::AssignTag => "Assign Tag",
        }
    }
}

impl std::fmt::Display for RuleActionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    pub id: i64,
    pub name: String,
    pub pattern: String,
    pub action_type: RuleActionType,
    pub action_value: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NewRule {
    pub name: String,
    pub pattern: String,
    pub action_type: RuleActionType,
    pub action_value: String,
}
