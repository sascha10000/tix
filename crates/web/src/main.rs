mod errors;
mod handlers;
mod middleware;

use actix_web::{web, App, HttpResponse, HttpServer};
use clap::Parser;
use ticketsystem_core::config::Config;
use ticketsystem_db::DbPool;

#[derive(Parser)]
#[command(name = "ticketsystem", about = "Simple ticket system")]
struct Args {
    /// Run database migrations and exit
    #[arg(long)]
    migrate: bool,
}

const EMBEDDED_CSS: &str = include_str!("../../../landingpage/static/style.css");
const EMBEDDED_FAVICON: &str = include_str!("../../../landingpage/static/favicon.svg");

async fn serve_css() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/css")
        .body(EMBEDDED_CSS)
}

async fn serve_favicon() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("image/svg+xml")
        .body(EMBEDDED_FAVICON)
}

/// Seed the default admin user if none exists (startup-only).
fn seed_admin(pool: &DbPool, config: &Config) {
    let conn = pool.get().expect("Failed to get connection for seeding");
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM users WHERE is_admin = 1",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    if count == 0 {
        let hash = ticketsystem_auth::hash_password(&config.admin_default_password)
            .expect("Failed to hash password");
        conn.execute(
            "INSERT INTO users (username, email, password_hash, is_admin) VALUES (?1, ?2, ?3, 1)",
            ticketsystem_db::rusqlite::params![
                config.admin_default_username,
                config.admin_default_email,
                hash
            ],
        )
        .expect("Failed to seed admin user");
        println!(
            "Seeded admin user ({} / {})",
            config.admin_default_username, config.admin_default_password
        );
    }
}

/// Register admin routes only when the `admin` feature is enabled.
#[cfg(feature = "admin")]
fn register_admin_routes(cfg: &mut web::ServiceConfig) {
    cfg.route("/admin/users", web::get().to(handlers::admin::users::list))
        .route("/admin/users/new", web::get().to(handlers::admin::users::new_page))
        .route("/admin/users", web::post().to(handlers::admin::users::create))
        .route("/admin/users/{id}/edit", web::get().to(handlers::admin::users::edit_page))
        .route("/admin/users/{id}/edit", web::post().to(handlers::admin::users::edit_submit))
        .route("/admin/users/{id}/delete", web::post().to(handlers::admin::users::delete))
        .route("/admin/statuses", web::get().to(handlers::admin::statuses::list))
        .route("/admin/statuses/new", web::get().to(handlers::admin::statuses::new_page))
        .route("/admin/statuses", web::post().to(handlers::admin::statuses::create))
        .route("/admin/statuses/{id}/edit", web::get().to(handlers::admin::statuses::edit_page))
        .route("/admin/statuses/{id}/edit", web::post().to(handlers::admin::statuses::edit_submit))
        .route("/admin/statuses/{id}/delete", web::post().to(handlers::admin::statuses::delete))
        .route("/admin/workflows", web::post().to(handlers::admin::statuses::toggle_workflow))
        .route("/admin/ticket-types", web::get().to(handlers::admin::ticket_types::list))
        .route("/admin/ticket-types/new", web::get().to(handlers::admin::ticket_types::new_page))
        .route("/admin/ticket-types", web::post().to(handlers::admin::ticket_types::create))
        .route("/admin/ticket-types/{id}/edit", web::get().to(handlers::admin::ticket_types::edit_page))
        .route("/admin/ticket-types/{id}/edit", web::post().to(handlers::admin::ticket_types::edit_submit))
        .route("/admin/ticket-types/{id}/fields", web::post().to(handlers::admin::ticket_types::add_field))
        .route("/admin/ticket-types/{id}/fields/{fid}/delete", web::post().to(handlers::admin::ticket_types::delete_field));
}

#[cfg(not(feature = "admin"))]
fn register_admin_routes(_cfg: &mut web::ServiceConfig) {}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenvy::dotenv().ok();
    let args = Args::parse();
    let config = Config::from_env();

    if args.migrate {
        #[cfg(feature = "migrations")]
        {
            let conn = ticketsystem_db::pool::open_connection(&config.db_name);
            ticketsystem_db::migrations::run_migrations(&conn);
            println!("Migrations complete.");
        }
        #[cfg(not(feature = "migrations"))]
        {
            eprintln!("Migrations feature is not enabled. Rebuild with --features migrations.");
        }
        return Ok(());
    }

    let pool = ticketsystem_db::pool::init_pool(&config.db_name, config.db_pool_size);
    seed_admin(&pool, &config);

    println!("Starting server at http://{}", config.bind_address);

    let bind_address = config.bind_address.clone();
    let session_hours = config.session_duration_hours;

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(pool.clone()))
            .app_data(web::Data::new(session_hours))
            // Static files (embedded in binary)
            .route("/static/style.css", web::get().to(serve_css))
            .route("/static/favicon.svg", web::get().to(serve_favicon))
            // Auth routes
            .route("/login", web::get().to(handlers::auth_handlers::login_page))
            .route("/login", web::post().to(handlers::auth_handlers::login_submit))
            .route("/register", web::get().to(handlers::auth_handlers::register_page))
            .route("/register", web::post().to(handlers::auth_handlers::register_submit))
            .route("/logout", web::post().to(handlers::auth_handlers::logout))
            // Profile
            .route("/profile", web::get().to(handlers::profile::page))
            .route("/profile", web::post().to(handlers::profile::update))
            .route("/profile/password", web::post().to(handlers::profile::change_password))
            // Dashboard
            .route("/", web::get().to(handlers::dashboard::index))
            // Admin (feature-gated)
            .configure(register_admin_routes)
            // Projects
            .route("/projects", web::get().to(handlers::projects::list))
            .route("/projects/new", web::get().to(handlers::projects::new_page))
            .route("/projects", web::post().to(handlers::projects::create))
            .route("/projects/{id}", web::get().to(handlers::projects::detail))
            .route("/projects/{id}/edit", web::get().to(handlers::projects::edit_page))
            .route("/projects/{id}/edit", web::post().to(handlers::projects::edit_submit))
            .route("/projects/{id}/delete", web::post().to(handlers::projects::delete_project))
            .route("/projects/{id}/members", web::post().to(handlers::projects::add_member))
            .route("/projects/{id}/members/{uid}/remove", web::post().to(handlers::projects::remove_member))
            .route("/projects/{id}/statuses", web::post().to(handlers::projects::toggle_status))
            .route("/projects/{id}/ticket-types", web::post().to(handlers::projects::toggle_ticket_type))
            // Tickets
            .route("/projects/{pid}/tickets", web::get().to(handlers::tickets::list))
            .route("/projects/{pid}/tickets/new", web::get().to(handlers::tickets::new_page))
            .route("/projects/{pid}/tickets", web::post().to(handlers::tickets::create))
            .route("/projects/{pid}/tickets/{id}", web::get().to(handlers::tickets::detail))
            .route("/projects/{pid}/tickets/{id}/edit", web::get().to(handlers::tickets::edit_page))
            .route("/projects/{pid}/tickets/{id}/edit", web::post().to(handlers::tickets::edit_submit))
            .route("/projects/{pid}/tickets/{id}/transition", web::post().to(handlers::tickets::transition))
            .route("/projects/{pid}/tickets/{id}/delete", web::post().to(handlers::tickets::delete))
    })
    .bind(&bind_address)?
    .run()
    .await
}
