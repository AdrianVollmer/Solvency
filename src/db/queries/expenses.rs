use crate::models::expense::{Expense, ExpenseWithRelations, NewExpense};
use crate::models::tag::{Tag, TagStyle};
use rusqlite::{params, Connection, OptionalExtension};
use tracing::{debug, trace};

#[derive(Default)]
pub struct ExpenseFilter {
    pub search: Option<String>,
    pub category_id: Option<i64>,
    pub tag_id: Option<i64>,
    pub from_date: Option<String>,
    pub to_date: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    /// SQL ORDER BY expression (e.g., "e.date DESC"). Defaults to "e.date DESC, e.id DESC".
    pub sort_sql: Option<String>,
}

pub fn list_expenses(
    conn: &Connection,
    filter: &ExpenseFilter,
) -> rusqlite::Result<Vec<ExpenseWithRelations>> {
    let mut sql = String::from(
        "SELECT e.id, e.date, e.amount_cents, e.currency, e.description,
                e.category_id, e.notes, e.created_at, e.updated_at,
                e.value_date, e.payer, e.payee, e.reference, e.transaction_type,
                e.counterparty_iban, e.creditor_id, e.mandate_reference, e.customer_reference,
                c.name as category_name, c.color as category_color
         FROM expenses e
         LEFT JOIN categories c ON e.category_id = c.id
         WHERE 1=1",
    );
    let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

    if let Some(ref search) = filter.search {
        sql.push_str(" AND e.description LIKE ?");
        params_vec.push(Box::new(format!("%{}%", search)));
    }
    if let Some(category_id) = filter.category_id {
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
        sql.push_str(" AND EXISTS(SELECT 1 FROM expense_tags et WHERE et.expense_id = e.id AND et.tag_id = ?)");
        params_vec.push(Box::new(tag_id));
    }

    // Use provided sort or default to date DESC
    let order_by = filter
        .sort_sql
        .as_deref()
        .unwrap_or("e.date DESC");
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

    let expense_iter = stmt.query_map(params_refs.as_slice(), |row| {
        Ok(ExpenseWithRelations {
            expense: Expense {
                id: row.get(0)?,
                date: row.get(1)?,
                amount_cents: row.get(2)?,
                currency: row.get(3)?,
                description: row.get(4)?,
                category_id: row.get(5)?,
                notes: row.get(6)?,
                created_at: row.get(7)?,
                updated_at: row.get(8)?,
                value_date: row.get(9)?,
                payer: row.get(10)?,
                payee: row.get(11)?,
                reference: row.get(12)?,
                transaction_type: row.get(13)?,
                counterparty_iban: row.get(14)?,
                creditor_id: row.get(15)?,
                mandate_reference: row.get(16)?,
                customer_reference: row.get(17)?,
            },
            category_name: row.get(18)?,
            category_color: row.get(19)?,
            tags: Vec::new(),
        })
    })?;

    let mut expenses: Vec<ExpenseWithRelations> = expense_iter.filter_map(|e| e.ok()).collect();

    let expense_ids: Vec<i64> = expenses.iter().map(|e| e.expense.id).collect();
    let mut tags_map = get_tags_for_expenses(conn, &expense_ids)?;

    for expense in &mut expenses {
        expense.tags = tags_map.remove(&expense.expense.id).unwrap_or_default();
    }

    debug!(count = expenses.len(), "Listed expenses");
    Ok(expenses)
}

pub fn count_expenses(conn: &Connection, filter: &ExpenseFilter) -> rusqlite::Result<i64> {
    let mut sql = String::from("SELECT COUNT(*) FROM expenses e WHERE 1=1");
    let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

    if let Some(ref search) = filter.search {
        sql.push_str(" AND e.description LIKE ?");
        params_vec.push(Box::new(format!("%{}%", search)));
    }
    if let Some(category_id) = filter.category_id {
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
        sql.push_str(" AND EXISTS(SELECT 1 FROM expense_tags et WHERE et.expense_id = e.id AND et.tag_id = ?)");
        params_vec.push(Box::new(tag_id));
    }

    let params_refs: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();
    conn.query_row(&sql, params_refs.as_slice(), |row| row.get(0))
}

pub fn get_expense(conn: &Connection, id: i64) -> rusqlite::Result<Option<ExpenseWithRelations>> {
    trace!(expense_id = id, "Fetching expense");
    let expense = conn
        .query_row(
            "SELECT e.id, e.date, e.amount_cents, e.currency, e.description,
                    e.category_id, e.notes, e.created_at, e.updated_at,
                    e.value_date, e.payer, e.payee, e.reference, e.transaction_type,
                    e.counterparty_iban, e.creditor_id, e.mandate_reference, e.customer_reference,
                    c.name, c.color
             FROM expenses e
             LEFT JOIN categories c ON e.category_id = c.id
             WHERE e.id = ?",
            [id],
            |row| {
                Ok(ExpenseWithRelations {
                    expense: Expense {
                        id: row.get(0)?,
                        date: row.get(1)?,
                        amount_cents: row.get(2)?,
                        currency: row.get(3)?,
                        description: row.get(4)?,
                        category_id: row.get(5)?,
                        notes: row.get(6)?,
                        created_at: row.get(7)?,
                        updated_at: row.get(8)?,
                        value_date: row.get(9)?,
                        payer: row.get(10)?,
                        payee: row.get(11)?,
                        reference: row.get(12)?,
                        transaction_type: row.get(13)?,
                        counterparty_iban: row.get(14)?,
                        creditor_id: row.get(15)?,
                        mandate_reference: row.get(16)?,
                        customer_reference: row.get(17)?,
                    },
                    category_name: row.get(18)?,
                    category_color: row.get(19)?,
                    tags: Vec::new(),
                })
            },
        )
        .optional()?;

    if let Some(mut exp) = expense {
        exp.tags = get_expense_tags(conn, id)?;
        Ok(Some(exp))
    } else {
        Ok(None)
    }
}

