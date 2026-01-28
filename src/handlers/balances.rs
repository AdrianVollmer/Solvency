use askama::Template;
use axum::extract::State;
use axum::response::Html;

use crate::db::queries::{accounts, balances, market_data, settings, trading};
use crate::error::{AppResult, RenderHtml};
use crate::filters;
use crate::models::account::{Account, AccountType};
use crate::models::trading::PositionWithMarketData;
use crate::models::Settings;
use crate::state::{AppState, JsManifest};
use crate::VERSION;

pub struct AccountBalance {
    pub account: Account,
    pub balance_cents: i64,
    pub balance_formatted: String,
    pub balance_color: &'static str,
}

#[derive(Template)]
#[template(path = "pages/balances.html")]
pub struct BalancesTemplate {
    pub title: String,
    pub settings: Settings,
    pub icons: crate::filters::Icons,
    pub manifest: JsManifest,
    pub version: &'static str,
    pub xsrf_token: String,
    pub accounts: Vec<AccountBalance>,
    pub total_balance_cents: i64,
    pub total_balance_formatted: String,
    pub total_balance_color: &'static str,
}

fn gain_loss_color(cents: i64) -> &'static str {
    if cents > 0 {
        "text-green-600 dark:text-green-400"
    } else if cents < 0 {
        "text-red-600 dark:text-red-400"
    } else {
        "text-neutral-600 dark:text-neutral-400"
    }
}

pub async fn index(State(state): State<AppState>) -> AppResult<Html<String>> {
    let conn = state.db.get()?;
    let app_settings = settings::get_settings(&conn)?;

    let all_accounts = accounts::list_accounts(&conn)?;
    let cash_balances = balances::get_cash_account_balances(&conn)?;

    let currency = &app_settings.currency;
    let locale = &app_settings.locale;

    let mut account_balances: Vec<AccountBalance> = Vec::new();

    for account in all_accounts {
        let balance_cents = match account.account_type {
            AccountType::Cash => *cash_balances.get(&account.id).unwrap_or(&0),
            AccountType::Securities => {
                let positions = trading::get_positions_for_account(&conn, account.id)?;

                let mut total: i64 = 0;
                for pos in positions {
                    // Try to get current market value
                    let enriched =
                        if let Ok(Some(data)) = market_data::get_latest_price(&conn, &pos.symbol) {
                            PositionWithMarketData::with_market_data(
                                pos,
                                data.close_price_cents,
                                data.date,
                            )
                        } else if let Ok(Some((price_cents, date))) =
                            trading::get_last_trade_price(&conn, &pos.symbol)
                        {
                            PositionWithMarketData::with_approximated_price(pos, price_cents, date)
                        } else {
                            PositionWithMarketData::from_position(pos)
                        };

                    total += enriched
                        .current_value_cents
                        .unwrap_or(enriched.position.total_cost_cents);
                }
                total
            }
        };

        account_balances.push(AccountBalance {
            balance_formatted: filters::format_money_plain(balance_cents, currency, locale),
            balance_color: gain_loss_color(balance_cents),
            account,
            balance_cents,
        });
    }

    let total_balance_cents: i64 = account_balances.iter().map(|a| a.balance_cents).sum();

    let template = BalancesTemplate {
        title: "Balances".into(),
        settings: app_settings.clone(),
        icons: crate::filters::Icons,
        manifest: state.manifest.clone(),
        version: VERSION,
        xsrf_token: state.xsrf_token.value().to_string(),
        accounts: account_balances,
        total_balance_formatted: filters::format_money_plain(total_balance_cents, currency, locale),
        total_balance_color: gain_loss_color(total_balance_cents),
        total_balance_cents,
    };

    template.render_html()
}
