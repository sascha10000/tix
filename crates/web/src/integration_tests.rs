//! End-to-end test of the OAuth 2.1 flow and the MCP endpoint against a real
//! (temporary) SQLite database.

use actix_web::cookie::Cookie;
use actix_web::http::StatusCode;
use actix_web::{test, web, App};
use serde_json::{json, Value};

use ticketsystem_db::repo::{oauth as oauth_repo, project, status, ticket_type, user};
use ticketsystem_db::DbPool;

use crate::{cors, handlers, mcp, oauth};

// Independent PKCE vector (computed with openssl, see ticketsystem-auth tests).
const VERIFIER: &str = "test-verifier-0123456789abcdefghijklmnopqrstuvwxyz";
const CHALLENGE: &str = "_i87_9qTgIdW6lIGSOGQx4iLENnqnygSha1TvdNTuWo";
const REDIRECT: &str = "http://127.0.0.1:33418/callback";

struct Fixture {
    pool: DbPool,
    db_path: std::path::PathBuf,
    alice: i64,
    bob: i64,
    project_id: i64,
    open_id: i64,
    done_id: i64,
    bug_type_id: i64,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{suffix}", self.db_path.display()));
        }
    }
}

fn fixture() -> Fixture {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let db_path = std::env::temp_dir().join(format!("ticketsystem-test-{}-{nanos}.db", std::process::id()));
    let pool = ticketsystem_db::pool::init_pool(db_path.to_str().unwrap(), 4);
    let conn = pool.get().unwrap();

    let alice = user::create(&conn, "alice", "alice@example.test", "x", false, false).unwrap();
    let bob = user::create(&conn, "bob", "bob@example.test", "x", false, false).unwrap();
    let project_id = project::create(&conn, "Website", "Public website", alice).unwrap();

    let open_id = status::create(&conn, "Open", "#888888", 1).unwrap();
    let done_id = status::create(&conn, "Done", "#00aa00", 2).unwrap();
    status::toggle_workflow(&conn, open_id, done_id).unwrap();
    project::toggle_status(&conn, project_id, open_id).unwrap();
    project::toggle_status(&conn, project_id, done_id).unwrap();

    let bug_type_id = ticket_type::create(&conn, "Bug", "Something is broken").unwrap();
    ticket_type::add_field(&conn, bug_type_id, "Severity", "number", true, 1, Some(1.0), Some(5.0), Some(1.0), "", "")
        .unwrap();
    project::toggle_ticket_type(&conn, project_id, bug_type_id).unwrap();

    drop(conn);
    Fixture { pool, db_path, alice, bob, project_id, open_id, done_id, bug_type_id }
}

fn settings() -> oauth::OAuthSettings {
    oauth::OAuthSettings {
        base_url: "http://localhost:8080".into(),
        access_token_minutes: 60,
        refresh_token_minutes: 60,
    }
}

macro_rules! app {
    ($fx:expr) => {
        test::init_service(
            App::new()
                .wrap(actix_web::middleware::from_fn(cors::cors))
                .app_data(web::Data::new($fx.pool.clone()))
                .app_data(web::Data::new(settings()))
                .configure(oauth::routes)
                .configure(mcp::routes)
                .route(
                    "/profile/connections/{client_id}/revoke",
                    web::post().to(handlers::profile::revoke_connection),
                ),
        )
        .await
    };
}

fn query_param(location: &str, key: &str) -> Option<String> {
    url::Url::parse(location)
        .ok()?
        .query_pairs()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.into_owned())
}

/// Calls `/mcp` and returns (status, body). Body is Null for empty responses.
macro_rules! mcp_call {
    ($app:expr, $token:expr, $body:expr) => {{
        let mut req = test::TestRequest::post().uri("/mcp").set_json($body);
        if let Some(token) = $token {
            req = req.insert_header(("Authorization", format!("Bearer {token}")));
        }
        let resp = test::call_service(&$app, req.to_request()).await;
        let status = resp.status();
        let bytes = test::read_body(resp).await;
        let body: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, body)
    }};
}

macro_rules! tool {
    ($app:expr, $token:expr, $name:expr, $args:expr) => {{
        let (status, body) = mcp_call!(
            $app,
            Some($token),
            json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": { "name": $name, "arguments": $args } })
        );
        assert_eq!(status, StatusCode::OK, "tool {} failed: {body}", $name);
        body["result"].clone()
    }};
}

