use rusqlite::{Connection, params};

pub struct Project {
    pub id: i64,
    pub name: String,
    pub description: String,
    pub created_by: Option<i64>,
    pub created_at: String,
}

pub struct ProjectMember {
    pub user_id: i64,
    pub username: String,
    pub role: String,
}

fn row_to_project(row: &rusqlite::Row) -> rusqlite::Result<Project> {
    Ok(Project {
        id: row.get(0)?,
        name: row.get(1)?,
        description: row.get(2)?,
        created_by: row.get(3)?,
        created_at: row.get(4)?,
    })
}

const SELECT_COLS: &str = "id, name, description, created_by, created_at";

pub fn list_all(conn: &Connection) -> Vec<Project> {
    let mut stmt = conn
        .prepare(&format!("SELECT {SELECT_COLS} FROM projects ORDER BY name"))
        .unwrap();
    stmt.query_map([], row_to_project)
        .unwrap()
        .filter_map(|r| r.ok())
        .collect()
}

pub fn list_for_user(conn: &Connection, user_id: i64) -> Vec<Project> {
    let mut stmt = conn
        .prepare(&format!(
            "SELECT p.{} FROM projects p
             JOIN project_members pm ON pm.project_id = p.id
             WHERE pm.user_id = ?1 ORDER BY p.name",
            SELECT_COLS.replace(", ", ", p.")
        ))
        .unwrap();
    stmt.query_map([user_id], row_to_project)
        .unwrap()
        .filter_map(|r| r.ok())
        .collect()
}

pub fn find_by_id(conn: &Connection, id: i64) -> Option<Project> {
    conn.query_row(
        &format!("SELECT {SELECT_COLS} FROM projects WHERE id = ?1"),
        [id],
        row_to_project,
    )
    .ok()
}

pub fn create(conn: &Connection, name: &str, description: &str, created_by: i64) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO projects (name, description, created_by) VALUES (?1, ?2, ?3)",
        params![name, description, created_by],
    )?;
    let id = conn.last_insert_rowid();
    // Auto-add creator as manager
    add_member(conn, id, created_by, "manager")?;
    Ok(id)
}

pub fn update(conn: &Connection, id: i64, name: &str, description: &str) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE projects SET name = ?1, description = ?2 WHERE id = ?3",
        params![name, description, id],
    )?;
    Ok(())
}

pub fn delete(conn: &Connection, id: i64) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM projects WHERE id = ?1", [id])?;
    Ok(())
}

/// Returns the user's role in the project, or None if not a member.
pub fn get_member_role(conn: &Connection, project_id: i64, user_id: i64) -> Option<String> {
    conn.query_row(
        "SELECT role FROM project_members WHERE project_id = ?1 AND user_id = ?2",
        params![project_id, user_id],
        |row| row.get(0),
    )
    .ok()
}

pub fn is_member(conn: &Connection, project_id: i64, user_id: i64) -> bool {
    get_member_role(conn, project_id, user_id).is_some()
}

pub fn add_member(conn: &Connection, project_id: i64, user_id: i64, role: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO project_members (project_id, user_id, role) VALUES (?1, ?2, ?3)
         ON CONFLICT(project_id, user_id) DO UPDATE SET role = ?3",
        params![project_id, user_id, role],
    )?;
    Ok(())
}

pub fn remove_member(conn: &Connection, project_id: i64, user_id: i64) -> rusqlite::Result<()> {
    conn.execute(
        "DELETE FROM project_members WHERE project_id = ?1 AND user_id = ?2",
        params![project_id, user_id],
    )?;
    Ok(())
}

pub fn list_members(conn: &Connection, project_id: i64) -> Vec<ProjectMember> {
    let mut stmt = conn
        .prepare(
            "SELECT pm.user_id, u.username, pm.role FROM project_members pm
             JOIN users u ON u.id = pm.user_id
             WHERE pm.project_id = ?1 ORDER BY u.username",
        )
        .unwrap();
    stmt.query_map([project_id], |row| {
        Ok(ProjectMember {
            user_id: row.get(0)?,
            username: row.get(1)?,
            role: row.get(2)?,
        })
    })
    .unwrap()
    .filter_map(|r| r.ok())
    .collect()
}

pub fn list_member_ids(conn: &Connection, project_id: i64) -> Vec<i64> {
    let mut stmt = conn
        .prepare("SELECT user_id FROM project_members WHERE project_id = ?1")
        .unwrap();
    stmt.query_map([project_id], |row| row.get(0))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect()
}

pub fn list_active_status_ids(conn: &Connection, project_id: i64) -> Vec<i64> {
    let mut stmt = conn
        .prepare("SELECT status_id FROM project_statuses WHERE project_id = ?1")
        .unwrap();
    stmt.query_map([project_id], |row| row.get(0))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect()
}

pub fn toggle_status(conn: &Connection, project_id: i64, status_id: i64) -> rusqlite::Result<()> {
    let exists = conn
        .query_row(
            "SELECT 1 FROM project_statuses WHERE project_id = ?1 AND status_id = ?2",
            params![project_id, status_id],
            |_| Ok(()),
        )
        .is_ok();
    if exists {
        conn.execute(
            "DELETE FROM project_statuses WHERE project_id = ?1 AND status_id = ?2",
            params![project_id, status_id],
        )?;
    } else {
        conn.execute(
            "INSERT INTO project_statuses (project_id, status_id) VALUES (?1, ?2)",
            params![project_id, status_id],
        )?;
    }
    Ok(())
}

pub fn list_active_ticket_type_ids(conn: &Connection, project_id: i64) -> Vec<i64> {
    let mut stmt = conn
        .prepare("SELECT ticket_type_id FROM project_ticket_types WHERE project_id = ?1")
        .unwrap();
    stmt.query_map([project_id], |row| row.get(0))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect()
}

pub fn toggle_ticket_type(conn: &Connection, project_id: i64, ticket_type_id: i64) -> rusqlite::Result<()> {
    let exists = conn
        .query_row(
            "SELECT 1 FROM project_ticket_types WHERE project_id = ?1 AND ticket_type_id = ?2",
            params![project_id, ticket_type_id],
            |_| Ok(()),
        )
        .is_ok();
    if exists {
        conn.execute(
            "DELETE FROM project_ticket_types WHERE project_id = ?1 AND ticket_type_id = ?2",
            params![project_id, ticket_type_id],
        )?;
    } else {
        conn.execute(
            "INSERT INTO project_ticket_types (project_id, ticket_type_id) VALUES (?1, ?2)",
            params![project_id, ticket_type_id],
        )?;
    }
    Ok(())
}
