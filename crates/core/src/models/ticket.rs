pub struct Ticket {
    pub id: i64,
    pub project_id: i64,
    pub ticket_type_id: i64,
    pub status_id: i64,
    pub creator_id: i64,
    pub assignee_id: i64,
    pub title: String,
    pub text: String,
    pub due_date: String,
    pub created_at: String,
    pub updated_at: String,
    pub is_deleted: bool,
    // joined fields
    pub creator_name: String,
    pub assignee_name: String,
    pub status_name: String,
    pub status_color: String,
    pub type_name: String,
}

pub struct FieldValue {
    pub custom_field_id: i64,
    pub field_name: String,
    pub field_type: String,
    pub value: String,
    pub is_required: bool,
    pub num_min: Option<f64>,
    pub num_max: Option<f64>,
    pub num_step: Option<f64>,
}
