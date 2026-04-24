use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::params;

use crate::config::Config;

pub type DbPool = Pool<SqliteConnectionManager>;

/// Total number of migrations. Keep in sync with the migrations vec in run_migrations().
const MIGRATION_COUNT: i64 = 5;

pub fn open_connection(database_url: &str) -> rusqlite::Connection {
    let conn = rusqlite::Connection::open(database_url).expect("Failed to open database");
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
        .expect("Failed to set pragmas");
    conn
}

pub fn init_pool(database_url: &str, config: &Config) -> DbPool {
    let manager = SqliteConnectionManager::file(database_url);
    let pool = Pool::builder()
        .max_size(config.db_pool_size)
        .build(manager)
        .expect("Failed to create pool");

    let conn = pool.get().expect("Failed to get connection");
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
        .expect("Failed to set pragmas");

    create_schema(&conn);
    seed_admin(&conn, config);

    pool
}

/// Initial schema — runs only when tables don't exist yet.
/// This reflects the LATEST schema (post all migrations).
/// When tables are freshly created, user_version is set to MIGRATION_COUNT
/// so that migrations are skipped (they're only for upgrading old databases).
fn create_schema(conn: &rusqlite::Connection) {
    // Check if this is a fresh database (no tables yet)
    let table_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'users'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    if table_count > 0 {
        return; // Tables already exist, nothing to do
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

    // Mark fresh database as fully migrated
    conn.pragma_update(None, "user_version", MIGRATION_COUNT)
        .expect("Failed to set initial schema version");
    println!("Created fresh database at migration version {MIGRATION_COUNT}");
}

/// Versioned migrations. Each entry runs once, in order.
/// Add new migrations to the end of the list — never modify existing ones.
/// Update MIGRATION_COUNT when adding new migrations.
pub fn run_migrations(conn: &rusqlite::Connection) {
    let migrations: Vec<&str> = vec![
        // Migration 1: Add 'user' and 'ticket' to custom_fields.field_type CHECK constraint.
        "
        PRAGMA foreign_keys=OFF;

        CREATE TABLE custom_fields_new (
            id             INTEGER PRIMARY KEY AUTOINCREMENT,
            ticket_type_id INTEGER NOT NULL REFERENCES ticket_types(id) ON DELETE CASCADE,
            name           TEXT NOT NULL,
            field_type     TEXT NOT NULL CHECK (field_type IN ('text','number','date','textarea','user','ticket')),
            is_required    INTEGER NOT NULL DEFAULT 0,
            position       INTEGER NOT NULL DEFAULT 0,
            num_min        REAL,
            num_max        REAL,
            num_step       REAL,
            UNIQUE(ticket_type_id, name)
        );

        INSERT INTO custom_fields_new SELECT * FROM custom_fields;
        DROP TABLE custom_fields;
        ALTER TABLE custom_fields_new RENAME TO custom_fields;

        PRAGMA foreign_keys=ON;
        ",

        // Migration 2: Move role from users to project_members.
        // - Replace users.role with users.is_admin flag
        // - Add role column to project_members
        // Order matters: project_members must be migrated FIRST while users still has the role column.
        "
        PRAGMA foreign_keys=OFF;

        -- Step 1: Recreate project_members with role column WHILE old users.role still exists
        CREATE TABLE project_members_new (
            project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
            user_id    INTEGER NOT NULL REFERENCES users(id),
            role       TEXT NOT NULL DEFAULT 'member' CHECK (role IN ('member', 'reporter')),
            PRIMARY KEY (project_id, user_id)
        );

        INSERT INTO project_members_new (project_id, user_id, role)
            SELECT pm.project_id, pm.user_id,
                   CASE WHEN u.role = 'reporter' THEN 'reporter' ELSE 'member' END
            FROM project_members pm
            JOIN users u ON u.id = pm.user_id;

        DROP TABLE project_members;
        ALTER TABLE project_members_new RENAME TO project_members;

        -- Step 2: Now recreate users without role, adding is_admin
        CREATE TABLE users_new (
            id            INTEGER PRIMARY KEY AUTOINCREMENT,
            username      TEXT NOT NULL UNIQUE,
            email         TEXT NOT NULL UNIQUE,
            password_hash TEXT NOT NULL,
            is_admin      INTEGER NOT NULL DEFAULT 0,
            created_at    TEXT NOT NULL DEFAULT (datetime('now')),
            is_active     INTEGER NOT NULL DEFAULT 1
        );

        INSERT INTO users_new (id, username, email, password_hash, is_admin, created_at, is_active)
            SELECT id, username, email, password_hash,
                   CASE WHEN role = 'admin' THEN 1 ELSE 0 END,
                   created_at, is_active
            FROM users;

        DROP TABLE users;
        ALTER TABLE users_new RENAME TO users;

        PRAGMA foreign_keys=ON;
        ",

        // Migration 3: Add 'manager' project role and created_by on projects.
        "
        PRAGMA foreign_keys=OFF;

        -- Add created_by to projects
        ALTER TABLE projects ADD COLUMN created_by INTEGER REFERENCES users(id);

        -- Recreate project_members with manager role option
        CREATE TABLE project_members_new (
            project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
            user_id    INTEGER NOT NULL REFERENCES users(id),
            role       TEXT NOT NULL DEFAULT 'member' CHECK (role IN ('manager', 'member', 'reporter')),
            PRIMARY KEY (project_id, user_id)
        );

        INSERT INTO project_members_new SELECT * FROM project_members;
        DROP TABLE project_members;
        ALTER TABLE project_members_new RENAME TO project_members;

        PRAGMA foreign_keys=ON;
        ",

        // Migration 4: Add is_manager flag to users.
        "
        ALTER TABLE users ADD COLUMN is_manager INTEGER NOT NULL DEFAULT 0;
        ",

        // Migration 5: Add assignee_id and due_date to tickets.
        // Existing tickets get creator as assignee and today as due date.
        "
        PRAGMA foreign_keys=OFF;

        CREATE TABLE tickets_new (
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

        INSERT INTO tickets_new (id, project_id, ticket_type_id, status_id, creator_id, assignee_id, title, text, due_date, created_at, updated_at, is_deleted)
            SELECT id, project_id, ticket_type_id, status_id, creator_id, creator_id, title, text, date('now'), created_at, updated_at, is_deleted
            FROM tickets;

        DROP TABLE tickets;
        ALTER TABLE tickets_new RENAME TO tickets;

        CREATE INDEX idx_tickets_project ON tickets(project_id, is_deleted);

        PRAGMA foreign_keys=ON;
        ",
    ];

    assert!(
        migrations.len() as i64 == MIGRATION_COUNT,
        "MIGRATION_COUNT ({MIGRATION_COUNT}) does not match migrations vec length ({})",
        migrations.len()
    );

    let current_version: i64 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap_or(0);

    if current_version >= MIGRATION_COUNT {
        println!("Database already at version {current_version}, no migrations needed.");
        return;
    }

    for (i, sql) in migrations.iter().enumerate() {
        let version = i as i64 + 1;
        if version <= current_version {
            continue;
        }
        println!("Running migration {version}/{MIGRATION_COUNT}...");
        conn.execute_batch(sql)
            .unwrap_or_else(|e| panic!("Migration {version} failed: {e}"));
    }

    conn.pragma_update(None, "user_version", MIGRATION_COUNT)
        .expect("Failed to update schema version");
    println!("Database migrated to version {MIGRATION_COUNT}");
}

fn seed_admin(conn: &rusqlite::Connection, config: &Config) {
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM users WHERE is_admin = 1",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    if count == 0 {
        let hash = crate::auth::hash_password(&config.admin_default_password)
            .expect("Failed to hash password");
        conn.execute(
            "INSERT INTO users (username, email, password_hash, is_admin) VALUES (?1, ?2, ?3, 1)",
            params![config.admin_default_username, config.admin_default_email, hash],
        )
        .expect("Failed to seed admin user");
        println!(
            "Seeded admin user ({} / {})",
            config.admin_default_username, config.admin_default_password
        );
    }
}
