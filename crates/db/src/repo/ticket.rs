use rusqlite::{Connection, params};
use ticketsystem_core::models::ticket::{FieldValue, Ticket};

pub fn list_for_project(conn: &Connection, project_id: i64) -> Vec<Ticket> {
    let mut stmt = conn
        .prepare(
            "SELECT t.id, t.project_id, t.ticket_type_id, t.status_id, t.creator_id,
                    t.assignee_id, t.title, t.text, t.due_date, t.created_at, t.updated_at, t.is_deleted,
                    u.username, a.username, s.name, s.color, tt.name
             FROM tickets t
             JOIN users u ON u.id = t.creator_id
             JOIN users a ON a.id = t.assignee_id
             JOIN statuses s ON s.id = t.status_id
             JOIN ticket_types tt ON tt.id = t.ticket_type_id
             WHERE t.project_id = ?1 AND t.is_deleted = 0
             ORDER BY t.updated_at DESC",
        )
        .unwrap();
    stmt.query_map([project_id], |row| {
        Ok(Ticket {
            id: row.get(0)?,
            project_id: row.get(1)?,
            ticket_type_id: row.get(2)?,
            status_id: row.get(3)?,
            creator_id: row.get(4)?,
            assignee_id: row.get(5)?,
            title: row.get(6)?,
            text: row.get(7)?,
            due_date: row.get(8)?,
            created_at: row.get(9)?,
            updated_at: row.get(10)?,
            is_deleted: row.get::<_, i64>(11)? != 0,
            creator_name: row.get(12)?,
            assignee_name: row.get(13)?,
            status_name: row.get(14)?,
            status_color: row.get(15)?,
            type_name: row.get(16)?,
        })
    })
    .unwrap()
    .filter_map(|r| r.ok())
    .collect()
}

pub fn list_for_user(conn: &Connection, user_id: i64) -> Vec<(Ticket, String)> {
    let mut stmt = conn
        .prepare(
            "SELECT t.id, t.project_id, t.ticket_type_id, t.status_id, t.creator_id,
                    t.assignee_id, t.title, t.text, t.due_date, t.created_at, t.updated_at, t.is_deleted,
                    u.username, a.username, s.name, s.color, tt.name, p.name
             FROM tickets t
             JOIN users u ON u.id = t.creator_id
             JOIN users a ON a.id = t.assignee_id
             JOIN statuses s ON s.id = t.status_id
             JOIN ticket_types tt ON tt.id = t.ticket_type_id
             JOIN projects p ON p.id = t.project_id
             WHERE (t.assignee_id = ?1 OR t.creator_id = ?1) AND t.is_deleted = 0
             ORDER BY t.updated_at DESC",
        )
        .unwrap();
    stmt.query_map([user_id], |row| {
        Ok((
            Ticket {
                id: row.get(0)?,
                project_id: row.get(1)?,
                ticket_type_id: row.get(2)?,
                status_id: row.get(3)?,
                creator_id: row.get(4)?,
                assignee_id: row.get(5)?,
                title: row.get(6)?,
                text: row.get(7)?,
                due_date: row.get(8)?,
                created_at: row.get(9)?,
                updated_at: row.get(10)?,
                is_deleted: row.get::<_, i64>(11)? != 0,
                creator_name: row.get(12)?,
                assignee_name: row.get(13)?,
                status_name: row.get(14)?,
                status_color: row.get(15)?,
                type_name: row.get(16)?,
            },
            row.get(17)?,
        ))
    })
    .unwrap()
    .filter_map(|r| r.ok())
    .collect()
}

