use crate::models::tag::{Tag, TagStyle};
use crate::models::transaction::{NewTransaction, Transaction, TransactionWithRelations};
use rusqlite::{params, Connection, OptionalExtension};
use tracing::{info, trace};

fn transaction_with_relations_from_row(
    row: &rusqlite::Row,
) -> rusqlite::Result<TransactionWithRelations> {
    Ok(TransactionWithRelations {
        transaction: Transaction {
            id: row.get(0)?,
            date: row.get(1)?,
            amount_cents: row.get(2)?,
            currency: row.get(3)?,
            description: row.get(4)?,
            category_id: row.get(5)?,
            account_id: row.get(6)?,
            notes: row.get(7)?,
            created_at: row.get(8)?,
            updated_at: row.get(9)?,
            value_date: row.get(10)?,
            payer: row.get(11)?,
            payee: row.get(12)?,
            reference: row.get(13)?,
            transaction_type: row.get(14)?,
            counterparty_iban: row.get(15)?,
            creditor_id: row.get(16)?,
            mandate_reference: row.get(17)?,
            customer_reference: row.get(18)?,
        },
        category_name: row.get(19)?,
        category_color: row.get(20)?,
        account_name: row.get(21)?,
        tags: Vec::new(),
    })
}

#[derive(Default)]
pub struct TransactionFilter {
    pub search: Option<String>,
    pub category_id: Option<i64>,
    /// Filter by multiple category IDs (OR). Takes precedence over `category_id` when non-empty.
    pub category_ids: Vec<i64>,
    pub tag_id: Option<i64>,
    pub from_date: Option<String>,
    pub to_date: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    /// SQL ORDER BY expression (e.g., "e.date DESC"). Defaults to "e.date DESC, e.id DESC".
    pub sort_sql: Option<String>,
    /// When true, only return transactions without a category.
    pub uncategorized_only: bool,
}

/// Build the WHERE clause fragments and params for a TransactionFilter.
/// Returns SQL conditions (without leading WHERE/AND) appended after "WHERE 1=1",
/// and the corresponding parameter vector.
fn build_filter_where(filter: &TransactionFilter) -> (String, Vec<Box<dyn rusqlite::ToSql>>) {
    let mut sql = String::new();
    let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

    if let Some(ref search) = filter.search {
        sql.push_str(" AND e.description LIKE ?");
        params_vec.push(Box::new(format!("%{}%", search)));
    }
    if !filter.category_ids.is_empty() {
        let placeholders: String = filter
            .category_ids
            .iter()
            .map(|_| "?")
            .collect::<Vec<_>>()
            .join(",");
        sql.push_str(&format!(" AND e.category_id IN ({})", placeholders));
        for &id in &filter.category_ids {
            params_vec.push(Box::new(id));
        }
    } else if let Some(category_id) = filter.category_id {
        sql.push_str(" AND e.category_id = ?");
        params_vec.push(Box::new(category_id));
    }
    if let Some(ref from_date) = filter.from_date {
        sql.push_str(" AND e.date >= ?");
        params_vec.push(Box::new(from_date.clone()));
    }
    if let Some(ref to_date) = filter.to_date {
        sql.push_str(" AND e.date <= ?");
        params_vec.push(Box::new(to_date.clone()));
    }
    if let Some(tag_id) = filter.tag_id {
        sql.push_str(" AND EXISTS(SELECT 1 FROM transaction_tags et WHERE et.transaction_id = e.id AND et.tag_id = ?)");
        params_vec.push(Box::new(tag_id));
    }
    if filter.uncategorized_only {
        sql.push_str(" AND e.category_id IS NULL");
    }

    (sql, params_vec)
}

