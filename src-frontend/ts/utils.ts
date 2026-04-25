// Shared utilities for chart files. Imported by each chart entry point;
// esbuild inlines this into each bundle.

export function isDarkMode(): boolean {
  return document.documentElement.classList.contains("dark");
}

export function getTheme(): string | undefined {
  return isDarkMode() ? "dark" : undefined;
}

// Symbol table mirrors filters.rs::currency_symbol() — keep in sync.
const CURRENCY_SYMBOLS: Record<string, string> = {
  USD: "$",
  EUR: "€",
  GBP: "£",
  JPY: "¥",
  CNY: "¥",
  CAD: "C$",
  AUD: "A$",
  CHF: "CHF ",
  INR: "₹",
  BRL: "R$",
  MXN: "MX$",
  KRW: "₩",
  SEK: "kr ",
  NOK: "kr ",
  DKK: "kr ",
  PLN: "zł ",
  RUB: "₽",
  TRY: "₺",
  ZAR: "R ",
  SGD: "S$",
  HKD: "HK$",
  NZD: "NZ$",
  THB: "฿",
};

// Falls back to the currency code + non-breaking space for unrecognised currencies
// (e.g. "CZK ") so the symbol is always visually distinct and correct.
export function getCurrencySymbol(currency: string): string {
  return CURRENCY_SYMBOLS[currency.toUpperCase()] ?? (currency.toUpperCase() + " ");
}

export function formatMoney(
  cents: number,
  currency: string,
  locale: string,
  fractionDigits = 2,
): string {
  const symbol = getCurrencySymbol(currency);
  const value = cents / 100;
  return (
    symbol +
    value.toLocaleString(locale, {
      minimumFractionDigits: fractionDigits,
      maximumFractionDigits: fractionDigits,
    })
  );
}
