use rusqlite::{Connection, params};
use ticketsystem_core::models::notification::Notification;

/// Notifications older than this are removed whenever a new one is created.
const RETENTION_DAYS: i64 = 30;

pub fn create(
    conn: &Connection,
    user_id: i64,
    ticket_id: i64,
    kind: &str,
    actor_id: Option<i64>,
) -> rusqlite::Result<i64> {
    conn.execute(
        "DELETE FROM notifications WHERE created_at < datetime('now', ?1)",
        [format!("-{RETENTION_DAYS} days")],
    )?;
    conn.execute(
        "INSERT INTO notifications (user_id, ticket_id, kind, actor_id) VALUES (?1, ?2, ?3, ?4)",
        params![user_id, ticket_id, kind, actor_id],
    )?;
    Ok(conn.last_insert_rowid())
}

/// The newest notification id of a user (0 if none), used as the starting point for polling.
pub fn latest_id(conn: &Connection, user_id: i64) -> i64 {
    conn.query_row(
        "SELECT COALESCE(MAX(id), 0) FROM notifications WHERE user_id = ?1",
        [user_id],
        |row| row.get(0),
    )
    .unwrap_or(0)
}

/// Notifications of a user newer than `after_id`, oldest first. Deleted tickets are skipped.
pub fn list_after(conn: &Connection, user_id: i64, after_id: i64, limit: i64) -> Vec<Notification> {
    let mut stmt = conn
        .prepare(
            "SELECT n.id, n.ticket_id, t.project_id, p.name, t.title, n.kind, a.username, n.created_at
             FROM notifications n
             JOIN tickets t ON t.id = n.ticket_id AND t.is_deleted = 0
             JOIN projects p ON p.id = t.project_id
             LEFT JOIN users a ON a.id = n.actor_id
             WHERE n.user_id = ?1 AND n.id > ?2
             ORDER BY n.id
             LIMIT ?3",
        )
        .unwrap();
    stmt.query_map(params![user_id, after_id, limit], |row| {
        Ok(Notification {
            id: row.get(0)?,
            ticket_id: row.get(1)?,
            project_id: row.get(2)?,
            project_name: row.get(3)?,
            ticket_title: row.get(4)?,
            kind: row.get(5)?,
            actor_name: row.get(6)?,
            created_at: row.get(7)?,
        })
    })
    .unwrap()
    .filter_map(|r| r.ok())
    .collect()
}
