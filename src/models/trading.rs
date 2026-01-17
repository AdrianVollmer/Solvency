use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// Trading activity types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TradingActivityType {
    Buy,
    Sell,
    Dividend,
    Interest,
    Deposit,
    Withdrawal,
    AddHolding,
    RemoveHolding,
    TransferIn,
    TransferOut,
    Fee,
    Tax,
    Split,
}

impl TradingActivityType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Buy => "BUY",
            Self::Sell => "SELL",
            Self::Dividend => "DIVIDEND",
            Self::Interest => "INTEREST",
            Self::Deposit => "DEPOSIT",
            Self::Withdrawal => "WITHDRAWAL",
            Self::AddHolding => "ADD_HOLDING",
            Self::RemoveHolding => "REMOVE_HOLDING",
            Self::TransferIn => "TRANSFER_IN",
            Self::TransferOut => "TRANSFER_OUT",
            Self::Fee => "FEE",
            Self::Tax => "TAX",
            Self::Split => "SPLIT",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Buy => "Buy",
            Self::Sell => "Sell",
            Self::Dividend => "Dividend",
            Self::Interest => "Interest",
            Self::Deposit => "Deposit",
            Self::Withdrawal => "Withdrawal",
            Self::AddHolding => "Add Holding",
            Self::RemoveHolding => "Remove Holding",
            Self::TransferIn => "Transfer In",
            Self::TransferOut => "Transfer Out",
            Self::Fee => "Fee",
            Self::Tax => "Tax",
            Self::Split => "Split",
        }
    }

    pub fn all() -> &'static [TradingActivityType] {
        &[
            Self::Buy,
            Self::Sell,
            Self::Dividend,
            Self::Interest,
            Self::Deposit,
            Self::Withdrawal,
            Self::AddHolding,
            Self::RemoveHolding,
            Self::TransferIn,
            Self::TransferOut,
            Self::Fee,
            Self::Tax,
            Self::Split,
        ]
    }

    /// Returns true if this activity type affects cash balance
    pub fn affects_cash(&self) -> bool {
        !matches!(self, Self::Split)
    }

    /// Returns true if this activity type affects holdings
    pub fn affects_holdings(&self) -> bool {
        matches!(
            self,
            Self::Buy
                | Self::Sell
                | Self::AddHolding
                | Self::RemoveHolding
                | Self::TransferIn
                | Self::TransferOut
                | Self::Split
        )
    }

    /// Returns true if this is a cash-only activity (uses $CASH-<CURRENCY> symbol)
    pub fn is_cash_only(&self) -> bool {
        matches!(self, Self::Interest | Self::Deposit | Self::Withdrawal)
    }
}

