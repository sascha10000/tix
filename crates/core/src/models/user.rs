pub struct User {
    pub id: i64,
    pub username: String,
    pub email: String,
    pub password_hash: String,
    pub is_admin: bool,
    pub is_manager: bool,
    pub created_at: String,
    pub is_active: bool,
}
