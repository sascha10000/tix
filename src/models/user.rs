use rusqlite::{Connection, params};

pub struct User {
    pub id: i64,
    pub username: String,
    pub email: String,
    pub password_hash: String,
    pub is_admin: bool,
    pub is_manager: bool,
    pub created_at: String,
    pub is_active: bool,
}

fn row_to_user(row: &rusqlite::Row) -> rusqlite::Result<User> {
    Ok(User {
        id: row.get(0)?,
        username: row.get(1)?,
        email: row.get(2)?,
        password_hash: row.get(3)?,
        is_admin: row.get::<_, i64>(4)? != 0,
        is_manager: row.get::<_, i64>(5)? != 0,
        created_at: row.get(6)?,
        is_active: row.get::<_, i64>(7)? != 0,
    })
}

const SELECT_COLS: &str = "id, username, email, password_hash, is_admin, is_manager, created_at, is_active";

pub fn find_by_username(conn: &Connection, username: &str) -> Option<User> {
    conn.query_row(
        &format!("SELECT {SELECT_COLS} FROM users WHERE username = ?1"),
        [username],
        row_to_user,
    )
    .ok()
}

pub fn find_by_id(conn: &Connection, id: i64) -> Option<User> {
    conn.query_row(
        &format!("SELECT {SELECT_COLS} FROM users WHERE id = ?1"),
        [id],
        row_to_user,
    )
    .ok()
}

pub fn list_all(conn: &Connection) -> Vec<User> {
    let mut stmt = conn
        .prepare(&format!("SELECT {SELECT_COLS} FROM users ORDER BY username"))
        .unwrap();
    stmt.query_map([], row_to_user)
        .unwrap()
        .filter_map(|r| r.ok())
        .collect()
}

pub fn create(conn: &Connection, username: &str, email: &str, password_hash: &str, is_admin: bool, is_manager: bool) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO users (username, email, password_hash, is_admin, is_manager) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![username, email, password_hash, is_admin as i64, is_manager as i64],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn set_manager(conn: &Connection, id: i64, is_manager: bool) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE users SET is_manager = ?1 WHERE id = ?2",
        params![is_manager as i64, id],
    )?;
    Ok(())
}

pub fn update_profile(conn: &Connection, id: i64, username: &str, email: &str) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE users SET username = ?1, email = ?2 WHERE id = ?3",
        params![username, email, id],
    )?;
    Ok(())
}

pub fn update_password(conn: &Connection, id: i64, password_hash: &str) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE users SET password_hash = ?1 WHERE id = ?2",
        params![password_hash, id],
    )?;
    Ok(())
}

pub fn set_admin(conn: &Connection, id: i64, is_admin: bool) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE users SET is_admin = ?1 WHERE id = ?2",
        params![is_admin as i64, id],
    )?;
    Ok(())
}

pub fn set_active(conn: &Connection, id: i64, active: bool) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE users SET is_active = ?1 WHERE id = ?2",
        params![active as i64, id],
    )?;
    Ok(())
}

pub fn create_session(conn: &Connection, user_id: i64, duration_hours: i64) -> rusqlite::Result<String> {
    let session_id = uuid::Uuid::new_v4().to_string();
    let modifier = format!("+{duration_hours} hours");
    conn.execute(
        "INSERT INTO sessions (id, user_id, expires_at) VALUES (?1, ?2, datetime('now', ?3))",
        params![session_id, user_id, modifier],
    )?;
    Ok(session_id)
}

pub fn delete_session(conn: &Connection, session_id: &str) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM sessions WHERE id = ?1", [session_id])?;
    Ok(())
}
