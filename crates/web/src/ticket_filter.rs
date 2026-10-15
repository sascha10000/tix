//! Ticket filtering and the overdue rule, shared by the web overviews and the MCP tools.

use chrono::NaiveDate;
use serde::Deserialize;

use ticketsystem_core::models::ticket::Ticket;

/// Today's date in the server's local time zone.
pub fn today() -> NaiveDate {
    chrono::Local::now().date_naive()
}

/// A ticket is overdue when its due date has passed and its status is not closed.
pub fn is_overdue(t: &Ticket, today: NaiveDate) -> bool {
    !t.status_closed
        && NaiveDate::parse_from_str(&t.due_date, "%Y-%m-%d").is_ok_and(|due| due < today)
}

/// Filter criteria; all set criteria must match (AND).
#[derive(Default)]
pub struct TicketFilter {
    pub project_id: Option<i64>,
    pub status_id: Option<i64>,
    pub assignee_id: Option<i64>,
    pub ticket_type_id: Option<i64>,
    /// Lower-cased search text, matched against title and description.
    pub query: Option<String>,
    pub only_open: bool,
    pub only_overdue: bool,
}

impl TicketFilter {
    pub fn with_query(mut self, query: Option<&str>) -> Self {
        self.query = query
            .map(str::trim)
            .filter(|q| !q.is_empty())
            .map(str::to_lowercase);
        self
    }

    pub fn matches(&self, t: &Ticket, today: NaiveDate) -> bool {
        self.project_id.is_none_or(|id| t.project_id == id)
            && self.status_id.is_none_or(|id| t.status_id == id)
            && self.assignee_id.is_none_or(|id| t.assignee_id == id)
            && self.ticket_type_id.is_none_or(|id| t.ticket_type_id == id)
            && (!self.only_open || !t.status_closed)
            && (!self.only_overdue || is_overdue(t, today))
            && self.query.as_deref().is_none_or(|q| {
                t.title.to_lowercase().contains(q) || t.text.to_lowercase().contains(q)
            })
    }
}

/// Filter parameters as sent by the HTML filter forms. Select boxes send an empty
/// string for "All" and checkboxes send "on" only when ticked, so everything is
/// read as optional text and parsed leniently.
#[derive(Deserialize, Default)]
pub struct FilterQuery {
    pub project: Option<String>,
    pub status: Option<String>,
    pub assignee: Option<String>,
    #[serde(rename = "type")]
    pub ticket_type: Option<String>,
    pub q: Option<String>,
    pub open: Option<String>,
    pub overdue: Option<String>,
}

fn parse_id(v: &Option<String>) -> Option<i64> {
    v.as_deref().and_then(|s| s.trim().parse().ok())
}

fn is_ticked(v: &Option<String>) -> bool {
    v.as_deref().is_some_and(|s| !s.is_empty())
}

impl FilterQuery {
    pub fn to_filter(&self) -> TicketFilter {
        TicketFilter {
            project_id: parse_id(&self.project),
            status_id: parse_id(&self.status),
            assignee_id: parse_id(&self.assignee),
            ticket_type_id: parse_id(&self.ticket_type),
            only_open: is_ticked(&self.open),
            only_overdue: is_ticked(&self.overdue),
            ..TicketFilter::default()
        }
        .with_query(self.q.as_deref())
    }

    /// The search text to re-display in the form.
    pub fn search_text(&self) -> String {
        self.q.clone().unwrap_or_default()
    }
}

/// An option of a filter `<select>`, with its selection state precomputed for the template.
pub struct SelectOption {
    pub id: i64,
    pub label: String,
    pub selected: bool,
}

impl SelectOption {
    pub fn list(items: impl IntoIterator<Item = (i64, String)>, selected: Option<i64>) -> Vec<Self> {
        items
            .into_iter()
            .map(|(id, label)| SelectOption { id, label, selected: selected == Some(id) })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ticket(due: &str, closed: bool) -> Ticket {
        Ticket {
            id: 1,
            project_id: 1,
            ticket_type_id: 1,
            status_id: 1,
            creator_id: 1,
            assignee_id: 2,
            title: "Login broken".into(),
            text: "500 on submit".into(),
            due_date: due.into(),
            created_at: String::new(),
            updated_at: String::new(),
            is_deleted: false,
            creator_name: String::new(),
            assignee_name: String::new(),
            status_name: String::new(),
            status_color: String::new(),
            status_closed: closed,
            type_name: String::new(),
        }
    }

    fn day(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    #[test]
    fn overdue_rule() {
        let today = day("2026-10-15");
        assert!(is_overdue(&ticket("2026-10-14", false), today));
        assert!(!is_overdue(&ticket("2026-10-15", false), today), "due today is not overdue yet");
        assert!(!is_overdue(&ticket("2026-10-16", false), today));
        assert!(!is_overdue(&ticket("2026-10-01", true), today), "closed tickets are never overdue");
        assert!(!is_overdue(&ticket("not a date", false), today));
    }

    #[test]
    fn form_parameters_are_parsed_leniently() {
        let q = FilterQuery {
            status: Some(String::new()),
            assignee: Some("2".into()),
            q: Some("  LOGIN ".into()),
            open: Some("on".into()),
            ..FilterQuery::default()
        };
        let f = q.to_filter();
        assert_eq!(f.status_id, None, "empty select means 'All'");
        assert_eq!(f.assignee_id, Some(2));
        assert_eq!(f.query.as_deref(), Some("login"));
        assert!(f.only_open && !f.only_overdue);
    }

    #[test]
    fn filter_combines_criteria() {
        let today = day("2026-10-15");
        let open_overdue = ticket("2026-10-01", false);
        let closed = ticket("2026-10-01", true);

        let f = TicketFilter { only_open: true, ..TicketFilter::default() };
        assert!(f.matches(&open_overdue, today) && !f.matches(&closed, today));

        let f = TicketFilter { only_overdue: true, assignee_id: Some(2), ..TicketFilter::default() };
        assert!(f.matches(&open_overdue, today));
        let f = TicketFilter { assignee_id: Some(3), ..TicketFilter::default() };
        assert!(!f.matches(&open_overdue, today));

        let f = TicketFilter::default().with_query(Some("submit"));
        assert!(f.matches(&open_overdue, today), "searches the description too");
        let f = TicketFilter::default().with_query(Some("nope"));
        assert!(!f.matches(&open_overdue, today));
    }
}
