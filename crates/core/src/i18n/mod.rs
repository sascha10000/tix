#[cfg(feature = "lang-en")]
mod en;
#[cfg(feature = "lang-de")]
mod de;
#[cfg(feature = "lang-es")]
mod es;

/// All UI strings, organized by section.
pub struct Translations {
    pub lang_code: &'static str,
    pub common: Common,
    pub nav: Nav,
    pub auth: Auth,
    pub dashboard: Dashboard,
    pub profile: Profile,
    pub projects: Projects,
    pub tickets: Tickets,
    pub admin: Admin,
}

pub struct Common {
    pub save: &'static str,
    pub cancel: &'static str,
    pub create: &'static str,
    pub edit: &'static str,
    pub delete: &'static str,
    pub remove: &'static str,
    pub add: &'static str,
    pub back: &'static str,
    pub yes: &'static str,
    pub no: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub position: &'static str,
    pub optional: &'static str,
    pub settings: &'static str,
    pub active: &'static str,
    pub type_label: &'static str,
    pub status: &'static str,
    pub required: &'static str,
}

pub struct Nav {
    pub projects: &'static str,
    pub users: &'static str,
    pub statuses: &'static str,
    pub types: &'static str,
    pub logout: &'static str,
    pub toggle_theme: &'static str,
    pub admin_role: &'static str,
}

pub struct Auth {
    pub login_title: &'static str,
    pub register_title: &'static str,
    pub username: &'static str,
    pub email: &'static str,
    pub password: &'static str,
    pub login_submit: &'static str,
    pub register_submit: &'static str,
    pub no_account: &'static str,
    pub has_account: &'static str,
    pub register_link: &'static str,
    pub login_link: &'static str,
}

pub struct Dashboard {
    pub title: &'static str,
    pub welcome: &'static str,
    pub your_projects: &'static str,
    pub no_projects: &'static str,
    pub create_one: &'static str,
    pub col_project: &'static str,
    pub col_description: &'static str,
    pub col_tickets: &'static str,
}

pub struct Profile {
    pub title: &'static str,
    pub account: &'static str,
    pub username: &'static str,
    pub email: &'static str,
    pub change_password: &'static str,
    pub current_password: &'static str,
    pub new_password: &'static str,
    pub change_password_btn: &'static str,
}

pub struct Projects {
    pub title: &'static str,
    pub new_project: &'static str,
    pub no_projects: &'static str,
    pub col_name: &'static str,
    pub col_description: &'static str,
    pub edit_title: &'static str,
    pub new_title: &'static str,
    pub view_tickets: &'static str,
    pub edit_project: &'static str,
    pub delete_project: &'static str,
    pub delete_confirm: &'static str,
    pub members: &'static str,
    pub col_user: &'static str,
    pub col_role: &'static str,
    pub role_manager: &'static str,
    pub role_member: &'static str,
    pub role_reporter: &'static str,
    pub active_statuses: &'static str,
    pub active_ticket_types: &'static str,
}

pub struct Tickets {
    pub title: &'static str,
    pub new_ticket: &'static str,
    pub project_settings: &'static str,
    pub no_tickets: &'static str,
    pub col_id: &'static str,
    pub col_title: &'static str,
    pub col_type: &'static str,
    pub col_status: &'static str,
    pub col_assignee: &'static str,
    pub col_due_date: &'static str,
    pub col_creator: &'static str,
    pub col_updated: &'static str,
    pub edit_title: &'static str,
    pub new_title: &'static str,
    pub title_label: &'static str,
    pub description_label: &'static str,
    pub assignee_label: &'static str,
    pub select_assignee: &'static str,
    pub due_date_label: &'static str,
    pub select_type: &'static str,
    pub no_types_active: &'static str,
    pub select_user: &'static str,
    pub select_ticket: &'static str,
    pub initial_status: &'static str,
    pub create_ticket: &'static str,
    pub detail_description: &'static str,
    pub detail_type: &'static str,
    pub detail_creator: &'static str,
    pub detail_assignee: &'static str,
    pub detail_due: &'static str,
    pub detail_created: &'static str,
    pub detail_updated: &'static str,
    pub fields: &'static str,
    pub change_status: &'static str,
    pub delete_confirm: &'static str,
    pub back_to_list: &'static str,
}