pub fn list_transactions(
    conn: &Connection,
    filter: &TransactionFilter,
) -> rusqlite::Result<Vec<TransactionWithRelations>> {
    let (where_clause, mut params_vec) = build_filter_where(filter);

    let mut sql = format!(
        "SELECT e.id, e.date, e.amount_cents, e.currency, e.description,
                e.category_id, e.account_id, e.notes, e.created_at, e.updated_at,
                e.value_date, e.payer, e.payee, e.reference, e.transaction_type,
                e.counterparty_iban, e.creditor_id, e.mandate_reference, e.customer_reference,
                c.name as category_name, c.color as category_color, a.name as account_name
         FROM transactions e
         LEFT JOIN categories c ON e.category_id = c.id
         LEFT JOIN accounts a ON e.account_id = a.id
         WHERE 1=1{}",
        where_clause,
    );

    // Use provided sort or default to date DESC
    let order_by = filter.sort_sql.as_deref().unwrap_or("e.date DESC");
    sql.push_str(&format!(" ORDER BY {}, e.id DESC", order_by));

    if let Some(limit) = filter.limit {
        sql.push_str(" LIMIT ?");
        params_vec.push(Box::new(limit));
    }
    if let Some(offset) = filter.offset {
        sql.push_str(" OFFSET ?");
        params_vec.push(Box::new(offset));
    }

    let params_refs: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();
    let mut stmt = conn.prepare(&sql)?;

    let transaction_iter = stmt.query_map(params_refs.as_slice(), |row| {
        transaction_with_relations_from_row(row)
    })?;

    let mut transactions: Vec<TransactionWithRelations> =
        transaction_iter.collect::<Result<Vec<_>, _>>()?;

    let transaction_ids: Vec<i64> = transactions.iter().map(|e| e.transaction.id).collect();
    let mut tags_map = get_tags_for_transactions(conn, &transaction_ids)?;

    for transaction in &mut transactions {
        transaction.tags = tags_map
            .remove(&transaction.transaction.id)
            .unwrap_or_default();
    }

    trace!(count = transactions.len(), "Listed transactions");
    Ok(transactions)
}

/// Returns the earliest and latest transaction dates, or `None` when the table is empty.
pub fn date_extent(conn: &Connection) -> rusqlite::Result<Option<(String, String)>> {
    conn.query_row("SELECT MIN(date), MAX(date) FROM transactions", [], |row| {
        let min: Option<String> = row.get(0)?;
        let max: Option<String> = row.get(1)?;
        Ok(min.zip(max))
    })
}

pub fn count_transactions(conn: &Connection, filter: &TransactionFilter) -> rusqlite::Result<i64> {
    let (where_clause, params_vec) = build_filter_where(filter);
    let sql = format!(
        "SELECT COUNT(*) FROM transactions e WHERE 1=1{}",
        where_clause,
    );
    let params_refs: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();
    conn.query_row(&sql, params_refs.as_slice(), |row| row.get(0))
}

pub fn bulk_set_category(
    conn: &Connection,
    filter: &TransactionFilter,
    category_id: Option<i64>,
) -> rusqlite::Result<usize> {
    let (where_clause, mut params_vec) = build_filter_where(filter);
    let sql = format!(
        "UPDATE transactions SET category_id = ?, updated_at = datetime('now') \
         WHERE id IN (SELECT e.id FROM transactions e WHERE 1=1{})",
        where_clause,
    );
    // The SET value param must come first.
    params_vec.insert(0, Box::new(category_id));
    let params_refs: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();
    let rows = conn.execute(&sql, params_refs.as_slice())?;
    info!(count = rows, category_id = ?category_id, "Bulk set category on transactions");
    Ok(rows)
}

pub fn bulk_set_account(
    conn: &Connection,
    filter: &TransactionFilter,
    account_id: Option<i64>,
) -> rusqlite::Result<usize> {
    let (where_clause, mut params_vec) = build_filter_where(filter);
    let sql = format!(
        "UPDATE transactions SET account_id = ?, updated_at = datetime('now') \
         WHERE id IN (SELECT e.id FROM transactions e WHERE 1=1{})",
        where_clause,
    );
    params_vec.insert(0, Box::new(account_id));
    let params_refs: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();
    let rows = conn.execute(&sql, params_refs.as_slice())?;
    info!(count = rows, account_id = ?account_id, "Bulk set account on transactions");
    Ok(rows)
}

