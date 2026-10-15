pub struct Status {
    pub id: i64,
    pub name: String,
    pub color: String,
    pub position: i64,
    /// A closed status means the ticket is done (excluded from "open" and overdue).
    pub is_closed: bool,
}

pub struct WorkflowTransition {
    pub from_status_id: i64,
    pub to_status_id: i64,
}
