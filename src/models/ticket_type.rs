use rusqlite::{Connection, params};

pub struct TicketType {
    pub id: i64,
    pub name: String,
    pub description: String,
}

pub struct CustomField {
    pub id: i64,
    pub ticket_type_id: i64,
    pub name: String,
    pub field_type: String,
    pub is_required: bool,
    pub position: i64,
    pub num_min: Option<f64>,
    pub num_max: Option<f64>,
    pub num_step: Option<f64>,
}

pub fn list_all(conn: &Connection) -> Vec<TicketType> {
    let mut stmt = conn
        .prepare("SELECT id, name, description FROM ticket_types ORDER BY name")
        .unwrap();
    stmt.query_map([], |row| {
        Ok(TicketType {
            id: row.get(0)?,
            name: row.get(1)?,
            description: row.get(2)?,
        })
    })
    .unwrap()
    .filter_map(|r| r.ok())
    .collect()
}

pub fn find_by_id(conn: &Connection, id: i64) -> Option<TicketType> {
    conn.query_row(
        "SELECT id, name, description FROM ticket_types WHERE id = ?1",
        [id],
        |row| {
            Ok(TicketType {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
            })
        },
    )
    .ok()
}

pub fn create(conn: &Connection, name: &str, description: &str) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO ticket_types (name, description) VALUES (?1, ?2)",
        params![name, description],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn update(conn: &Connection, id: i64, name: &str, description: &str) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE ticket_types SET name = ?1, description = ?2 WHERE id = ?3",
        params![name, description, id],
    )?;
    Ok(())
}

pub fn delete(conn: &Connection, id: i64) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM ticket_types WHERE id = ?1", [id])?;
    Ok(())
}

pub fn list_fields(conn: &Connection, ticket_type_id: i64) -> Vec<CustomField> {
    let mut stmt = conn
        .prepare(
            "SELECT id, ticket_type_id, name, field_type, is_required, position, num_min, num_max, num_step
             FROM custom_fields WHERE ticket_type_id = ?1 ORDER BY position, name",
        )
        .unwrap();
    stmt.query_map([ticket_type_id], |row| {
        Ok(CustomField {
            id: row.get(0)?,
            ticket_type_id: row.get(1)?,
            name: row.get(2)?,
            field_type: row.get(3)?,
            is_required: row.get::<_, i64>(4)? != 0,
            position: row.get(5)?,
            num_min: row.get(6)?,
            num_max: row.get(7)?,
            num_step: row.get(8)?,
        })
    })
    .unwrap()
    .filter_map(|r| r.ok())
    .collect()
}

pub fn add_field(
    conn: &Connection,
    ticket_type_id: i64,
    name: &str,
    field_type: &str,
    is_required: bool,
    position: i64,
    num_min: Option<f64>,
    num_max: Option<f64>,
    num_step: Option<f64>,
) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO custom_fields (ticket_type_id, name, field_type, is_required, position, num_min, num_max, num_step)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![ticket_type_id, name, field_type, is_required as i64, position, num_min, num_max, num_step],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn delete_field(conn: &Connection, field_id: i64) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM custom_fields WHERE id = ?1", [field_id])?;
    Ok(())
}
