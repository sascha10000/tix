/// Total number of migrations. Keep in sync with the migrations vec in run_migrations().
pub const MIGRATION_COUNT: i64 = 5;

/// Versioned migrations. Each entry runs once, in order.
/// Add new migrations to the end of the list -- never modify existing ones.
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
        "
        PRAGMA foreign_keys=OFF;

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

        ALTER TABLE projects ADD COLUMN created_by INTEGER REFERENCES users(id);

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
