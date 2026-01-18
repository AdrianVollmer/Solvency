//! Money formatting utilities for displaying monetary amounts.
//!
//! Format: sign + currency symbol + number with thousands separator
//!
//! Color coding:
//! - Positive amounts (> 0): green
//! - Negative amounts (< 0): red
//! - Zero (= 0): default text color (black in light mode, white in dark mode)

/// Format cents as a colored money display with proper locale formatting.
/// Returns HTML with appropriate Tailwind color classes.
pub fn format_money(cents: i64, currency: &str, locale: &str) -> String {
    let (formatted, color_class) = format_money_impl(cents, currency, locale);
    format!(r#"<span class="{}">{}</span>"#, color_class, formatted)
}

/// Format cents as plain text (no HTML/color), useful for inputs or exports.
pub fn format_money_plain(cents: i64, currency: &str, locale: &str) -> String {
    let (formatted, _) = format_money_impl(cents, currency, locale);
    formatted
}

/// Format cents as plain text without sign prefix, useful for prices/fees.
/// This is "neutral" formatting - no +/- sign, no color coding.
pub fn format_money_neutral(cents: i64, currency: &str, locale: &str) -> String {
    let abs_cents = cents.abs();
    let whole = abs_cents / 100;
    let fractional = abs_cents % 100;

    let (thousands_sep, decimal_sep) = locale_separators(locale);
    let whole_str = format_with_thousands(whole, thousands_sep);
    let symbol = currency_symbol(currency);

    format!("{}{}{}{:02}", symbol, whole_str, decimal_sep, fractional)
}

/// Format a percentage value with locale-aware decimal separator.
/// Shows sign (+/-) and two decimal places.
/// Example: 12.345 -> "+12.35%" (en-US) or "+12,35%" (de-DE)
pub fn format_percent(value: f64, locale: &str) -> String {
    let (_, decimal_sep) = locale_separators(locale);
    let sign = if value > 0.0 {
        "+"
    } else if value < 0.0 {
        "-"
    } else {
        ""
    };
    let abs_value = value.abs();
    let whole = abs_value.trunc() as i64;
    let fractional = ((abs_value.fract() * 100.0).round() as i64).abs();

    format!("{}{}{}{}%", sign, whole, decimal_sep, format_args!("{:02}", fractional))
}

fn format_money_impl(cents: i64, currency: &str, locale: &str) -> (String, &'static str) {
    // Determine color class based on amount
    let color_class = if cents > 0 {
        "text-green-600 dark:text-green-400"
    } else if cents < 0 {
        "text-red-600 dark:text-red-400"
    } else {
        "text-gray-900 dark:text-gray-100"
    };

    // Format the amount
    let is_negative = cents < 0;
    let abs_cents = cents.abs();
    let whole = abs_cents / 100;
    let fractional = abs_cents % 100;

    // Get separators based on locale
    let (thousands_sep, decimal_sep) = locale_separators(locale);

    // Format with thousands separator
    let whole_str = format_with_thousands(whole, thousands_sep);

    // Get currency symbol
    let symbol = currency_symbol(currency);

    // Build final string: sign + symbol + formatted number
    let formatted = if is_negative {
        format!("-{}{}{}{:02}", symbol, whole_str, decimal_sep, fractional)
    } else if cents > 0 {
        format!("+{}{}{}{:02}", symbol, whole_str, decimal_sep, fractional)
    } else {
        format!("{}{}{}{:02}", symbol, whole_str, decimal_sep, fractional)
    };

    (formatted, color_class)
}

/// Get thousands and decimal separators based on locale.
fn locale_separators(locale: &str) -> (char, char) {
    // Locales that use period as thousands separator and comma as decimal
    match locale {
        "de-DE" | "de-AT" | "de-CH" | "fr-FR" | "fr-BE" | "fr-CA" | "es-ES" | "es-AR" | "it-IT"
        | "pt-BR" | "pt-PT" | "nl-NL" | "nl-BE" | "pl-PL" | "ru-RU" | "tr-TR" | "vi-VN"
        | "id-ID" | "da-DK" | "nb-NO" | "sv-SE" | "fi-FI" | "cs-CZ" | "sk-SK" | "hu-HU"
        | "ro-RO" | "bg-BG" | "uk-UA" | "el-GR" => ('.', ','),
        // Most English-speaking countries and others use comma as thousands, period as decimal
        _ => (',', '.'),
    }
}

/// Format a number with thousands separators.
fn format_with_thousands(n: i64, sep: char) -> String {
    if n == 0 {
        return "0".to_string();
    }

    let s = n.to_string();
    let chars: Vec<char> = s.chars().rev().collect();
    let mut result = Vec::new();

    for (i, c) in chars.iter().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(sep);
        }
        result.push(*c);
    }

    result.iter().rev().collect()
}

