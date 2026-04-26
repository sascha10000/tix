pub struct Status {
    pub id: i64,
    pub name: String,
    pub color: String,
    pub position: i64,
}

pub struct WorkflowTransition {
    pub from_status_id: i64,
    pub to_status_id: i64,
}
