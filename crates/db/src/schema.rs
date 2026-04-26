/// Current schema version. Must match the number of migrations in migrations.rs.
const SCHEMA_VERSION: i64 = 6;

/// Initial schema -- runs only when tables don't exist yet.
/// This reflects the LATEST schema (post all migrations).
/// When tables are freshly created, user_version is set to SCHEMA_VERSION
/// so that migrations are skipped (they're only for upgrading old databases).
pub(crate) fn create_schema(conn: &rusqlite::Connection) {
    let table_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'users'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    if table_count > 0 {
        return;
    }

    conn.execute_batch(
        "
        CREATE TABLE users (
            id            INTEGER PRIMARY KEY AUTOINCREMENT,
            username      TEXT NOT NULL UNIQUE,
            email         TEXT NOT NULL UNIQUE,
            password_hash TEXT NOT NULL,
            is_admin      INTEGER NOT NULL DEFAULT 0,
            is_manager    INTEGER NOT NULL DEFAULT 0,
            created_at    TEXT NOT NULL DEFAULT (datetime('now')),
            is_active     INTEGER NOT NULL DEFAULT 1
        );

        CREATE TABLE sessions (
            id         TEXT PRIMARY KEY,
            user_id    INTEGER NOT NULL REFERENCES users(id),
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            expires_at TEXT NOT NULL
        );

        CREATE TABLE statuses (
            id       INTEGER PRIMARY KEY AUTOINCREMENT,
            name     TEXT NOT NULL UNIQUE,
            color    TEXT NOT NULL DEFAULT '#6b7280',
            position INTEGER NOT NULL DEFAULT 0
        );

        CREATE TABLE workflows (
            id             INTEGER PRIMARY KEY AUTOINCREMENT,
            from_status_id INTEGER NOT NULL REFERENCES statuses(id),
            to_status_id   INTEGER NOT NULL REFERENCES statuses(id),
            UNIQUE(from_status_id, to_status_id)
        );

        CREATE TABLE ticket_types (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            name        TEXT NOT NULL UNIQUE,
            description TEXT NOT NULL DEFAULT ''
        );

        CREATE TABLE custom_fields (
            id             INTEGER PRIMARY KEY AUTOINCREMENT,
            ticket_type_id INTEGER NOT NULL REFERENCES ticket_types(id) ON DELETE CASCADE,
            name           TEXT NOT NULL,
            field_type     TEXT NOT NULL CHECK (field_type IN ('text','number','date','textarea','user','ticket')),
            is_required    INTEGER NOT NULL DEFAULT 0,
            position       INTEGER NOT NULL DEFAULT 0,
            num_min        REAL,
            num_max        REAL,
            num_step       REAL,
            placeholder    TEXT NOT NULL DEFAULT '',
            default_value  TEXT NOT NULL DEFAULT '',
            UNIQUE(ticket_type_id, name)
        );

        CREATE TABLE projects (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            name        TEXT NOT NULL UNIQUE,
            description TEXT NOT NULL DEFAULT '',
            created_by  INTEGER REFERENCES users(id),
            created_at  TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE TABLE project_members (
            project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
            user_id    INTEGER NOT NULL REFERENCES users(id),
            role       TEXT NOT NULL DEFAULT 'member' CHECK (role IN ('manager', 'member', 'reporter')),
            PRIMARY KEY (project_id, user_id)
        );

        CREATE TABLE project_statuses (
            project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
            status_id  INTEGER NOT NULL REFERENCES statuses(id),
            PRIMARY KEY (project_id, status_id)
        );

        CREATE TABLE project_ticket_types (
            project_id     INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
            ticket_type_id INTEGER NOT NULL REFERENCES ticket_types(id),
            PRIMARY KEY (project_id, ticket_type_id)
        );

        CREATE TABLE tickets (
            id             INTEGER PRIMARY KEY AUTOINCREMENT,
            project_id     INTEGER NOT NULL REFERENCES projects(id),
            ticket_type_id INTEGER NOT NULL REFERENCES ticket_types(id),
            status_id      INTEGER NOT NULL REFERENCES statuses(id),
            creator_id     INTEGER NOT NULL REFERENCES users(id),
            assignee_id    INTEGER NOT NULL REFERENCES users(id),
            title          TEXT NOT NULL,
            text           TEXT NOT NULL DEFAULT '',
            due_date       TEXT NOT NULL,
            created_at     TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at     TEXT NOT NULL DEFAULT (datetime('now')),
            is_deleted     INTEGER NOT NULL DEFAULT 0
        );

        CREATE INDEX idx_tickets_project ON tickets(project_id, is_deleted);

        CREATE TABLE ticket_field_values (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            ticket_id       INTEGER NOT NULL REFERENCES tickets(id) ON DELETE CASCADE,
            custom_field_id INTEGER NOT NULL REFERENCES custom_fields(id),
            value           TEXT NOT NULL DEFAULT '',
            UNIQUE(ticket_id, custom_field_id)
        );
        ",
    )
    .expect("Failed to create schema");

    conn.pragma_update(None, "user_version", SCHEMA_VERSION)
        .expect("Failed to set initial schema version");
    println!("Created fresh database at migration version {SCHEMA_VERSION}");
}
