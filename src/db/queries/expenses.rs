use crate::models::expense::{Expense, ExpenseWithRelations, NewExpense};
use crate::models::tag::Tag;
use rusqlite::{params, Connection, OptionalExtension};

#[derive(Default)]
pub struct ExpenseFilter {
    pub search: Option<String>,
    pub category_id: Option<i64>,
    pub tag_id: Option<i64>,
    pub from_date: Option<String>,
    pub to_date: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

pub fn list_expenses(
    conn: &Connection,
    filter: &ExpenseFilter,
) -> rusqlite::Result<Vec<ExpenseWithRelations>> {
    let mut sql = String::from(
        "SELECT e.id, e.date, e.amount_cents, e.currency, e.description,
                e.category_id, e.notes, e.created_at, e.updated_at,
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

    sql.push_str(" ORDER BY e.date DESC, e.id DESC");

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
            },
            category_name: row.get(9)?,
            category_color: row.get(10)?,
            tags: Vec::new(),
        })
    })?;

    let mut expenses: Vec<ExpenseWithRelations> = expense_iter.filter_map(|e| e.ok()).collect();

    for expense in &mut expenses {
        expense.tags = get_expense_tags(conn, expense.expense.id)?;
    }

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
    let expense = conn
        .query_row(
            "SELECT e.id, e.date, e.amount_cents, e.currency, e.description,
                    e.category_id, e.notes, e.created_at, e.updated_at,
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
                    },
                    category_name: row.get(9)?,
                    category_color: row.get(10)?,
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
        "INSERT INTO expenses (date, amount_cents, currency, description, category_id, notes)
         VALUES (?, ?, ?, ?, ?, ?)",
        params![
            expense.date,
            expense.amount_cents,
            expense.currency,
            expense.description,
            expense.category_id,
            expense.notes,
        ],
    )?;

    let id = conn.last_insert_rowid();

    for tag_id in &expense.tag_ids {
        conn.execute(
            "INSERT OR IGNORE INTO expense_tags (expense_id, tag_id) VALUES (?, ?)",
            params![id, tag_id],
        )?;
    }

    Ok(id)
}

pub fn update_expense(conn: &Connection, id: i64, expense: &NewExpense) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE expenses SET date = ?, amount_cents = ?, currency = ?,
         description = ?, category_id = ?, notes = ?, updated_at = datetime('now')
         WHERE id = ?",
        params![
            expense.date,
            expense.amount_cents,
            expense.currency,
            expense.description,
            expense.category_id,
            expense.notes,
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

    Ok(())
}

pub fn delete_expense(conn: &Connection, id: i64) -> rusqlite::Result<bool> {
    let rows = conn.execute("DELETE FROM expenses WHERE id = ?", [id])?;
    Ok(rows > 0)
}

fn get_expense_tags(conn: &Connection, expense_id: i64) -> rusqlite::Result<Vec<Tag>> {
    let mut stmt = conn.prepare(
        "SELECT t.id, t.name, t.color, t.created_at
         FROM tags t
         JOIN expense_tags et ON t.id = et.tag_id
         WHERE et.expense_id = ?
         ORDER BY t.name",
    )?;

    let tags = stmt
        .query_map([expense_id], |row| {
            Ok(Tag {
                id: row.get(0)?,
                name: row.get(1)?,
                color: row.get(2)?,
                created_at: row.get(3)?,
            })
        })?
        .filter_map(|t| t.ok())
        .collect();

    Ok(tags)
}
