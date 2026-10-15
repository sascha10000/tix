//! Notifications when a ticket is created for, or assigned to, a user.
//!
//! Each event is stored in `notifications` (the browser polls `/notifications` and
//! shows a desktop notification) and, when SMTP is configured, sent as an e-mail in
//! a background task. Failures are logged and never fail the triggering request.

use lettre::message::Mailbox;
use lettre::message::header::ContentType;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

use ticketsystem_core::config::SmtpConfig;
use ticketsystem_core::i18n::{self, Translations};
use ticketsystem_db::repo::{notification, project, ticket, user};
use ticketsystem_db::rusqlite::Connection;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    /// A new ticket was created with the user as assignee.
    Created,
    /// An existing ticket was reassigned to the user.
    Assigned,
}

impl Kind {
    fn as_str(self) -> &'static str {
        match self {
            Kind::Created => "created",
            Kind::Assigned => "assigned",
        }
    }
}

struct Mailer {
    transport: AsyncSmtpTransport<Tokio1Executor>,
    from: Mailbox,
}

pub struct Notifier {
    mailer: Option<Mailer>,
    base_url: String,
    lang: &'static Translations,
}

impl Notifier {
    /// Builds the notifier; e-mail is enabled only when `smtp` is configured.
    pub fn new(smtp: Option<&SmtpConfig>, mail_lang: Option<&str>, base_url: &str) -> Result<Self, String> {
        let lang = match mail_lang {
            Some(code) if i18n::available_languages().contains(&code) => i18n::get(code),
            Some(code) => return Err(format!("MAIL_LANG '{code}' is not a compiled-in language")),
            None => i18n::default(),
        };
        let mailer = smtp.map(build_mailer).transpose()?;
        Ok(Self { mailer, base_url: base_url.to_string(), lang })
    }

    pub fn mail_enabled(&self) -> bool {
        self.mailer.is_some()
    }

    /// Records the notification and e-mails the assignee. Does nothing when users
    /// assign tickets to themselves.
    pub fn ticket_assigned(&self, conn: &Connection, ticket_id: i64, assignee_id: i64, actor_id: i64, kind: Kind) {
        if assignee_id == actor_id {
            return;
        }
        if let Err(e) = notification::create(conn, assignee_id, ticket_id, kind.as_str(), Some(actor_id)) {
            eprintln!("Failed to store notification for ticket {ticket_id}: {e}");
        }

        let Some(mailer) = &self.mailer else { return };
        let (Some(recipient), Some(t)) = (user::find_by_id(conn, assignee_id), ticket::find_by_id(conn, ticket_id)) else {
            return;
        };
        if !recipient.is_active || !recipient.email_notifications {
            return;
        }
        let actor = user::find_by_id(conn, actor_id).map(|u| u.username).unwrap_or_default();
        let project_name = project::find_by_id(conn, t.project_id).map(|p| p.name).unwrap_or_default();
        let link = format!("{}/projects/{}/tickets/{}", self.base_url, t.project_id, t.id);
        let email = render_email(self.lang, kind, &actor, &t.title, &project_name, &t.due_date, &link);

        let to = match recipient.email.parse() {
            Ok(address) => Mailbox::new(Some(recipient.username.clone()), address),
            Err(e) => {
                eprintln!("Not e-mailing user {assignee_id}: invalid address '{}': {e}", recipient.email);
                return;
            }
        };
        let message = match Message::builder()
            .from(mailer.from.clone())
            .to(to)
            .subject(email.subject)
            // Without this, clients assume US-ASCII and garble umlauts.
            .header(ContentType::TEXT_PLAIN)
            .body(email.body)
        {
            Ok(m) => m,
            Err(e) => {
                eprintln!("Failed to build notification e-mail: {e}");
                return;
            }
        };

        let transport = mailer.transport.clone();
        actix_web::rt::spawn(async move {
            if let Err(e) = transport.send(message).await {
                eprintln!("Failed to send notification e-mail: {e}");
            }
        });
    }
}

fn build_mailer(smtp: &SmtpConfig) -> Result<Mailer, String> {
    let builder = match smtp.security.as_str() {
        "starttls" => AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&smtp.host),
        "tls" => AsyncSmtpTransport::<Tokio1Executor>::relay(&smtp.host),
        "none" => Ok(AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&smtp.host)),
        other => return Err(format!("SMTP_SECURITY must be starttls, tls or none, got '{other}'")),
    }
    .map_err(|e| format!("invalid SMTP_HOST '{}': {e}", smtp.host))?;

    let mut builder = builder;
    if let Some(port) = smtp.port {
        builder = builder.port(port);
    }
    if let (Some(username), Some(password)) = (&smtp.username, &smtp.password) {
        builder = builder.credentials(Credentials::new(username.clone(), password.clone()));
    }
    let from = smtp
        .from
        .parse()
        .map_err(|e| format!("invalid SMTP_FROM '{}': {e}", smtp.from))?;
    Ok(Mailer { transport: builder.build(), from })
}

pub struct RenderedEmail {
    pub subject: String,
    pub body: String,
}

/// Plain-text notification e-mail in the configured language.
pub fn render_email(
    t: &Translations,
    kind: Kind,
    actor: &str,
    title: &str,
    project: &str,
    due: &str,
    link: &str,
) -> RenderedEmail {
    let n = &t.notifications;
    // Titles are user input: keep the subject on one line.
    let one_line_title = title.replace(['\r', '\n'], " ");
    let (subject, intro) = match kind {
        Kind::Created => (n.mail_subject_created, n.mail_intro_created),
        Kind::Assigned => (n.mail_subject_assigned, n.mail_intro_assigned),
    };
    let body = format!(
        "{intro}\n\n{title}\n{label_project}: {project}\n{label_due}: {due}\n\n{open}: {link}\n\n-- \n{footer}\n",
        intro = intro.replace("{actor}", actor),
        label_project = n.mail_project,
        label_due = n.mail_due,
        open = n.mail_open,
        footer = n.mail_footer,
    );
    RenderedEmail { subject: subject.replace("{title}", &one_line_title), body }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ticketsystem_core::config::SmtpConfig;

    #[test]
    fn email_text_is_filled_in() {
        let t = i18n::get("de");
        let e = render_email(t, Kind::Created, "umeyer", "Login\r\nbroken", "Website", "2026-11-01", "https://x/projects/1/tickets/2");
        assert_eq!(e.subject, "Neues Ticket: Login  broken");
        assert!(e.body.starts_with("umeyer hat ein neues Ticket f\u{00fc}r dich erstellt:"));
        assert!(e.body.contains("Projekt: Website") && e.body.contains("F\u{00e4}llig: 2026-11-01"));
        assert!(e.body.contains("https://x/projects/1/tickets/2"));
    }

    #[test]
    fn configuration_errors_are_reported() {
        let smtp = |security: &str, from: &str| SmtpConfig {
            host: "smtp.example.com".into(),
            port: None,
            username: None,
            password: None,
            from: from.into(),
            security: security.into(),
        };
        assert!(Notifier::new(None, None, "https://x").is_ok_and(|n| !n.mail_enabled()));
        assert!(Notifier::new(Some(&smtp("starttls", "Tickets <t@example.com>")), Some("de"), "https://x")
            .is_ok_and(|n| n.mail_enabled()));
        assert!(Notifier::new(Some(&smtp("ssl", "t@example.com")), None, "https://x").is_err());
        assert!(Notifier::new(Some(&smtp("tls", "not an address")), None, "https://x").is_err());
        assert!(Notifier::new(None, Some("xx"), "https://x").is_err());
    }
}