pub fn find_by_id(conn: &Connection, id: i64) -> Option<Ticket> {
    conn.query_row(
        "SELECT t.id, t.project_id, t.ticket_type_id, t.status_id, t.creator_id,
                t.assignee_id, t.title, t.text, t.due_date, t.created_at, t.updated_at, t.is_deleted,
                u.username, a.username, s.name, s.color, tt.name
         FROM tickets t
         JOIN users u ON u.id = t.creator_id
         JOIN users a ON a.id = t.assignee_id
         JOIN statuses s ON s.id = t.status_id
         JOIN ticket_types tt ON tt.id = t.ticket_type_id
         WHERE t.id = ?1",
        [id],
        |row| {
            Ok(Ticket {
                id: row.get(0)?,
                project_id: row.get(1)?,
                ticket_type_id: row.get(2)?,
                status_id: row.get(3)?,
                creator_id: row.get(4)?,
                assignee_id: row.get(5)?,
                title: row.get(6)?,
                text: row.get(7)?,
                due_date: row.get(8)?,
                created_at: row.get(9)?,
                updated_at: row.get(10)?,
                is_deleted: row.get::<_, i64>(11)? != 0,
                creator_name: row.get(12)?,
                assignee_name: row.get(13)?,
                status_name: row.get(14)?,
                status_color: row.get(15)?,
                type_name: row.get(16)?,
            })
        },
    )
    .ok()
}

pub fn create(
    conn: &Connection,
    project_id: i64,
    ticket_type_id: i64,
    status_id: i64,
    creator_id: i64,
    assignee_id: i64,
    title: &str,
    text: &str,
    due_date: &str,
) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO tickets (project_id, ticket_type_id, status_id, creator_id, assignee_id, title, text, due_date)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![project_id, ticket_type_id, status_id, creator_id, assignee_id, title, text, due_date],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn update(conn: &Connection, id: i64, title: &str, text: &str, assignee_id: i64, due_date: &str) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE tickets SET title = ?1, text = ?2, assignee_id = ?3, due_date = ?4, updated_at = datetime('now') WHERE id = ?5",
        params![title, text, assignee_id, due_date, id],
    )?;
    Ok(())
}

pub fn transition_status(conn: &Connection, id: i64, new_status_id: i64) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE tickets SET status_id = ?1, updated_at = datetime('now') WHERE id = ?2",
        params![new_status_id, id],
    )?;
    Ok(())
}

pub fn soft_delete(conn: &Connection, id: i64) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE tickets SET is_deleted = 1, updated_at = datetime('now') WHERE id = ?1",
        [id],
    )?;
    Ok(())
}

pub fn set_field_value(conn: &Connection, ticket_id: i64, custom_field_id: i64, value: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO ticket_field_values (ticket_id, custom_field_id, value)
         VALUES (?1, ?2, ?3)
         ON CONFLICT(ticket_id, custom_field_id) DO UPDATE SET value = ?3",
        params![ticket_id, custom_field_id, value],
    )?;
    Ok(())
}

pub fn get_field_values(conn: &Connection, ticket_id: i64) -> Vec<FieldValue> {
    let mut stmt = conn
        .prepare(
            "SELECT cf.id, cf.name, cf.field_type, COALESCE(fv.value, ''), cf.is_required,
                    cf.num_min, cf.num_max, cf.num_step, cf.placeholder, cf.default_value
             FROM custom_fields cf
             JOIN tickets t ON t.ticket_type_id = cf.ticket_type_id AND t.id = ?1
             LEFT JOIN ticket_field_values fv ON fv.custom_field_id = cf.id AND fv.ticket_id = ?1
             ORDER BY cf.position, cf.name",
        )
        .unwrap();
    stmt.query_map([ticket_id], |row| {
        Ok(FieldValue {
            custom_field_id: row.get(0)?,
            field_name: row.get(1)?,
            field_type: row.get(2)?,
            value: row.get(3)?,
            is_required: row.get::<_, i64>(4)? != 0,
            num_min: row.get(5)?,
            num_max: row.get(6)?,
            num_step: row.get(7)?,
            placeholder: row.get(8)?,
            default_value: row.get(9)?,
        })
    })
    .unwrap()
    .filter_map(|r| r.ok())
    .collect()
}