impl fmt::Display for TradingActivityType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for TradingActivityType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "BUY" => Ok(Self::Buy),
            "SELL" => Ok(Self::Sell),
            "DIVIDEND" => Ok(Self::Dividend),
            "INTEREST" => Ok(Self::Interest),
            "DEPOSIT" => Ok(Self::Deposit),
            "WITHDRAWAL" => Ok(Self::Withdrawal),
            "ADD_HOLDING" => Ok(Self::AddHolding),
            "REMOVE_HOLDING" => Ok(Self::RemoveHolding),
            "TRANSFER_IN" => Ok(Self::TransferIn),
            "TRANSFER_OUT" => Ok(Self::TransferOut),
            "FEE" => Ok(Self::Fee),
            "TAX" => Ok(Self::Tax),
            "SPLIT" => Ok(Self::Split),
            _ => Err(format!("Unknown activity type: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradingActivity {
    pub id: i64,
    pub date: String,
    pub symbol: String,
    pub quantity: Option<f64>,
    pub activity_type: TradingActivityType,
    pub unit_price_cents: Option<i64>,
    pub currency: String,
    pub fee_cents: i64,
    pub notes: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl TradingActivity {
    pub fn unit_price_display(&self) -> Option<String> {
        self.unit_price_cents.map(|cents| {
            let dollars = cents / 100;
            let remainder = cents % 100;
            format!("{}.{:02}", dollars, remainder)
        })
    }

    pub fn unit_price_formatted(&self) -> Option<String> {
        self.unit_price_display()
            .map(|price| format!("{}{}", currency_symbol(&self.currency), price))
    }

    pub fn fee_display(&self) -> String {
        let dollars = self.fee_cents / 100;
        let cents = self.fee_cents % 100;
        format!("{}.{:02}", dollars, cents)
    }

    pub fn fee_formatted(&self) -> String {
        format!("{}{}", currency_symbol(&self.currency), self.fee_display())
    }

    pub fn total_value_cents(&self) -> Option<i64> {
        match (self.quantity, self.unit_price_cents) {
            (Some(qty), Some(price)) => Some((qty * price as f64).round() as i64),
            _ => None,
        }
    }

    pub fn total_value_display(&self) -> Option<String> {
        self.total_value_cents().map(|cents| {
            let dollars = cents / 100;
            let remainder = cents % 100;
            format!("{}.{:02}", dollars, remainder)
        })
    }

    pub fn total_value_formatted(&self) -> Option<String> {
        self.total_value_display()
            .map(|value| format!("{}{}", currency_symbol(&self.currency), value))
    }

    pub fn is_cash_symbol(&self) -> bool {
        self.symbol.starts_with("$CASH-")
    }

    pub fn quantity_display(&self) -> String {
        self.quantity
            .map(|q| {
                format!("{:.4}", q)
                    .trim_end_matches('0')
                    .trim_end_matches('.')
                    .to_string()
            })
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct NewTradingActivity {
    pub date: String,
    pub symbol: String,
    pub quantity: Option<f64>,
    pub activity_type: TradingActivityType,
    pub unit_price_cents: Option<i64>,
    pub currency: String,
    pub fee_cents: i64,
    pub notes: Option<String>,
}

impl NewTradingActivity {
    pub fn from_decimal_price(price: f64) -> i64 {
        (price * 100.0).round() as i64
    }

    pub fn from_decimal_fee(fee: f64) -> i64 {
        (fee * 100.0).round() as i64
    }
}

/// Represents a calculated position from aggregated activities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub symbol: String,
    pub quantity: f64,
    pub total_cost_cents: i64,
    pub currency: String,
}

impl Position {
    pub fn average_cost_cents(&self) -> Option<i64> {
        if self.quantity > 0.0 {
            Some((self.total_cost_cents as f64 / self.quantity).round() as i64)
        } else {
            None
        }
    }

    pub fn average_cost_display(&self) -> Option<String> {
        self.average_cost_cents().map(|cents| {
            let dollars = cents / 100;
            let remainder = cents % 100;
            format!("{}.{:02}", dollars, remainder)
        })
    }

    pub fn average_cost_formatted(&self) -> Option<String> {
        self.average_cost_display()
            .map(|cost| format!("{}{}", currency_symbol(&self.currency), cost))
    }

    pub fn total_cost_display(&self) -> String {
        let dollars = self.total_cost_cents / 100;
        let cents = self.total_cost_cents % 100;
        format!("{}.{:02}", dollars, cents)
    }

    pub fn total_cost_formatted(&self) -> String {
        format!(
            "{}{}",
            currency_symbol(&self.currency),
            self.total_cost_display()
        )
    }

    pub fn quantity_display(&self) -> String {
        format!("{:.4}", self.quantity)
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string()
    }

    pub fn is_cash(&self) -> bool {
        self.symbol.starts_with("$CASH-")
    }
}

// Trading Import types

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TradingImportStatus {
    Parsing,
    Preview,
    Importing,
    Completed,
    Failed,
}

impl TradingImportStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Parsing => "parsing",
            Self::Preview => "preview",
            Self::Importing => "importing",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Parsing => "Parsing files...",
            Self::Preview => "Preview",
            Self::Importing => "Importing...",
            Self::Completed => "Import Complete",
            Self::Failed => "Import Failed",
        }
    }
}

impl FromStr for TradingImportStatus {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "parsing" => Ok(Self::Parsing),
            "preview" => Ok(Self::Preview),
            "importing" => Ok(Self::Importing),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradingImportSession {
    pub id: String,
    pub status: TradingImportStatus,
    pub total_rows: i64,
    pub processed_rows: i64,
    pub error_count: i64,
    pub errors: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl TradingImportSession {
    pub fn progress_percent(&self) -> i64 {
        if self.total_rows == 0 {
            0
        } else {
            (self.processed_rows * 100) / self.total_rows
        }
    }

    pub fn is_processing(&self) -> bool {
        matches!(
            self.status,
            TradingImportStatus::Parsing | TradingImportStatus::Importing
        )
    }

    pub fn is_parsing(&self) -> bool {
        matches!(self.status, TradingImportStatus::Parsing)
    }

    pub fn is_preview(&self) -> bool {
        matches!(self.status, TradingImportStatus::Preview)
    }

    pub fn is_importing(&self) -> bool {
        matches!(self.status, TradingImportStatus::Importing)
    }

    pub fn is_completed(&self) -> bool {
        matches!(self.status, TradingImportStatus::Completed)
    }

    pub fn is_failed(&self) -> bool {
        matches!(self.status, TradingImportStatus::Failed)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TradingImportRowStatus {
    Pending,
    Imported,
    Error,
}

impl TradingImportRowStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Imported => "imported",
            Self::Error => "error",
        }
    }
}

impl FromStr for TradingImportRowStatus {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(Self::Pending),
            "imported" => Ok(Self::Imported),
            "error" => Ok(Self::Error),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradingImportRow {
    pub id: i64,
    pub session_id: String,
    pub row_index: i64,
    pub data: crate::services::trading_csv_parser::ParsedTradingActivity,
    pub status: String,
    pub error: Option<String>,
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