pub struct Admin {
    pub users: AdminUsers,
    pub statuses: AdminStatuses,
    pub ticket_types: AdminTicketTypes,
}

pub struct AdminUsers {
    pub title: &'static str,
    pub new_user: &'static str,
    pub col_username: &'static str,
    pub col_email: &'static str,
    pub col_admin: &'static str,
    pub col_manager: &'static str,
    pub col_active: &'static str,
    pub col_created: &'static str,
    pub deactivate: &'static str,
    pub deactivate_confirm: &'static str,
    pub new_title: &'static str,
    pub edit_title: &'static str,
    pub username: &'static str,
    pub email: &'static str,
    pub password: &'static str,
    pub admin_label: &'static str,
    pub manager_label: &'static str,
    pub active_label: &'static str,
    pub create_user: &'static str,
}

pub struct AdminStatuses {
    pub title: &'static str,
    pub new_status: &'static str,
    pub col_name: &'static str,
    pub col_color: &'static str,
    pub col_position: &'static str,
    pub delete_confirm: &'static str,
    pub edit_title: &'static str,
    pub new_title: &'static str,
    pub color_label: &'static str,
    pub workflow_title: &'static str,
    pub workflow_hint: &'static str,
    pub workflow_from_to: &'static str,
}

pub struct AdminTicketTypes {
    pub title: &'static str,
    pub new_type: &'static str,
    pub col_name: &'static str,
    pub col_description: &'static str,
    pub col_fields: &'static str,
    pub edit_title: &'static str,
    pub new_title: &'static str,
    pub custom_fields: &'static str,
    pub col_type: &'static str,
    pub col_required: &'static str,
    pub col_position: &'static str,
    pub col_constraints: &'static str,
    pub delete_field_confirm: &'static str,
    pub add_field: &'static str,
    pub field_text: &'static str,
    pub field_textarea: &'static str,
    pub field_number: &'static str,
    pub field_date: &'static str,
    pub field_user: &'static str,
    pub field_ticket: &'static str,
    pub pos: &'static str,
    pub placeholder: &'static str,
    pub default_value: &'static str,
    pub min: &'static str,
    pub max: &'static str,
    pub step: &'static str,
    pub copy_fields_from: &'static str,
    pub none_option: &'static str,
}

/// Returns a list of all compiled-in language codes.
pub fn available_languages() -> &'static [&'static str] {
    &[
        #[cfg(feature = "lang-en")]
        "en",
        #[cfg(feature = "lang-de")]
        "de",
        #[cfg(feature = "lang-es")]
        "es",
    ]
}

/// Get translations for a language code. Falls back to first available language.
pub fn get(lang: &str) -> &'static Translations {
    match lang {
        #[cfg(feature = "lang-en")]
        "en" => &en::TRANSLATIONS,
        #[cfg(feature = "lang-de")]
        "de" => &de::TRANSLATIONS,
        #[cfg(feature = "lang-es")]
        "es" => &es::TRANSLATIONS,
        _ => default(),
    }
}

/// Default translations (first compiled-in language).
pub fn default() -> &'static Translations {
    #[cfg(feature = "lang-en")]
    { return &en::TRANSLATIONS; }
    #[cfg(all(not(feature = "lang-en"), feature = "lang-de"))]
    { return &de::TRANSLATIONS; }
    #[cfg(all(not(feature = "lang-en"), not(feature = "lang-de"), feature = "lang-es"))]
    { return &es::TRANSLATIONS; }
    #[cfg(not(any(feature = "lang-en", feature = "lang-de", feature = "lang-es")))]
    compile_error!("At least one language feature must be enabled (lang-en, lang-de, or lang-es)");
}
