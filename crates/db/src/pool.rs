use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;

pub type DbPool = Pool<SqliteConnectionManager>;

pub fn open_connection(database_url: &str) -> rusqlite::Connection {
    let conn = rusqlite::Connection::open(database_url).expect("Failed to open database");
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
        .expect("Failed to set pragmas");
    conn
}

pub fn init_pool(database_url: &str, pool_size: u32) -> DbPool {
    let manager = SqliteConnectionManager::file(database_url);
    let pool = Pool::builder()
        .max_size(pool_size)
        .build(manager)
        .expect("Failed to create pool");

    let conn = pool.get().expect("Failed to get connection");
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
        .expect("Failed to set pragmas");

    crate::schema::create_schema(&conn);

    pool
}