/// Get currency symbol for a currency code.
fn currency_symbol(currency: &str) -> &'static str {
    match currency.to_uppercase().as_str() {
        "USD" => "$",
        "EUR" => "\u{20ac}",
        "GBP" => "\u{00a3}",
        "JPY" => "\u{00a5}",
        "CNY" => "\u{00a5}",
        "CAD" => "C$",
        "AUD" => "A$",
        "CHF" => "CHF\u{00a0}",
        "INR" => "\u{20b9}",
        "BRL" => "R$",
        "MXN" => "MX$",
        "KRW" => "\u{20a9}",
        "SEK" => "kr\u{00a0}",
        "NOK" => "kr\u{00a0}",
        "DKK" => "kr\u{00a0}",
        "PLN" => "z\u{0142}\u{00a0}",
        "RUB" => "\u{20bd}",
        "TRY" => "\u{20ba}",
        "ZAR" => "R\u{00a0}",
        "SGD" => "S$",
        "HKD" => "HK$",
        "NZD" => "NZ$",
        "THB" => "\u{0e3f}",
        _ => "$",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_positive_amount() {
        let result = format_money_plain(12345, "USD", "en-US");
        assert_eq!(result, "+$123.45");
    }

    #[test]
    fn test_negative_amount() {
        let result = format_money_plain(-12345, "USD", "en-US");
        assert_eq!(result, "-$123.45");
    }

    #[test]
    fn test_zero_amount() {
        let result = format_money_plain(0, "USD", "en-US");
        assert_eq!(result, "$0.00");
    }

    #[test]
    fn test_thousands_separator_en() {
        let result = format_money_plain(123456789, "USD", "en-US");
        assert_eq!(result, "+$1,234,567.89");
    }

    #[test]
    fn test_thousands_separator_de() {
        let result = format_money_plain(123456789, "EUR", "de-DE");
        assert_eq!(result, "+\u{20ac}1.234.567,89");
    }

    #[test]
    fn test_color_class_positive() {
        let result = format_money(100, "USD", "en-US");
        assert!(result.contains("text-green-600"));
    }

    #[test]
    fn test_color_class_negative() {
        let result = format_money(-100, "USD", "en-US");
        assert!(result.contains("text-red-600"));
    }

    #[test]
    fn test_color_class_zero() {
        let result = format_money(0, "USD", "en-US");
        assert!(result.contains("text-gray-900"));
    }

    #[test]
    fn test_neutral_no_sign() {
        let result = format_money_neutral(12345, "USD", "en-US");
        assert_eq!(result, "$123.45");
    }

    #[test]
    fn test_neutral_thousands_separator_en() {
        let result = format_money_neutral(123456789, "USD", "en-US");
        assert_eq!(result, "$1,234,567.89");
    }

    #[test]
    fn test_neutral_thousands_separator_de() {
        let result = format_money_neutral(123456789, "EUR", "de-DE");
        assert_eq!(result, "\u{20ac}1.234.567,89");
    }

    #[test]
    fn test_percent_positive_en() {
        let result = format_percent(12.34, "en-US");
        assert_eq!(result, "+12.34%");
    }

    #[test]
    fn test_percent_negative_en() {
        let result = format_percent(-5.67, "en-US");
        assert_eq!(result, "-5.67%");
    }

    #[test]
    fn test_percent_zero() {
        let result = format_percent(0.0, "en-US");
        assert_eq!(result, "0.00%");
    }

    #[test]
    fn test_percent_de_locale() {
        let result = format_percent(12.34, "de-DE");
        assert_eq!(result, "+12,34%");
    }
}
