//! Project and ticket permission rules, shared by the web handlers and the MCP tools.

use ticketsystem_core::models::project::Project;
use ticketsystem_db::repo::project;
use ticketsystem_db::rusqlite::Connection;

use crate::errors::AppError;
use crate::middleware::AuthenticatedUser;

/// Loads the project and ensures the user is an admin or a project member.
pub fn check_project_access(
    conn: &Connection,
    user: &AuthenticatedUser,
    project_id: i64,
) -> Result<Project, AppError> {
    let p = project::find_by_id(conn, project_id)
        .ok_or(AppError::NotFound("Project not found".into()))?;
    if !user.is_admin() && !project::is_member(conn, project_id, user.id) {
        return Err(AppError::Forbidden);
    }
    Ok(p)
}

pub fn can_edit_ticket(user: &AuthenticatedUser, project_role: Option<&str>, ticket_creator_id: i64) -> bool {
    user.is_admin()
        || matches!(project_role, Some("manager" | "member"))
        || user.id == ticket_creator_id
}

pub fn can_transition_ticket(user: &AuthenticatedUser, project_role: Option<&str>) -> bool {
    user.is_admin() || matches!(project_role, Some("manager" | "member"))
}
