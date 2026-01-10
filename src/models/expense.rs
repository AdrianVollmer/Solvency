use crate::models::tag::Tag;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Expense {
    pub id: i64,
    pub date: String,
    pub amount_cents: i64,
    pub currency: String,
    pub description: String,
    pub category_id: Option<i64>,
    pub notes: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl Expense {
    pub fn amount_display(&self) -> String {
        let is_negative = self.amount_cents < 0;
        let abs_cents = self.amount_cents.abs();
        let dollars = abs_cents / 100;
        let cents = abs_cents % 100;
        if is_negative {
            format!("-{}.{:02}", dollars, cents)
        } else {
            format!("{}.{:02}", dollars, cents)
        }
    }

    pub fn amount_formatted(&self) -> String {
        let symbol = currency_symbol(&self.currency);
        format!("{}{}", symbol, self.amount_display())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpenseWithRelations {
    #[serde(flatten)]
    pub expense: Expense,
    pub category_name: Option<String>,
    pub category_color: Option<String>,
    pub tags: Vec<Tag>,
}

impl ExpenseWithRelations {
    pub fn amount_display(&self) -> String {
        self.expense.amount_display()
    }

    pub fn amount_formatted(&self) -> String {
        self.expense.amount_formatted()
    }

    pub fn category_color_or_default(&self) -> &str {
        self.category_color.as_deref().unwrap_or("#6b7280")
    }

    pub fn category_name_or_default(&self) -> &str {
        self.category_name.as_deref().unwrap_or("Uncategorized")
    }

    pub fn category_initial(&self) -> char {
        self.category_name
            .as_ref()
            .and_then(|n| n.chars().next())
            .unwrap_or('?')
    }

    pub fn has_category(&self) -> bool {
        self.category_name.is_some()
    }

    pub fn has_notes(&self) -> bool {
        self.expense.notes.is_some()
    }

    pub fn notes_text(&self) -> &str {
        self.expense.notes.as_deref().unwrap_or("")
    }

    pub fn is_currency(&self, currency: &str) -> bool {
        self.expense.currency == currency
    }

    pub fn matches_category(&self, id: &i64) -> bool {
        self.expense.category_id == Some(*id)
    }

    pub fn has_tag(&self, id: &i64) -> bool {
        self.tags.iter().any(|t| t.id == *id)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct NewExpense {
    pub date: String,
    pub amount_cents: i64,
    pub currency: String,
    pub description: String,
    pub category_id: Option<i64>,
    pub notes: Option<String>,
    #[serde(default)]
    pub tag_ids: Vec<i64>,
}

impl NewExpense {
    pub fn from_decimal(amount: f64) -> i64 {
        (amount * 100.0).round() as i64
    }
}

fn currency_symbol(currency: &str) -> &'static str {
    match currency.to_uppercase().as_str() {
        "USD" => "$",
        "EUR" => "€",
        "GBP" => "£",
        "JPY" => "¥",
        "CNY" => "¥",
        "CAD" => "C$",
        "AUD" => "A$",
        "CHF" => "CHF ",
        "INR" => "₹",
        "BRL" => "R$",
        "MXN" => "MX$",
        _ => "$",
    }
}