pub fn bulk_add_tag(
    conn: &Connection,
    filter: &TransactionFilter,
    tag_id: i64,
) -> rusqlite::Result<usize> {
    let (where_clause, mut params_vec) = build_filter_where(filter);
    let sql = format!(
        "INSERT OR IGNORE INTO transaction_tags (transaction_id, tag_id) \
         SELECT e.id, ? FROM transactions e WHERE 1=1{}",
        where_clause,
    );
    params_vec.insert(0, Box::new(tag_id));
    let params_refs: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();
    let rows = conn.execute(&sql, params_refs.as_slice())?;
    info!(
        count = rows,
        tag_id = tag_id,
        "Bulk added tag to transactions"
    );
    Ok(rows)
}

pub fn get_transaction(
    conn: &Connection,
    id: i64,
) -> rusqlite::Result<Option<TransactionWithRelations>> {
    trace!(transaction_id = id, "Fetching transaction");
    let transaction = conn
        .query_row(
            "SELECT e.id, e.date, e.amount_cents, e.currency, e.description,
                    e.category_id, e.account_id, e.notes, e.created_at, e.updated_at,
                    e.value_date, e.payer, e.payee, e.reference, e.transaction_type,
                    e.counterparty_iban, e.creditor_id, e.mandate_reference, e.customer_reference,
                    c.name, c.color, a.name
             FROM transactions e
             LEFT JOIN categories c ON e.category_id = c.id
             LEFT JOIN accounts a ON e.account_id = a.id
             WHERE e.id = ?",
            [id],
            transaction_with_relations_from_row,
        )
        .optional()?;

    if let Some(mut exp) = transaction {
        exp.tags = get_transaction_tags(conn, id)?;
        Ok(Some(exp))
    } else {
        Ok(None)
    }
}

pub fn create_transaction(
    conn: &Connection,
    transaction: &NewTransaction,
) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO transactions (date, amount_cents, currency, description, category_id, account_id, notes,
         value_date, payer, payee, reference, transaction_type, counterparty_iban,
         creditor_id, mandate_reference, customer_reference)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        params![
            transaction.date,
            transaction.amount_cents,
            transaction.currency,
            transaction.description,
            transaction.category_id,
            transaction.account_id,
            transaction.notes,
            transaction.value_date,
            transaction.payer,
            transaction.payee,
            transaction.reference,
            transaction.transaction_type,
            transaction.counterparty_iban,
            transaction.creditor_id,
            transaction.mandate_reference,
            transaction.customer_reference,
        ],
    )?;

    let id = conn.last_insert_rowid();

    for tag_id in &transaction.tag_ids {
        conn.execute(
            "INSERT OR IGNORE INTO transaction_tags (transaction_id, tag_id) VALUES (?, ?)",
            params![id, tag_id],
        )?;
    }

    info!(
        transaction_id = id,
        amount_cents = transaction.amount_cents,
        "Created transaction"
    );
    Ok(id)
}

pub fn update_transaction(
    conn: &Connection,
    id: i64,
    transaction: &NewTransaction,
) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE transactions SET date = ?, amount_cents = ?, currency = ?,
         description = ?, category_id = ?, account_id = ?, notes = ?,
         value_date = ?, payer = ?, payee = ?, reference = ?, transaction_type = ?,
         counterparty_iban = ?, creditor_id = ?, mandate_reference = ?, customer_reference = ?,
         updated_at = datetime('now')
         WHERE id = ?",
        params![
            transaction.date,
            transaction.amount_cents,
            transaction.currency,
            transaction.description,
            transaction.category_id,
            transaction.account_id,
            transaction.notes,
            transaction.value_date,
            transaction.payer,
            transaction.payee,
            transaction.reference,
            transaction.transaction_type,
            transaction.counterparty_iban,
            transaction.creditor_id,
            transaction.mandate_reference,
            transaction.customer_reference,
            id,
        ],
    )?;

    conn.execute(
        "DELETE FROM transaction_tags WHERE transaction_id = ?",
        [id],
    )?;

    for tag_id in &transaction.tag_ids {
        conn.execute(
            "INSERT OR IGNORE INTO transaction_tags (transaction_id, tag_id) VALUES (?, ?)",
            params![id, tag_id],
        )?;
    }

    info!(transaction_id = id, "Updated transaction");
    Ok(())
}

