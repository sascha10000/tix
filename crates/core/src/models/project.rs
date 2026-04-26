pub struct Project {
    pub id: i64,
    pub name: String,
    pub description: String,
    pub created_by: Option<i64>,
    pub created_at: String,
}

pub struct ProjectMember {
    pub user_id: i64,
    pub username: String,
    pub role: String,
}
