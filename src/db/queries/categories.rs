use crate::models::category::{Category, CategoryWithPath, NewCategory};
use rusqlite::{params, Connection, OptionalExtension};
use tracing::{debug, warn};

pub fn list_categories(conn: &Connection) -> rusqlite::Result<Vec<Category>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, parent_id, color, icon, created_at, updated_at
         FROM categories
         ORDER BY name",
    )?;

    let categories = stmt
        .query_map([], |row| {
            Ok(Category {
                id: row.get(0)?,
                name: row.get(1)?,
                parent_id: row.get(2)?,
                color: row.get(3)?,
                icon: row.get(4)?,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
            })
        })?
        .filter_map(|c| c.ok())
        .collect();

    Ok(categories)
}

pub fn list_categories_with_path(conn: &Connection) -> rusqlite::Result<Vec<CategoryWithPath>> {
    let mut stmt = conn.prepare(
        "WITH RECURSIVE category_path AS (
            SELECT id, name, parent_id, color, icon, created_at, updated_at,
                   name as path, 0 as depth
            FROM categories WHERE parent_id IS NULL
            UNION ALL
            SELECT c.id, c.name, c.parent_id, c.color, c.icon, c.created_at, c.updated_at,
                   cp.path || ' > ' || c.name, cp.depth + 1
            FROM categories c
            JOIN category_path cp ON c.parent_id = cp.id
        )
        SELECT id, name, parent_id, color, icon, created_at, updated_at, path, depth
        FROM category_path
        ORDER BY path",
    )?;

    let categories = stmt
        .query_map([], |row| {
            Ok(CategoryWithPath {
                category: Category {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    parent_id: row.get(2)?,
                    color: row.get(3)?,
                    icon: row.get(4)?,
                    created_at: row.get(5)?,
                    updated_at: row.get(6)?,
                },
                path: row.get(7)?,
                depth: row.get(8)?,
            })
        })?
        .filter_map(|c| c.ok())
        .collect();

    Ok(categories)
}

pub fn get_category(conn: &Connection, id: i64) -> rusqlite::Result<Option<Category>> {
    conn.query_row(
        "SELECT id, name, parent_id, color, icon, created_at, updated_at
         FROM categories WHERE id = ?",
        [id],
        |row| {
            Ok(Category {
                id: row.get(0)?,
                name: row.get(1)?,
                parent_id: row.get(2)?,
                color: row.get(3)?,
                icon: row.get(4)?,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
            })
        },
    )
    .optional()
}

pub fn create_category(conn: &Connection, category: &NewCategory) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO categories (name, parent_id, color, icon) VALUES (?, ?, ?, ?)",
        params![
            category.name,
            category.parent_id,
            category.color,
            category.icon
        ],
    )?;
    let id = conn.last_insert_rowid();
    debug!(category_id = id, name = %category.name, "Created category");
    Ok(id)
}

pub fn update_category(
    conn: &Connection,
    id: i64,
    category: &NewCategory,
) -> rusqlite::Result<bool> {
    let rows = conn.execute(
        "UPDATE categories SET name = ?, parent_id = ?, color = ?, icon = ?,
         updated_at = datetime('now') WHERE id = ?",
        params![
            category.name,
            category.parent_id,
            category.color,
            category.icon,
            id
        ],
    )?;
    if rows > 0 {
        debug!(category_id = id, name = %category.name, "Updated category");
    }
    Ok(rows > 0)
}

pub fn delete_category(conn: &Connection, id: i64) -> rusqlite::Result<bool> {
    let rows = conn.execute("DELETE FROM categories WHERE id = ?", [id])?;
    if rows > 0 {
        debug!(category_id = id, "Deleted category");
    }
    Ok(rows > 0)
}

pub fn get_top_level_categories(conn: &Connection) -> rusqlite::Result<Vec<Category>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, parent_id, color, icon, created_at, updated_at
         FROM categories
         WHERE parent_id IS NULL
         ORDER BY name",
    )?;

    let categories = stmt
        .query_map([], |row| {
            Ok(Category {
                id: row.get(0)?,
                name: row.get(1)?,
                parent_id: row.get(2)?,
                color: row.get(3)?,
                icon: row.get(4)?,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
            })
        })?
        .filter_map(|c| c.ok())
        .collect();

    Ok(categories)
}

pub fn delete_all_categories(conn: &Connection) -> rusqlite::Result<usize> {
    let rows = conn.execute("DELETE FROM categories", [])?;
    warn!(count = rows, "Deleted all categories");
    Ok(rows)
}

pub fn get_child_categories(conn: &Connection, parent_id: i64) -> rusqlite::Result<Vec<Category>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, parent_id, color, icon, created_at, updated_at
         FROM categories
         WHERE parent_id = ?
         ORDER BY name",
    )?;

    let categories = stmt
        .query_map([parent_id], |row| {
            Ok(Category {
                id: row.get(0)?,
                name: row.get(1)?,
                parent_id: row.get(2)?,
                color: row.get(3)?,
                icon: row.get(4)?,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
            })
        })?
        .filter_map(|c| c.ok())
        .collect();

    Ok(categories)
}
