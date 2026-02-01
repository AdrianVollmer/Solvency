use std::str::FromStr;

use chrono::{Datelike, Local, NaiveDate};

/// Trait for filter params that support date filtering with presets and navigation.
#[allow(clippy::wrong_self_convention)]
pub trait DateFilterable {
    fn from_date(&self) -> Option<&String>;
    fn to_date(&self) -> Option<&String>;
    fn preset(&self) -> Option<&String>;

    /// Override to support prev/next navigation. Defaults to None.
    fn nav(&self) -> Option<&String> {
        None
    }

    fn resolve_date_range(&self) -> DateRange {
        let base_range = if let Some(preset_str) = self.preset() {
            preset_str
                .parse::<DatePreset>()
                .map(DateRange::from_preset)
                .unwrap_or_default()
        } else if let (Some(from), Some(to)) = (self.from_date(), self.to_date()) {
            if let (Ok(from_date), Ok(to_date)) = (
                NaiveDate::parse_from_str(from, "%Y-%m-%d"),
                NaiveDate::parse_from_str(to, "%Y-%m-%d"),
            ) {
                DateRange::from_dates(from_date, to_date)
            } else {
                DateRange::default()
            }
        } else {
            DateRange::default()
        };

        match self.nav().map(|s| s.as_str()) {
            Some("prev") => base_range.prev(),
            Some("next") => base_range.next(),
            _ => base_range,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PeriodType {
    Week,
    Month,
    Quarter,
    Year,
    All,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DatePreset {
    ThisWeek,
    ThisMonth,
    ThisQuarter,
    ThisYear,
    LastWeek,
    LastMonth,
    LastQuarter,
    LastYear,
    All,
}

impl FromStr for DatePreset {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "this_week" => Ok(Self::ThisWeek),
            "this_month" => Ok(Self::ThisMonth),
            "this_quarter" => Ok(Self::ThisQuarter),
            "this_year" => Ok(Self::ThisYear),
            "last_week" => Ok(Self::LastWeek),
            "last_month" => Ok(Self::LastMonth),
            "last_quarter" => Ok(Self::LastQuarter),
            "last_year" => Ok(Self::LastYear),
            "all" => Ok(Self::All),
            _ => Err(()),
        }
    }
}

impl DatePreset {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ThisWeek => "this_week",
            Self::ThisMonth => "this_month",
            Self::ThisQuarter => "this_quarter",
            Self::ThisYear => "this_year",
            Self::LastWeek => "last_week",
            Self::LastMonth => "last_month",
            Self::LastQuarter => "last_quarter",
            Self::LastYear => "last_year",
            Self::All => "all",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::ThisWeek => "This Week",
            Self::ThisMonth => "This Month",
            Self::ThisQuarter => "This Quarter",
            Self::ThisYear => "This Year",
            Self::LastWeek => "Last Week",
            Self::LastMonth => "Last Month",
            Self::LastQuarter => "Last Quarter",
            Self::LastYear => "Last Year",
            Self::All => "All",
        }
    }

    pub fn all() -> &'static [DatePreset] {
        &[
            Self::ThisWeek,
            Self::ThisMonth,
            Self::ThisQuarter,
            Self::ThisYear,
            Self::LastWeek,
            Self::LastMonth,
            Self::LastQuarter,
            Self::LastYear,
            Self::All,
        ]
    }
}

#[derive(Debug, Clone)]
pub struct DateRange {
    pub from: NaiveDate,
    pub to: NaiveDate,
    pub preset: Option<DatePreset>,
}

impl DateRange {
    pub fn from_preset(preset: DatePreset) -> Self {
        let today = Local::now().date_naive();
        let (from, to) = match preset {
            DatePreset::ThisWeek => {
                let start = week_start(today);
                let end = week_end(today);
                (start, end)
            }
            DatePreset::LastWeek => {
                let last_week = today - chrono::Duration::days(7);
                let start = week_start(last_week);
                let end = week_end(last_week);
                (start, end)
            }
            DatePreset::ThisMonth => {
                let start = month_start(today);
                let end = month_end(today);
                (start, end)
            }
            DatePreset::LastMonth => {
                let last_month = today - chrono::Duration::days(today.day() as i64);
                let start = month_start(last_month);
                let end = month_end(last_month);
                (start, end)
            }
            DatePreset::ThisQuarter => {
                let start = quarter_start(today);
                let end = quarter_end(today);
                (start, end)
            }
            DatePreset::LastQuarter => {
                let current_quarter_start = quarter_start(today);
                let last_quarter = current_quarter_start - chrono::Duration::days(1);
                let start = quarter_start(last_quarter);
                let end = quarter_end(last_quarter);
                (start, end)
            }
            DatePreset::ThisYear => {
                let start = year_start(today);
                let end = year_end(today);
                (start, end)
            }
            DatePreset::LastYear => {
                let last_year = NaiveDate::from_ymd_opt(today.year() - 1, 1, 1).unwrap();
                let start = year_start(last_year);
                let end = year_end(last_year);
                (start, end)
            }
            DatePreset::All => {
                let start = NaiveDate::from_ymd_opt(1970, 1, 1).unwrap();
                let end = NaiveDate::from_ymd_opt(2099, 12, 31).unwrap();
                (start, end)
            }
        };
        Self {
            from,
            to,
            preset: Some(preset),
        }
    }

