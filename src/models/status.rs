use rusqlite::{Connection, params};

pub struct Status {
    pub id: i64,
    pub name: String,
    pub color: String,
    pub position: i64,
}

pub struct WorkflowTransition {
    pub from_status_id: i64,
    pub to_status_id: i64,
}

pub fn list_all(conn: &Connection) -> Vec<Status> {
    let mut stmt = conn
        .prepare("SELECT id, name, color, position FROM statuses ORDER BY position, name")
        .unwrap();
    stmt.query_map([], |row| {
        Ok(Status {
            id: row.get(0)?,
            name: row.get(1)?,
            color: row.get(2)?,
            position: row.get(3)?,
        })
    })
    .unwrap()
    .filter_map(|r| r.ok())
    .collect()
}

pub fn find_by_id(conn: &Connection, id: i64) -> Option<Status> {
    conn.query_row(
        "SELECT id, name, color, position FROM statuses WHERE id = ?1",
        [id],
        |row| {
            Ok(Status {
                id: row.get(0)?,
                name: row.get(1)?,
                color: row.get(2)?,
                position: row.get(3)?,
            })
        },
    )
    .ok()
}

pub fn create(conn: &Connection, name: &str, color: &str, position: i64) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO statuses (name, color, position) VALUES (?1, ?2, ?3)",
        params![name, color, position],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn update(conn: &Connection, id: i64, name: &str, color: &str, position: i64) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE statuses SET name = ?1, color = ?2, position = ?3 WHERE id = ?4",
        params![name, color, position, id],
    )?;
    Ok(())
}

pub fn delete(conn: &Connection, id: i64) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM statuses WHERE id = ?1", [id])?;
    Ok(())
}

pub fn list_workflows(conn: &Connection) -> Vec<WorkflowTransition> {
    let mut stmt = conn
        .prepare("SELECT from_status_id, to_status_id FROM workflows")
        .unwrap();
    stmt.query_map([], |row| {
        Ok(WorkflowTransition {
            from_status_id: row.get(0)?,
            to_status_id: row.get(1)?,
        })
    })
    .unwrap()
    .filter_map(|r| r.ok())
    .collect()
}

pub fn has_transition(conn: &Connection, from_id: i64, to_id: i64) -> bool {
    conn.query_row(
        "SELECT 1 FROM workflows WHERE from_status_id = ?1 AND to_status_id = ?2",
        params![from_id, to_id],
        |_| Ok(()),
    )
    .is_ok()
}

pub fn toggle_workflow(conn: &Connection, from_id: i64, to_id: i64) -> rusqlite::Result<()> {
    if has_transition(conn, from_id, to_id) {
        conn.execute(
            "DELETE FROM workflows WHERE from_status_id = ?1 AND to_status_id = ?2",
            params![from_id, to_id],
        )?;
    } else {
        conn.execute(
            "INSERT INTO workflows (from_status_id, to_status_id) VALUES (?1, ?2)",
            params![from_id, to_id],
        )?;
    }
    Ok(())
}
