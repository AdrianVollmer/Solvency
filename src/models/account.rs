use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AccountType {
    Cash,
    Securities,
}

impl AccountType {
    pub fn as_str(&self) -> &'static str {
        match self {
            AccountType::Cash => "Cash",
            AccountType::Securities => "Securities",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "Cash" => Some(AccountType::Cash),
            "Securities" => Some(AccountType::Securities),
            _ => None,
        }
    }
}

impl std::fmt::Display for AccountType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub id: i64,
    pub name: String,
    pub account_type: AccountType,
    pub active: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NewAccount {
    pub name: String,
    pub account_type: AccountType,
    pub active: bool,
}