    pub fn from_dates(from: NaiveDate, to: NaiveDate) -> Self {
        let preset = detect_preset(from, to);
        Self { from, to, preset }
    }

    pub fn prev(&self) -> Self {
        let period = self.detect_period_type();
        let (new_from, new_to) = shift_by_period(self.from, self.to, period, -1);
        Self::from_dates(new_from, new_to)
    }

    pub fn next(&self) -> Self {
        let period = self.detect_period_type();
        let (new_from, new_to) = shift_by_period(self.from, self.to, period, 1);
        Self::from_dates(new_from, new_to)
    }

    fn detect_period_type(&self) -> PeriodType {
        // Check if it's "All" (the widest possible range)
        if self.preset == Some(DatePreset::All) {
            return PeriodType::All;
        }

        // Check if it's a week (7 days, starts on Monday)
        let start = week_start(self.from);
        let end = week_end(self.from);
        if self.from == start && self.to == end {
            return PeriodType::Week;
        }

        // Check if it's a month
        let start = month_start(self.from);
        let end = month_end(self.from);
        if self.from == start && self.to == end {
            return PeriodType::Month;
        }

        // Check if it's a quarter
        let start = quarter_start(self.from);
        let end = quarter_end(self.from);
        if self.from == start && self.to == end {
            return PeriodType::Quarter;
        }

        // Check if it's a year
        let start = year_start(self.from);
        let end = year_end(self.from);
        if self.from == start && self.to == end {
            return PeriodType::Year;
        }

        PeriodType::Custom
    }

    /// Human-readable label for the current range, e.g. "January 2026", "Q1 2026",
    /// "2025", "Jan 27 – Feb 2, 2026", or "Jan 1 – Mar 15, 2026".
    pub fn display_label(&self) -> String {
        let period = self.detect_period_type();
        match period {
            PeriodType::Week => {
                let from_fmt = self.from.format("%b %-d");
                if self.from.year() == self.to.year() {
                    if self.from.month() == self.to.month() {
                        format!(
                            "{} – {}, {}",
                            from_fmt,
                            self.to.format("%-d"),
                            self.to.format("%Y")
                        )
                    } else {
                        format!(
                            "{} – {}, {}",
                            from_fmt,
                            self.to.format("%b %-d"),
                            self.to.format("%Y")
                        )
                    }
                } else {
                    format!(
                        "{}, {} – {}, {}",
                        from_fmt,
                        self.from.format("%Y"),
                        self.to.format("%b %-d"),
                        self.to.format("%Y")
                    )
                }
            }
            PeriodType::Month => self.from.format("%B %Y").to_string(),
            PeriodType::Quarter => {
                let q = (self.from.month() - 1) / 3 + 1;
                format!("Q{} {}", q, self.from.year())
            }
            PeriodType::Year => self.from.format("%Y").to_string(),
            PeriodType::All => "All Time".to_string(),
            PeriodType::Custom => {
                let from_fmt = self.from.format("%b %-d");
                if self.from.year() == self.to.year() {
                    format!(
                        "{} – {}, {}",
                        from_fmt,
                        self.to.format("%b %-d"),
                        self.to.format("%Y")
                    )
                } else {
                    format!(
                        "{}, {} – {}, {}",
                        from_fmt,
                        self.from.format("%Y"),
                        self.to.format("%b %-d"),
                        self.to.format("%Y")
                    )
                }
            }
        }
    }

    pub fn from_str(&self) -> String {
        self.from.format("%Y-%m-%d").to_string()
    }

    pub fn to_str(&self) -> String {
        self.to.format("%Y-%m-%d").to_string()
    }

    pub fn is_preset(&self, preset: &DatePreset) -> bool {
        self.preset == Some(*preset)
    }

