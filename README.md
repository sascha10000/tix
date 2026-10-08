# Ticketsystem

A self-hosted, lightweight ticket management system built with Rust. It uses server-side rendering, SQLite for storage, and ships as a single binary with no external dependencies.

## Features

### Ticket Management
- Create, edit, and delete tickets within projects
- Every ticket requires an **Assignee** and a **Due Date**
- Configurable ticket types with custom fields (text, number, date, textarea, user reference, ticket reference)
- Custom fields can be marked as required per ticket type
- Status transitions governed by a configurable workflow matrix

### Projects
- Organize tickets into projects
- Per-project activation of statuses and ticket types
- Member management with project-level roles: **Manager**, **Member**, **Reporter**
- Only project members (and admins) can access a project's tickets

### User Management
- User registration and login with session-based authentication
- Passwords hashed with Argon2
- Global roles: **Admin** (full system access), **Manager** (can create projects)
- User profiles with password change support
- Accounts can be deactivated by admins

### Administration
- Manage users, statuses, and ticket types from the admin panel
- Define statuses with custom colors and display order
- Build a workflow matrix to control which status transitions are allowed
- Create ticket types and attach custom fields with validation constraints (min/max/step for numbers)

## MCP Server

The server exposes a [Model Context Protocol](https://modelcontextprotocol.io) endpoint at `/mcp` (Streamable HTTP transport, stateless JSON responses), so AI clients such as Claude Code, Claude.ai connectors, Cursor or MCP Inspector can work with tickets.

### Connecting

Only the URL is needed. The client discovers the OAuth server, registers itself, and opens a browser window. Log in with your normal account and approve the access on the consent page.

```sh
# Claude Code
claude mcp add --transport http ticketsystem http://127.0.0.1:8080/mcp
# then run /mcp inside Claude Code to authenticate

# MCP Inspector
npx @modelcontextprotocol/inspector   # transport: Streamable HTTP, URL: http://127.0.0.1:8080/mcp
```

Connected apps are listed on the **Profile** page, where they can be revoked. Deactivating a user invalidates their tokens immediately.

### Tools

Every tool runs as the authorizing user with the same project-membership and role rules as the web UI.

| Tool | Scope | Description |
|---|---|---|
| `list_projects` | `tickets:read` | Projects you can access, with your role |
| `get_project` | `tickets:read` | Active statuses, allowed transitions, ticket types with custom fields, members |
| `list_tickets` | `tickets:read` | Tickets in a project; filter by status, assignee, type, text; paginated |
| `get_ticket` | `tickets:read` | Full ticket with custom fields, allowed transitions and your permissions |
| `list_my_tickets` | `tickets:read` | Tickets assigned to or created by you across projects |
| `create_ticket` | `tickets:write` | Create a ticket (validates type, status, assignee, required/numeric/date fields) |
| `update_ticket` | `tickets:write` | Partially update title, description, assignee, due date, custom fields |
| `transition_ticket` | `tickets:write` | Change status along the configured workflow |

Tickets cannot be deleted through MCP.

### Authorization

The MCP endpoint implements the [MCP authorization spec](https://modelcontextprotocol.io/specification/2025-06-18/basic/authorization), i.e. OAuth 2.1 with PKCE:

| Endpoint | Purpose |
|---|---|
| `GET /.well-known/oauth-protected-resource` | Protected resource metadata (RFC 9728) |
| `GET /.well-known/oauth-authorization-server` | Authorization server metadata (RFC 8414) |
| `POST /oauth/register` | Dynamic client registration (RFC 7591), public clients only |
| `GET/POST /oauth/authorize` | Login + consent; authorization code grant with mandatory PKCE `S256` |
| `POST /oauth/token` | Code exchange and refresh-token rotation |
| `POST /oauth/revoke` | Token revocation (RFC 7009) |

Tokens are opaque random values; only their SHA-256 hashes are stored. Redirect URIs must be `https://` or `http://` on a loopback host. When running behind a reverse proxy, set `PUBLIC_BASE_URL` to the public URL. Clients that run in the browser also need the proxy to pass through the CORS headers.

## Tech Stack

- **Rust** with [Actix-web](https://actix.rs/) (HTTP server)
- **SQLite** via rusqlite with r2d2 connection pooling
- **Askama** templates (server-side HTML rendering)
- **Argon2** password hashing
- Embedded CSS — no external asset pipeline required

## Setup

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) (edition 2024)

### Configuration

Copy the example environment file and adjust as needed:

```sh
cp .env.example .env
```

Available environment variables:

| Variable | Default | Description |
|---|---|---|
| `DB_NAME` | `ticketsystem.db` | SQLite database file path |
| `BIND_ADDRESS` | `127.0.0.1:8080` | Host and port to listen on |
| `DB_POOL_SIZE` | `4` | Number of database connections |
| `SESSION_DURATION_HOURS` | `24` | Session lifetime in hours |
| `ADMIN_DEFAULT_USERNAME` | `admin` | Initial admin username |
| `ADMIN_DEFAULT_EMAIL` | `admin@localhost` | Initial admin email |
| `ADMIN_DEFAULT_PASSWORD` | `admin` | Initial admin password |
| `PUBLIC_BASE_URL` | `http://{BIND_ADDRESS}` | Externally reachable URL (OAuth issuer / MCP resource). Set this behind a reverse proxy |
| `OAUTH_ACCESS_TOKEN_MINUTES` | `60` | Lifetime of MCP access tokens |
| `OAUTH_REFRESH_TOKEN_DAYS` | `30` | Lifetime of MCP refresh tokens (rotated on every use) |

### Build and Run

```sh
cargo build --release
./target/release/ticketsystem
```

The server starts at the configured bind address (default: `http://127.0.0.1:8080`).

On first launch, the database and schema are created automatically and an admin user is seeded with the configured credentials.

### Database Migrations

For existing databases that need schema upgrades:

```sh
./target/release/ticketsystem --migrate
```

Migrations run automatically in order and are tracked via SQLite's `user_version` pragma.

## Getting Started

1. Log in with the default admin credentials
2. Create **statuses** (e.g. Open, In Progress, Done) and set up the **workflow** transitions between them
3. Create **ticket types** (e.g. Bug, Feature, Task) and add custom fields as needed
4. Create a **project**, activate the desired statuses and ticket types, and add team members
5. Start creating tickets