pub fn create_expense(conn: &Connection, expense: &NewExpense) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO expenses (date, amount_cents, currency, description, category_id, notes,
         value_date, payer, payee, reference, transaction_type, counterparty_iban,
         creditor_id, mandate_reference, customer_reference)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        params![
            expense.date,
            expense.amount_cents,
            expense.currency,
            expense.description,
            expense.category_id,
            expense.notes,
            expense.value_date,
            expense.payer,
            expense.payee,
            expense.reference,
            expense.transaction_type,
            expense.counterparty_iban,
            expense.creditor_id,
            expense.mandate_reference,
            expense.customer_reference,
        ],
    )?;

    let id = conn.last_insert_rowid();

    for tag_id in &expense.tag_ids {
        conn.execute(
            "INSERT OR IGNORE INTO expense_tags (expense_id, tag_id) VALUES (?, ?)",
            params![id, tag_id],
        )?;
    }

    debug!(
        expense_id = id,
        amount_cents = expense.amount_cents,
        "Created expense"
    );
    Ok(id)
}

pub fn update_expense(conn: &Connection, id: i64, expense: &NewExpense) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE expenses SET date = ?, amount_cents = ?, currency = ?,
         description = ?, category_id = ?, notes = ?,
         value_date = ?, payer = ?, payee = ?, reference = ?, transaction_type = ?,
         counterparty_iban = ?, creditor_id = ?, mandate_reference = ?, customer_reference = ?,
         updated_at = datetime('now')
         WHERE id = ?",
        params![
            expense.date,
            expense.amount_cents,
            expense.currency,
            expense.description,
            expense.category_id,
            expense.notes,
            expense.value_date,
            expense.payer,
            expense.payee,
            expense.reference,
            expense.transaction_type,
            expense.counterparty_iban,
            expense.creditor_id,
            expense.mandate_reference,
            expense.customer_reference,
            id,
        ],
    )?;

    conn.execute("DELETE FROM expense_tags WHERE expense_id = ?", [id])?;

    for tag_id in &expense.tag_ids {
        conn.execute(
            "INSERT OR IGNORE INTO expense_tags (expense_id, tag_id) VALUES (?, ?)",
            params![id, tag_id],
        )?;
    }

    debug!(expense_id = id, "Updated expense");
    Ok(())
}

pub fn delete_expense(conn: &Connection, id: i64) -> rusqlite::Result<bool> {
    let rows = conn.execute("DELETE FROM expenses WHERE id = ?", [id])?;
    if rows > 0 {
        debug!(expense_id = id, "Deleted expense");
    }
    Ok(rows > 0)
}

pub fn delete_all_expenses(conn: &Connection) -> rusqlite::Result<usize> {
    conn.execute("DELETE FROM expense_tags", [])?;
    let rows = conn.execute("DELETE FROM expenses", [])?;
    tracing::warn!(count = rows, "Deleted all expenses");
    Ok(rows)
}

fn get_expense_tags(conn: &Connection, expense_id: i64) -> rusqlite::Result<Vec<Tag>> {
    let mut stmt = conn.prepare(
        "SELECT t.id, t.name, t.color, t.style, t.created_at
         FROM tags t
         JOIN expense_tags et ON t.id = et.tag_id
         WHERE et.expense_id = ?
         ORDER BY t.name",
    )?;

    let tags = stmt
        .query_map([expense_id], |row| {
            let style_str: String = row.get(3)?;
            Ok(Tag {
                id: row.get(0)?,
                name: row.get(1)?,
                color: row.get(2)?,
                style: TagStyle::parse(&style_str),
                created_at: row.get(4)?,
            })
        })?
        .filter_map(|t| t.ok())
        .collect();

    Ok(tags)
}

fn get_tags_for_expenses(
    conn: &Connection,
    expense_ids: &[i64],
) -> rusqlite::Result<std::collections::HashMap<i64, Vec<Tag>>> {
    use std::collections::HashMap;

    if expense_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let placeholders: String = expense_ids
        .iter()
        .map(|_| "?")
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        "SELECT et.expense_id, t.id, t.name, t.color, t.style, t.created_at
         FROM tags t
         JOIN expense_tags et ON t.id = et.tag_id
         WHERE et.expense_id IN ({})
         ORDER BY t.name",
        placeholders
    );

    let mut stmt = conn.prepare(&sql)?;
    let params: Vec<&dyn rusqlite::ToSql> = expense_ids
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
    for row in rows.filter_map(|r| r.ok()) {
        tags_map.entry(row.0).or_default().push(row.1);
    }

    Ok(tags_map)
}