#[actix_web::test]
async fn oauth_flow_and_mcp_tools() {
    let fx = fixture();
    let app = app!(fx);

    // --- Discovery -------------------------------------------------------
    let resp = test::call_service(&app, test::TestRequest::get().uri("/.well-known/oauth-protected-resource").to_request()).await;
    assert_eq!(resp.headers().get("access-control-allow-origin").unwrap(), "*");
    let meta: Value = test::read_body_json(resp).await;
    assert_eq!(meta["resource"], "http://localhost:8080/mcp");
    assert_eq!(meta["authorization_servers"][0], "http://localhost:8080");

    let meta: Value = test::read_body_json(
        test::call_service(&app, test::TestRequest::get().uri("/.well-known/oauth-authorization-server").to_request()).await,
    )
    .await;
    assert_eq!(meta["code_challenge_methods_supported"], json!(["S256"]));

    // --- Unauthenticated MCP call advertises the metadata ----------------
    let resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/mcp")
            .set_json(json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {} }))
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let challenge = resp.headers().get("www-authenticate").unwrap().to_str().unwrap();
    assert!(challenge.contains(r#"resource_metadata="http://localhost:8080/.well-known/oauth-protected-resource""#));

    // --- Dynamic client registration ------------------------------------
    let resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/oauth/register")
            .set_json(json!({ "client_name": "Test <b>Client</b>", "redirect_uris": [REDIRECT] }))
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let reg: Value = test::read_body_json(resp).await;
    let client_id = reg["client_id"].as_str().unwrap().to_string();

    let resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/oauth/register")
            .set_json(json!({ "redirect_uris": ["http://evil.example.com/cb"] }))
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    // --- Authorization ----------------------------------------------------
    let authorize_url = format!(
        "/oauth/authorize?response_type=code&client_id={client_id}&redirect_uri={}&code_challenge={CHALLENGE}&code_challenge_method=S256&state=xyz&scope=tickets:read%20tickets:write&resource=http://localhost:8080/mcp",
        url::form_urlencoded::byte_serialize(REDIRECT.as_bytes()).collect::<String>()
    );

    // Not logged in: bounced to the login page, coming back afterwards.
    let resp = test::call_service(&app, test::TestRequest::get().uri(&authorize_url).to_request()).await;
    assert_eq!(resp.status(), StatusCode::FOUND);
    let location = resp.headers().get("location").unwrap().to_str().unwrap();
    assert!(location.starts_with("/login?next=%2Foauth%2Fauthorize"), "{location}");

    // Unknown redirect_uri is an error page, never a redirect.
    let bad = authorize_url.replace("33418", "1");
    let session = {
        let conn = fx.pool.get().unwrap();
        user::create_session(&conn, fx.alice, 1).unwrap()
    };
    let resp = test::call_service(
        &app,
        test::TestRequest::get().uri(&bad).cookie(Cookie::new("session_id", session.clone())).to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    // Logged in: consent page, with the client name escaped.
    let resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(&authorize_url)
            .cookie(Cookie::new("session_id", session.clone()))
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(resp.headers().get("x-frame-options").unwrap(), "DENY");
    let html = String::from_utf8(test::read_body(resp).await.to_vec()).unwrap();
    assert!(html.contains("Test &lt;b&gt;Client&lt;/b&gt;"));

    let approve = |decision: &'static str| {
        test::TestRequest::post()
            .uri("/oauth/authorize")
            .cookie(Cookie::new("session_id", session.clone()))
            .set_form([
                ("client_id", client_id.as_str()),
                ("redirect_uri", REDIRECT),
                ("code_challenge", CHALLENGE),
                ("scope", "tickets:read tickets:write"),
                ("state", "xyz"),
                ("decision", decision),
            ])
            .to_request()
    };

    let resp = test::call_service(&app, approve("deny")).await;
    let location = resp.headers().get("location").unwrap().to_str().unwrap().to_string();
    assert_eq!(query_param(&location, "error").as_deref(), Some("access_denied"));

    let resp = test::call_service(&app, approve("allow")).await;
    assert_eq!(resp.status(), StatusCode::FOUND);
    let location = resp.headers().get("location").unwrap().to_str().unwrap().to_string();
    assert!(location.starts_with(REDIRECT));
    assert_eq!(query_param(&location, "state").as_deref(), Some("xyz"));
    let code = query_param(&location, "code").unwrap();

    // --- Token exchange ---------------------------------------------------
    let exchange = |code: &str, verifier: &str| {
        test::TestRequest::post()
            .uri("/oauth/token")
            .set_form([
                ("grant_type", "authorization_code"),
                ("code", code),
                ("client_id", client_id.as_str()),
                ("redirect_uri", REDIRECT),
                ("code_verifier", verifier),
            ])
            .to_request()
    };

    // A wrong verifier fails and burns the code.
    let wrong_verifier = "x".repeat(43);
    let resp = test::call_service(&app, exchange(&code, &wrong_verifier)).await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let resp = test::call_service(&app, exchange(&code, VERIFIER)).await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "code must be single-use");

    let resp = test::call_service(&app, approve("allow")).await;
    let location = resp.headers().get("location").unwrap().to_str().unwrap().to_string();
    let code = query_param(&location, "code").unwrap();
    let resp = test::call_service(&app, exchange(&code, VERIFIER)).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(resp.headers().get("cache-control").unwrap(), "no-store");
    let tokens: Value = test::read_body_json(resp).await;
    let access = tokens["access_token"].as_str().unwrap().to_string();
    let refresh_token = tokens["refresh_token"].as_str().unwrap().to_string();
    assert_eq!(tokens["scope"], "tickets:read tickets:write");

    // --- MCP lifecycle ----------------------------------------------------
    let (status, body) = mcp_call!(
        app,
        Some(&access),
        json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": { "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "test", "version": "1" } } })
    );
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["result"]["protocolVersion"], "2025-06-18");
    assert!(body["result"]["capabilities"]["tools"].is_object());

    let (status, _) = mcp_call!(app, Some(&access), json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }));
    assert_eq!(status, StatusCode::ACCEPTED);

    let (_, body) = mcp_call!(app, Some(&access), json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" }));
    let names: Vec<&str> = body["result"]["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert_eq!(
        names,
        ["list_projects", "get_project", "list_tickets", "get_ticket", "list_my_tickets", "create_ticket", "update_ticket", "transition_ticket"]
    );

    let (_, body) = mcp_call!(app, Some(&access), json!({ "jsonrpc": "2.0", "id": 3, "method": "nope" }));
    assert_eq!(body["error"]["code"], -32601);

    // --- Tools --------------------------------------------------------------
    let r = tool!(app, &access, "list_projects", json!({}));
    assert_eq!(r["structuredContent"]["projects"][0]["name"], "Website");
    assert_eq!(r["structuredContent"]["projects"][0]["your_role"], "manager");

    let r = tool!(app, &access, "get_project", json!({ "project_id": fx.project_id }));
    let p = &r["structuredContent"];
    assert_eq!(p["statuses"].as_array().unwrap().len(), 2);
    assert_eq!(p["ticket_types"][0]["fields"][0]["name"], "Severity");
    assert_eq!(p["transitions"][0]["to_status_id"], fx.done_id);

    // Missing required custom field.
    let r = tool!(app, &access, "create_ticket", json!({
        "project_id": fx.project_id, "title": "Login broken", "ticket_type_id": fx.bug_type_id, "due_date": "2026-11-01"
    }));
    assert_eq!(r["isError"], true);
    assert!(r["content"][0]["text"].as_str().unwrap().contains("Severity"));

    // Out-of-range number and bad date.
    let r = tool!(app, &access, "create_ticket", json!({
        "project_id": fx.project_id, "title": "Login broken", "ticket_type_id": fx.bug_type_id, "due_date": "2026-11-01", "fields": { "Severity": 9 }
    }));
    assert_eq!(r["isError"], true);
    let r = tool!(app, &access, "create_ticket", json!({
        "project_id": fx.project_id, "title": "Login broken", "ticket_type_id": fx.bug_type_id, "due_date": "tomorrow", "fields": { "Severity": 3 }
    }));
    assert_eq!(r["isError"], true);

    // Non-member assignee.
    let r = tool!(app, &access, "create_ticket", json!({
        "project_id": fx.project_id, "title": "Login broken", "ticket_type_id": fx.bug_type_id, "due_date": "2026-11-01", "assignee_id": fx.bob, "fields": { "Severity": 3 }
    }));
    assert_eq!(r["isError"], true);

    let r = tool!(app, &access, "create_ticket", json!({
        "project_id": fx.project_id, "title": "Login broken", "text": "500 on submit", "ticket_type_id": fx.bug_type_id,
        "due_date": "2026-11-01", "fields": { "severity": 3 }
    }));
    assert_eq!(r["isError"], false, "{r}");
    let created = &r["structuredContent"];
    let ticket_id = created["ticket"]["id"].as_i64().unwrap();
    assert_eq!(created["ticket"]["status"]["id"], fx.open_id, "defaults to first active status");
    assert_eq!(created["ticket"]["assignee"]["id"], fx.alice, "defaults to caller");
    assert_eq!(created["fields"][0]["value"], "3");
    assert_eq!(created["allowed_transitions"][0]["id"], fx.done_id);

    let r = tool!(app, &access, "create_ticket", json!({
        "project_id": fx.project_id, "title": "Update footer", "ticket_type_id": fx.bug_type_id, "due_date": "2026-12-01", "fields": { "Severity": 1 }
    }));
    assert_eq!(r["isError"], false, "{r}");

    let r = tool!(app, &access, "list_tickets", json!({ "project_id": fx.project_id, "query": "LOGIN" }));
    assert_eq!(r["structuredContent"]["total"], 1);
    assert_eq!(r["structuredContent"]["tickets"][0]["id"], ticket_id);
    let r = tool!(app, &access, "list_tickets", json!({ "project_id": fx.project_id, "limit": 1 }));
    assert_eq!(r["structuredContent"]["total"], 2);
    assert_eq!(r["structuredContent"]["tickets"].as_array().unwrap().len(), 1);

    let r = tool!(app, &access, "update_ticket", json!({ "ticket_id": ticket_id, "due_date": "2026-11-15", "fields": { "Severity": 5 } }));
    assert_eq!(r["isError"], false, "{r}");
    assert_eq!(r["structuredContent"]["ticket"]["due_date"], "2026-11-15");
    assert_eq!(r["structuredContent"]["ticket"]["title"], "Login broken", "untouched fields are kept");
    assert_eq!(r["structuredContent"]["fields"][0]["value"], "5");

    let r = tool!(app, &access, "transition_ticket", json!({ "ticket_id": ticket_id, "status_id": fx.done_id }));
    assert_eq!(r["isError"], false, "{r}");
    assert_eq!(r["structuredContent"]["ticket"]["status"]["name"], "Done");
    // No Done -> Open transition in the workflow.
    let r = tool!(app, &access, "transition_ticket", json!({ "ticket_id": ticket_id, "status_id": fx.open_id }));
    assert_eq!(r["isError"], true);

    let r = tool!(app, &access, "list_my_tickets", json!({}));
    assert_eq!(r["structuredContent"]["tickets"].as_array().unwrap().len(), 2);

    // --- Another user, read-only token ------------------------------------
    let bob_token = "bob-read-only-token";
    {
        let conn = fx.pool.get().unwrap();
        oauth_repo::create_token(&conn, &ticketsystem_auth::hash_token(bob_token), "access", &client_id, fx.bob, "tickets:read", 60)
            .unwrap();
    }
    let r = tool!(app, bob_token, "list_projects", json!({}));
    assert_eq!(r["structuredContent"]["projects"], json!([]));
    let r = tool!(app, bob_token, "get_ticket", json!({ "ticket_id": ticket_id }));
    assert_eq!(r["isError"], true);
    assert!(r["content"][0]["text"].as_str().unwrap().contains("not found"), "no existence leak");
    let r = tool!(app, bob_token, "list_tickets", json!({ "project_id": fx.project_id }));
    assert_eq!(r["isError"], true);
    let r = tool!(app, bob_token, "create_ticket", json!({
        "project_id": fx.project_id, "title": "x", "ticket_type_id": fx.bug_type_id, "due_date": "2026-11-01"
    }));
    assert!(r["content"][0]["text"].as_str().unwrap().contains("tickets:write"));

    // --- Refresh-token rotation --------------------------------------------
    let refresh = |rt: &str| {
        test::TestRequest::post()
            .uri("/oauth/token")
            .set_form([("grant_type", "refresh_token"), ("refresh_token", rt), ("client_id", client_id.as_str())])
            .to_request()
    };
    let resp = test::call_service(&app, refresh(&refresh_token)).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let rotated: Value = test::read_body_json(resp).await;
    let new_access = rotated["access_token"].as_str().unwrap().to_string();
    let resp = test::call_service(&app, refresh(&refresh_token)).await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "old refresh token is invalid after rotation");
    let r = tool!(app, &new_access, "list_projects", json!({}));
    assert_eq!(r["isError"], false, "rotated access token works");

    // --- Revocation from the profile page ----------------------------------
    let resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri(&format!("/profile/connections/{client_id}/revoke"))
            .cookie(Cookie::new("session_id", session.clone()))
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let (status, _) = mcp_call!(app, Some(&new_access), json!({ "jsonrpc": "2.0", "id": 9, "method": "ping" }));
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    // Bob's grant to the same client is unaffected.
    let (status, _) = mcp_call!(app, Some(bob_token), json!({ "jsonrpc": "2.0", "id": 9, "method": "ping" }));
    assert_eq!(status, StatusCode::OK);

    // --- CORS preflight -------------------------------------------------------
    let resp = test::call_service(&app, test::TestRequest::default().method(actix_web::http::Method::OPTIONS).uri("/mcp").to_request()).await;
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    assert!(resp.headers().get("access-control-allow-headers").unwrap().to_str().unwrap().contains("Authorization"));
}