    /// For the "All" preset, narrow the range to the actual data extent.
    /// `extent` should be `Some((min_date, max_date))` with `"YYYY-MM-DD"` strings,
    /// typically from a `SELECT MIN(date), MAX(date)` query.
    /// For other presets or when `extent` is `None`, returns `self` unchanged.
    pub fn resolve_all(self, extent: Option<(String, String)>) -> Self {
        if self.preset != Some(DatePreset::All) {
            return self;
        }
        match extent {
            Some((min_date, max_date)) => {
                let from = NaiveDate::parse_from_str(&min_date, "%Y-%m-%d").unwrap_or(self.from);
                let to = NaiveDate::parse_from_str(&max_date, "%Y-%m-%d").unwrap_or(self.to);
                Self {
                    from,
                    to,
                    preset: Some(DatePreset::All),
                }
            }
            None => self,
        }
    }

    /// Returns query string for preserving date range state in URLs.
    /// Format: `from_date=YYYY-MM-DD&to_date=YYYY-MM-DD` with optional `&preset=X`
    pub fn query_string(&self) -> String {
        let mut qs = format!("from_date={}&to_date={}", self.from_str(), self.to_str());
        if let Some(preset) = &self.preset {
            qs.push_str("&preset=");
            qs.push_str(preset.as_str());
        }
        qs
    }
}

impl Default for DateRange {
    fn default() -> Self {
        Self::from_preset(DatePreset::ThisMonth)
    }
}

fn week_start(date: NaiveDate) -> NaiveDate {
    let days_from_monday = date.weekday().num_days_from_monday();
    date - chrono::Duration::days(days_from_monday as i64)
}

fn week_end(date: NaiveDate) -> NaiveDate {
    week_start(date) + chrono::Duration::days(6)
}

fn month_start(date: NaiveDate) -> NaiveDate {
    NaiveDate::from_ymd_opt(date.year(), date.month(), 1).unwrap()
}

fn month_end(date: NaiveDate) -> NaiveDate {
    let next_month = if date.month() == 12 {
        NaiveDate::from_ymd_opt(date.year() + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(date.year(), date.month() + 1, 1)
    };
    next_month.unwrap() - chrono::Duration::days(1)
}

fn quarter_start(date: NaiveDate) -> NaiveDate {
    let quarter = (date.month() - 1) / 3;
    let start_month = quarter * 3 + 1;
    NaiveDate::from_ymd_opt(date.year(), start_month, 1).unwrap()
}

fn quarter_end(date: NaiveDate) -> NaiveDate {
    let quarter = (date.month() - 1) / 3;
    let end_month = quarter * 3 + 3;
    let next = if end_month == 12 {
        NaiveDate::from_ymd_opt(date.year() + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(date.year(), end_month + 1, 1)
    };
    next.unwrap() - chrono::Duration::days(1)
}

fn year_start(date: NaiveDate) -> NaiveDate {
    NaiveDate::from_ymd_opt(date.year(), 1, 1).unwrap()
}

fn year_end(date: NaiveDate) -> NaiveDate {
    NaiveDate::from_ymd_opt(date.year(), 12, 31).unwrap()
}

fn detect_preset(from: NaiveDate, to: NaiveDate) -> Option<DatePreset> {
    for preset in DatePreset::all() {
        let range = DateRange::from_preset(*preset);
        if range.from == from && range.to == to {
            return Some(*preset);
        }
    }
    None
}

fn shift_by_period(
    from: NaiveDate,
    to: NaiveDate,
    period: PeriodType,
    direction: i32,
) -> (NaiveDate, NaiveDate) {
    match period {
        PeriodType::Week => {
            let shift = chrono::Duration::days(7 * direction as i64);
            (from + shift, to + shift)
        }
        PeriodType::Month => {
            let new_from = shift_months(from, direction);
            let new_to = month_end(new_from);
            (new_from, new_to)
        }
        PeriodType::Quarter => {
            let new_from = shift_months(from, direction * 3);
            let new_to = quarter_end(new_from);
            (new_from, new_to)
        }
        PeriodType::Year => {
            let new_year = from.year() + direction;
            let new_from = NaiveDate::from_ymd_opt(new_year, 1, 1).unwrap();
            let new_to = NaiveDate::from_ymd_opt(new_year, 12, 31).unwrap();
            (new_from, new_to)
        }
        PeriodType::All => {
            // "All" covers everything; navigation is a no-op
            (from, to)
        }
        PeriodType::Custom => {
            // For custom ranges, shift by the range duration
            let duration = to - from + chrono::Duration::days(1);
            let shift = if direction > 0 { duration } else { -duration };
            (from + shift, to + shift)
        }
    }
}

fn shift_months(date: NaiveDate, months: i32) -> NaiveDate {
    let total_months = date.year() * 12 + date.month() as i32 - 1 + months;
    let new_year = total_months.div_euclid(12);
    let new_month = (total_months.rem_euclid(12) + 1) as u32;
    NaiveDate::from_ymd_opt(new_year, new_month, 1).unwrap()
}
