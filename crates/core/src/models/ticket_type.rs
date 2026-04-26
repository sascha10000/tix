pub struct TicketType {
    pub id: i64,
    pub name: String,
    pub description: String,
}

pub struct CustomField {
    pub id: i64,
    pub ticket_type_id: i64,
    pub name: String,
    pub field_type: String,
    pub is_required: bool,
    pub position: i64,
    pub num_min: Option<f64>,
    pub num_max: Option<f64>,
    pub num_step: Option<f64>,
}
