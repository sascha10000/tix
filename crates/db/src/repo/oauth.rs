use rusqlite::{Connection, params};
use ticketsystem_core::models::oauth::{AuthorizationCode, OAuthClient, OAuthConnection, TokenGrant};

// Redirect URIs are stored newline-separated (a URI can never contain a raw newline).

pub fn create_client(conn: &Connection, id: &str, name: &str, redirect_uris: &[String]) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO oauth_clients (id, name, redirect_uris) VALUES (?1, ?2, ?3)",
        params![id, name, redirect_uris.join("\n")],
    )?;
    Ok(())
}

pub fn find_client(conn: &Connection, id: &str) -> Option<OAuthClient> {
    conn.query_row(
        "SELECT id, name, redirect_uris, created_at FROM oauth_clients WHERE id = ?1",
        [id],
        |row| {
            let uris: String = row.get(2)?;
            Ok(OAuthClient {
                id: row.get(0)?,
                name: row.get(1)?,
                redirect_uris: uris.lines().map(str::to_string).collect(),
                created_at: row.get(3)?,
            })
        },
    )
    .ok()
}

#[allow(clippy::too_many_arguments)]
pub fn create_code(
    conn: &Connection,
    code_hash: &str,
    client_id: &str,
    user_id: i64,
    redirect_uri: &str,
    code_challenge: &str,
    scope: &str,
    ttl_minutes: i64,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO oauth_codes (code_hash, client_id, user_id, redirect_uri, code_challenge, scope, expires_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, datetime('now', ?7))",
        params![
            code_hash,
            client_id,
            user_id,
            redirect_uri,
            code_challenge,
            scope,
            format!("+{ttl_minutes} minutes")
        ],
    )?;
    Ok(())
}

/// Deletes the code and returns it if it had not yet expired.
/// Because the row is removed in the same statement, a code can be redeemed at most once.
pub fn take_code(conn: &Connection, code_hash: &str) -> Option<AuthorizationCode> {
    let (code, valid) = conn
        .query_row(
            "DELETE FROM oauth_codes WHERE code_hash = ?1
             RETURNING client_id, user_id, redirect_uri, code_challenge, scope, expires_at > datetime('now')",
            [code_hash],
            |row| {
                Ok((
                    AuthorizationCode {
                        client_id: row.get(0)?,
                        user_id: row.get(1)?,
                        redirect_uri: row.get(2)?,
                        code_challenge: row.get(3)?,
                        scope: row.get(4)?,
                    },
                    row.get::<_, i64>(5)? != 0,
                ))
            },
        )
        .ok()?;
    valid.then_some(code)
}

pub fn create_token(
    conn: &Connection,
    token_hash: &str,
    kind: &str,
    client_id: &str,
    user_id: i64,
    scope: &str,
    ttl_minutes: i64,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO oauth_tokens (token_hash, kind, client_id, user_id, scope, expires_at)
         VALUES (?1, ?2, ?3, ?4, ?5, datetime('now', ?6))",
        params![token_hash, kind, client_id, user_id, scope, format!("+{ttl_minutes} minutes")],
    )?;
    Ok(())
}

/// Looks up an unexpired token of the given kind belonging to an active user.
pub fn find_valid_token(conn: &Connection, token_hash: &str, kind: &str) -> Option<TokenGrant> {
    conn.query_row(
        "SELECT t.client_id, u.id, u.username, u.is_admin, u.is_manager, t.scope
         FROM oauth_tokens t
         JOIN users u ON u.id = t.user_id
         WHERE t.token_hash = ?1 AND t.kind = ?2
           AND t.expires_at > datetime('now') AND u.is_active = 1",
        params![token_hash, kind],
        |row| {
            Ok(TokenGrant {
                client_id: row.get(0)?,
                user_id: row.get(1)?,
                username: row.get(2)?,
                is_admin: row.get::<_, i64>(3)? != 0,
                is_manager: row.get::<_, i64>(4)? != 0,
                scope: row.get(5)?,
            })
        },
    )
    .ok()
}

/// Deletes a single token. Returns the number of rows removed.
pub fn delete_token(conn: &Connection, token_hash: &str) -> rusqlite::Result<usize> {
    conn.execute("DELETE FROM oauth_tokens WHERE token_hash = ?1", [token_hash])
}

/// Revokes every token a user has granted to a client.
pub fn delete_tokens_for(conn: &Connection, user_id: i64, client_id: &str) -> rusqlite::Result<()> {
    conn.execute(
        "DELETE FROM oauth_tokens WHERE user_id = ?1 AND client_id = ?2",
        params![user_id, client_id],
    )?;
    Ok(())
}

pub fn list_connections(conn: &Connection, user_id: i64) -> Vec<OAuthConnection> {
    let mut stmt = conn
        .prepare(
            "SELECT c.id, c.name, MAX(t.scope), MAX(t.created_at) AS last_issued
             FROM oauth_tokens t
             JOIN oauth_clients c ON c.id = t.client_id
             WHERE t.user_id = ?1 AND t.expires_at > datetime('now')
             GROUP BY c.id, c.name
             ORDER BY last_issued DESC",
        )
        .unwrap();
    stmt.query_map([user_id], |row| {
        Ok(OAuthConnection {
            client_id: row.get(0)?,
            client_name: row.get(1)?,
            scope: row.get(2)?,
            last_issued_at: row.get(3)?,
        })
    })
    .unwrap()
    .filter_map(|r| r.ok())
    .collect()
}

pub fn purge_expired(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "DELETE FROM oauth_codes WHERE expires_at <= datetime('now');
         DELETE FROM oauth_tokens WHERE expires_at <= datetime('now');",
    )
}