pub fn delete_transaction(conn: &Connection, id: i64) -> rusqlite::Result<bool> {
    let rows = conn.execute("DELETE FROM transactions WHERE id = ?", [id])?;
    if rows > 0 {
        info!(transaction_id = id, "Deleted transaction");
    }
    Ok(rows > 0)
}

pub fn unset_category(conn: &Connection, category_id: i64) -> rusqlite::Result<usize> {
    let rows = conn.execute(
        "UPDATE transactions SET category_id = NULL, updated_at = datetime('now') WHERE category_id = ?",
        [category_id],
    )?;
    info!(
        category_id = category_id,
        count = rows,
        "Unset category from transactions"
    );
    Ok(rows)
}

pub fn delete_all_transactions(conn: &Connection) -> rusqlite::Result<usize> {
    conn.execute("DELETE FROM transaction_tags", [])?;
    let rows = conn.execute("DELETE FROM transactions", [])?;
    tracing::warn!(count = rows, "Deleted all transactions");
    Ok(rows)
}

// ---------------------------------------------------------------------------
// Aggregate query helpers (push GROUP BY / SUM into SQL instead of Rust).
// ---------------------------------------------------------------------------

/// Sum `amount_cents` for all transactions matching the given date range.
pub fn sum_amount_cents(
    conn: &Connection,
    from_date: Option<&str>,
    to_date: Option<&str>,
) -> rusqlite::Result<i64> {
    let mut sql =
        "SELECT COALESCE(SUM(e.amount_cents), 0) FROM transactions e WHERE 1=1".to_string();
    let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    if let Some(from) = from_date {
        sql.push_str(" AND e.date >= ?");
        params_vec.push(Box::new(from.to_string()));
    }
    if let Some(to) = to_date {
        sql.push_str(" AND e.date <= ?");
        params_vec.push(Box::new(to.to_string()));
    }
    let params_refs: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();
    conn.query_row(&sql, params_refs.as_slice(), |row| row.get(0))
}

/// Result of a per-category aggregation.
pub struct CategorySum {
    pub category_id: Option<i64>,
    pub category_name: String,
    pub category_color: String,
    pub total_cents: i64,
    pub count: i64,
}

