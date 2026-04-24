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
