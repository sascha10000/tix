/// A "you got a ticket" event for one user, joined with what the browser needs to show it.
pub struct Notification {
    pub id: i64,
    pub ticket_id: i64,
    pub project_id: i64,
    pub project_name: String,
    pub ticket_title: String,
    /// `created` (new ticket assigned to the user) or `assigned` (existing ticket reassigned).
    pub kind: String,
    pub actor_name: Option<String>,
    pub created_at: String,
}