/// Sum transactions grouped by category, excluding the given category IDs.
pub fn sum_by_category(
    conn: &Connection,
    from_date: Option<&str>,
    to_date: Option<&str>,
    exclude_ids: &[i64],
) -> rusqlite::Result<Vec<CategorySum>> {
    let mut sql = String::from(
        "SELECT e.category_id, COALESCE(c.name, 'Uncategorized'), COALESCE(c.color, '#6b7280'), \
         SUM(e.amount_cents), COUNT(*) \
         FROM transactions e \
         LEFT JOIN categories c ON e.category_id = c.id \
         WHERE 1=1",
    );
    let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    if let Some(from) = from_date {
        sql.push_str(" AND e.date >= ?");
        params_vec.push(Box::new(from.to_string()));
    }
    if let Some(to) = to_date {
        sql.push_str(" AND e.date <= ?");
        params_vec.push(Box::new(to.to_string()));
    }
    if !exclude_ids.is_empty() {
        let placeholders: String = exclude_ids
            .iter()
            .map(|_| "?")
            .collect::<Vec<_>>()
            .join(",");
        sql.push_str(&format!(
            " AND (e.category_id IS NULL OR e.category_id NOT IN ({}))",
            placeholders
        ));
        for &id in exclude_ids {
            params_vec.push(Box::new(id));
        }
    }
    sql.push_str(" GROUP BY e.category_id ORDER BY SUM(e.amount_cents)");

    let params_refs: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map(params_refs.as_slice(), |row| {
            Ok(CategorySum {
                category_id: row.get(0)?,
                category_name: row.get(1)?,
                category_color: row.get(2)?,
                total_cents: row.get(3)?,
                count: row.get(4)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Sum transactions grouped by date.
pub fn sum_by_date(
    conn: &Connection,
    from_date: Option<&str>,
    to_date: Option<&str>,
) -> rusqlite::Result<Vec<(String, i64)>> {
    let mut sql = "SELECT e.date, SUM(e.amount_cents) FROM transactions e WHERE 1=1".to_string();
    let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    if let Some(from) = from_date {
        sql.push_str(" AND e.date >= ?");
        params_vec.push(Box::new(from.to_string()));
    }
    if let Some(to) = to_date {
        sql.push_str(" AND e.date <= ?");
        params_vec.push(Box::new(to.to_string()));
    }
    sql.push_str(" GROUP BY e.date ORDER BY e.date");

    let params_refs: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map(params_refs.as_slice(), |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Result of a per-month aggregation.
pub struct MonthSum {
    pub month: String,
    pub total_cents: i64,
    pub count: i64,
}

/// Sum transactions grouped by month (YYYY-MM), filtering to only income
/// (positive amounts) or only expenses (negative amounts, returned as positive).
pub fn sum_by_month(
    conn: &Connection,
    from_date: Option<&str>,
    to_date: Option<&str>,
    income_mode: bool,
) -> rusqlite::Result<Vec<MonthSum>> {
    let amount_expr = if income_mode {
        "e.amount_cents"
    } else {
        "-e.amount_cents"
    };
    let sign_filter = if income_mode {
        " AND e.amount_cents > 0"
    } else {
        " AND e.amount_cents < 0"
    };

    let mut sql = format!(
        "SELECT substr(e.date, 1, 7), SUM({}), COUNT(*) FROM transactions e WHERE 1=1{}",
        amount_expr, sign_filter
    );
    let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    if let Some(from) = from_date {
        sql.push_str(" AND e.date >= ?");
        params_vec.push(Box::new(from.to_string()));
    }
    if let Some(to) = to_date {
        sql.push_str(" AND e.date <= ?");
        params_vec.push(Box::new(to.to_string()));
    }
    sql.push_str(" GROUP BY substr(e.date, 1, 7) ORDER BY 1");

    let params_refs: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map(params_refs.as_slice(), |row| {
            Ok(MonthSum {
                month: row.get(0)?,
                total_cents: row.get(1)?,
                count: row.get(2)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Result of a category-id aggregation with date range.
pub struct CategoryIdSumsWithDates {
    pub sums: Vec<(Option<i64>, i64)>,
    pub min_date: Option<String>,
    pub max_date: Option<String>,
}

/// Sum transactions grouped by category_id, also returning the actual date
/// range (MIN/MAX date) of the matched transactions.  Used by the sankey endpoint.
pub fn sum_by_category_id_with_dates(
    conn: &Connection,
    from_date: Option<&str>,
    to_date: Option<&str>,
) -> rusqlite::Result<CategoryIdSumsWithDates> {
    // Date extent
    let mut date_sql = "SELECT MIN(e.date), MAX(e.date) FROM transactions e WHERE 1=1".to_string();
    let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    if let Some(from) = from_date {
        date_sql.push_str(" AND e.date >= ?");
        params_vec.push(Box::new(from.to_string()));
    }
    if let Some(to) = to_date {
        date_sql.push_str(" AND e.date <= ?");
        params_vec.push(Box::new(to.to_string()));
    }
    let params_refs: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();
    let (min_date, max_date): (Option<String>, Option<String>) =
        conn.query_row(&date_sql, params_refs.as_slice(), |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?;

    // Category sums
    let mut agg_sql =
        "SELECT e.category_id, SUM(e.amount_cents) FROM transactions e WHERE 1=1".to_string();
    let mut params_vec2: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    if let Some(from) = from_date {
        agg_sql.push_str(" AND e.date >= ?");
        params_vec2.push(Box::new(from.to_string()));
    }
    if let Some(to) = to_date {
        agg_sql.push_str(" AND e.date <= ?");
        params_vec2.push(Box::new(to.to_string()));
    }
    agg_sql.push_str(" GROUP BY e.category_id");
    let params_refs2: Vec<&dyn rusqlite::ToSql> = params_vec2.iter().map(|p| p.as_ref()).collect();
    let mut stmt = conn.prepare(&agg_sql)?;
    let rows = stmt
        .query_map(params_refs2.as_slice(), |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(CategoryIdSumsWithDates {
        sums: rows,
        min_date,
        max_date,
    })
}

/// Raw expense row used for recurring expense detection.
pub struct ExpenseRow {
    pub date: String,
    pub amount_cents: i64,
    pub description: String,
    pub payee: Option<String>,
    pub counterparty_iban: Option<String>,
}

/// Fetch all expense transactions (amount_cents < 0), excluding the given
/// category IDs (typically the Transfers subtree), ordered by date.
/// Used for recurring expense detection in the handler.
pub fn fetch_expenses_for_recurring_detection(
    conn: &Connection,
    exclude_category_ids: &[i64],
) -> rusqlite::Result<Vec<ExpenseRow>> {
    let mut sql = String::from(
        "SELECT e.date, e.amount_cents, e.description, e.payee, e.counterparty_iban \
         FROM transactions e \
         WHERE e.amount_cents < 0",
    );
    let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

    if !exclude_category_ids.is_empty() {
        let placeholders: String = exclude_category_ids
            .iter()
            .map(|_| "?")
            .collect::<Vec<_>>()
            .join(",");
        sql.push_str(&format!(
            " AND (e.category_id IS NULL OR e.category_id NOT IN ({}))",
            placeholders
        ));
        for &id in exclude_category_ids {
            params_vec.push(Box::new(id));
        }
    }

    sql.push_str(" ORDER BY e.date");

    let params_refs: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map(params_refs.as_slice(), |row| {
            Ok(ExpenseRow {
                date: row.get(0)?,
                amount_cents: row.get(1)?,
                description: row.get(2)?,
                payee: row.get(3)?,
                counterparty_iban: row.get(4)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn get_transaction_tags(conn: &Connection, transaction_id: i64) -> rusqlite::Result<Vec<Tag>> {
    let mut stmt = conn.prepare(
        "SELECT t.id, t.name, t.color, t.style, t.created_at
         FROM tags t
         JOIN transaction_tags et ON t.id = et.tag_id
         WHERE et.transaction_id = ?
         ORDER BY t.name",
    )?;

    let tags = stmt
        .query_map([transaction_id], |row| {
            let style_str: String = row.get(3)?;
            Ok(Tag {
                id: row.get(0)?,
                name: row.get(1)?,
                color: row.get(2)?,
                style: TagStyle::parse(&style_str),
                created_at: row.get(4)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(tags)
}

fn get_tags_for_transactions(
    conn: &Connection,
    transaction_ids: &[i64],
) -> rusqlite::Result<std::collections::HashMap<i64, Vec<Tag>>> {
    use std::collections::HashMap;

    if transaction_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let placeholders: String = transaction_ids
        .iter()
        .map(|_| "?")
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        "SELECT et.transaction_id, t.id, t.name, t.color, t.style, t.created_at
         FROM tags t
         JOIN transaction_tags et ON t.id = et.tag_id
         WHERE et.transaction_id IN ({})
         ORDER BY t.name",
        placeholders
    );

    let mut stmt = conn.prepare(&sql)?;
    let params: Vec<&dyn rusqlite::ToSql> = transaction_ids
        .iter()
        .map(|id| id as &dyn rusqlite::ToSql)
        .collect();

    let rows = stmt.query_map(params.as_slice(), |row| {
        let style_str: String = row.get(4)?;
        Ok((
            row.get::<_, i64>(0)?,
            Tag {
                id: row.get(1)?,
                name: row.get(2)?,
                color: row.get(3)?,
                style: TagStyle::parse(&style_str),
                created_at: row.get(5)?,
            },
        ))
    })?;

    let mut tags_map: HashMap<i64, Vec<Tag>> = HashMap::new();
    for row in rows {
        let row = row?;
        tags_map.entry(row.0).or_default().push(row.1);
    }

    Ok(tags_map)
}
