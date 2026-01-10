use crate::models::tag::{NewTag, Tag, TagStyle};
use rusqlite::{params, Connection, OptionalExtension};

pub fn list_tags(conn: &Connection) -> rusqlite::Result<Vec<Tag>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, color, style, created_at
         FROM tags
         ORDER BY name",
    )?;

    let tags = stmt
        .query_map([], |row| {
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

pub fn search_tags(conn: &Connection, query: &str) -> rusqlite::Result<Vec<Tag>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, color, style, created_at
         FROM tags
         WHERE name LIKE ?
         ORDER BY name
         LIMIT 10",
    )?;

    let tags = stmt
        .query_map([format!("%{}%", query)], |row| {
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

pub fn get_tag(conn: &Connection, id: i64) -> rusqlite::Result<Option<Tag>> {
    conn.query_row(
        "SELECT id, name, color, style, created_at FROM tags WHERE id = ?",
        [id],
        |row| {
            let style_str: String = row.get(3)?;
            Ok(Tag {
                id: row.get(0)?,
                name: row.get(1)?,
                color: row.get(2)?,
                style: TagStyle::parse(&style_str),
                created_at: row.get(4)?,
            })
        },
    )
    .optional()
}

pub fn get_tag_by_name(conn: &Connection, name: &str) -> rusqlite::Result<Option<Tag>> {
    conn.query_row(
        "SELECT id, name, color, style, created_at FROM tags WHERE name = ?",
        [name],
        |row| {
            let style_str: String = row.get(3)?;
            Ok(Tag {
                id: row.get(0)?,
                name: row.get(1)?,
                color: row.get(2)?,
                style: TagStyle::parse(&style_str),
                created_at: row.get(4)?,
            })
        },
    )
    .optional()
}

pub fn create_tag(conn: &Connection, tag: &NewTag) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO tags (name, color, style) VALUES (?, ?, ?)",
        params![tag.name, tag.color, tag.style.as_str()],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn create_or_get_tag(conn: &Connection, name: &str) -> rusqlite::Result<Tag> {
    if let Some(existing) = get_tag_by_name(conn, name)? {
        return Ok(existing);
    }

    let tag = NewTag {
        name: name.to_string(),
        color: "#6b7280".to_string(),
        style: TagStyle::Solid,
    };
    let id = create_tag(conn, &tag)?;

    Ok(Tag {
        id,
        name: tag.name,
        color: tag.color,
        style: tag.style,
        created_at: String::new(),
    })
}

pub fn update_tag(conn: &Connection, id: i64, tag: &NewTag) -> rusqlite::Result<bool> {
    let rows = conn.execute(
        "UPDATE tags SET name = ?, color = ?, style = ? WHERE id = ?",
        params![tag.name, tag.color, tag.style.as_str(), id],
    )?;
    Ok(rows > 0)
}

pub fn delete_tag(conn: &Connection, id: i64) -> rusqlite::Result<bool> {
    let rows = conn.execute("DELETE FROM tags WHERE id = ?", [id])?;
    Ok(rows > 0)
}
