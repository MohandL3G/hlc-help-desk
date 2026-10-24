use askama::Template;
use axum::{
    extract::{Form, Path, State},
    http::StatusCode,
    response::{Html, Redirect},
    routing::{get, post},
    Router,
};
use chrono::Local;
use serde::Deserialize;
use sqlx::{
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    FromRow, SqlitePool,
};
use std::{env, net::SocketAddr, path::Path as FsPath};
use tower_http::services::ServeDir;
use tracing_subscriber::EnvFilter;

type AppError = (StatusCode, String);
const ACTOR: &str = "فريق تقنية المعلومات";
const TICKET_COLUMNS: &str = "id, requester, department, title, description, category, priority, channel, status, resolution_note, created_at, updated_at";

#[derive(Debug, FromRow)]
struct TicketRow {
    id: i64,
    requester: String,
    department: String,
    title: String,
    description: String,
    category: String,
    priority: String,
    channel: String,
    status: String,
    resolution_note: String,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, FromRow)]
struct EventRow {
    actor: String,
    event_type: String,
    details: String,
    created_at: String,
}

#[derive(Clone)]
struct TicketView {
    id: i64,
    number: String,
    requester: String,
    department: String,
    title: String,
    description: String,
    category_label: String,
    priority_label: String,
    priority_class: String,
    channel_label: String,
    status: String,
    status_label: String,
    status_class: String,
    resolution_note: String,
    created_at: String,
    updated_at: String,
}

struct EventView {
    actor: String,
    label: String,
    details: String,
    created_at: String,
}

#[derive(Template)]
#[template(path = "dashboard.html")]
struct DashboardTemplate {
    page_title: &'static str,
    active_page: &'static str,
    today: String,
    total_count: i64,
    open_count: i64,
    waiting_count: i64,
    resolved_count: i64,
    tickets: Vec<TicketView>,
    has_tickets: bool,
}

#[derive(Template)]
#[template(path = "ticket_list.html")]
struct TicketListTemplate {
    page_title: &'static str,
    active_page: &'static str,
    today: String,
    total_count: i64,
    tickets: Vec<TicketView>,
    has_tickets: bool,
}

#[derive(Template)]
#[template(path = "new_ticket.html")]
struct NewTicketTemplate {
    page_title: &'static str,
    active_page: &'static str,
    today: String,
}

#[derive(Template)]
#[template(path = "ticket_detail.html")]
struct TicketDetailTemplate {
    page_title: &'static str,
    active_page: &'static str,
    today: String,
    ticket: TicketView,
    events: Vec<EventView>,
    has_events: bool,
}

#[derive(Deserialize)]
struct NewTicketForm {
    requester: String,
    department: String,
    title: String,
    description: String,
    category: String,
    priority: String,
    channel: String,
}

#[derive(Deserialize)]
struct UpdateStatusForm {
    status: String,
    note: String,
}

fn now_string() -> String {
    Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

fn today_string() -> String {
    Local::now().format("%Y-%m-%d").to_string()
}

fn db_error(error: sqlx::Error) -> AppError {
    tracing::error!(error = %error, "database operation failed");
    (StatusCode::INTERNAL_SERVER_ERROR, "حدث خطأ داخلي أثناء معالجة الطلب.".to_string())
}

fn render<T: Template>(template: T) -> Result<Html<String>, AppError> {
    template.render().map(Html).map_err(|error| {
        tracing::error!(error = %error, "template rendering failed");
        (StatusCode::INTERNAL_SERVER_ERROR, "تعذر عرض الصفحة.".to_string())
    })
}

fn ticket_view(row: TicketRow) -> TicketView {
    let status_label = match row.status.as_str() {
        "in_progress" => "قيد التنفيذ",
        "waiting" => "بانتظار رد",
        "resolved" => "مكتمل",
        _ => "جديد",
    };
    let status_class = match row.status.as_str() {
        "in_progress" => "status-progress",
        "waiting" => "status-waiting",
        "resolved" => "status-resolved",
        _ => "status-new",
    };
    let (priority_label, priority_class) = match row.priority.as_str() {
        "urgent" => ("عاجل", "priority-urgent"),
        "high" => ("مرتفع", "priority-high"),
        _ => ("عادي", "priority-normal"),
    };
    let category_label = match row.category.as_str() {
        "hardware" => "أجهزة ومعدات",
        "software" => "برامج وتطبيقات",
        "network" => "شبكة وإنترنت",
        "account" => "حسابات وصلاحيات",
        _ => "أخرى",
    };
    let channel_label = match row.channel.as_str() {
        "paper" => "ورقي",
        "phone" => "هاتف / حضور شخصي",
        "whatsapp" => "واتساب",
        _ => "بوابة النظام",
    };
    TicketView {
        id: row.id,
        number: format!("IT-{:06}", row.id),
        requester: row.requester,
        department: row.department,
        title: row.title,
        description: row.description,
        category_label: category_label.to_string(),
        priority_label: priority_label.to_string(),
        priority_class: priority_class.to_string(),
        channel_label: channel_label.to_string(),
        status: row.status,
        status_label: status_label.to_string(),
        status_class: status_class.to_string(),
        resolution_note: row.resolution_note,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

fn valid_choice(value: &str, choices: &[&str]) -> bool {
    choices.contains(&value)
}

async fn init_db(db: &SqlitePool) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"CREATE TABLE IF NOT EXISTS tickets (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            requester TEXT NOT NULL,
            department TEXT NOT NULL,
            title TEXT NOT NULL,
            description TEXT NOT NULL,
            category TEXT NOT NULL,
            priority TEXT NOT NULL CHECK(priority IN ('normal','high','urgent')),
            channel TEXT NOT NULL CHECK(channel IN ('portal','paper','phone','whatsapp')),
            status TEXT NOT NULL DEFAULT 'new' CHECK(status IN ('new','in_progress','waiting','resolved')),
            resolution_note TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )"#,
    )
    .execute(db)
    .await?;

    sqlx::query(
        r#"CREATE TABLE IF NOT EXISTS ticket_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            ticket_id INTEGER NOT NULL REFERENCES tickets(id),
            actor TEXT NOT NULL,
            event_type TEXT NOT NULL,
            details TEXT NOT NULL,
            created_at TEXT NOT NULL
        )"#,
    )
    .execute(db)
    .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS tickets_status_idx ON tickets(status)")
        .execute(db)
        .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS tickets_created_idx ON tickets(created_at DESC)")
        .execute(db)
        .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS ticket_events_ticket_idx ON ticket_events(ticket_id, id DESC)")
        .execute(db)
        .await?;
    Ok(())
}

async fn dashboard(State(db): State<SqlitePool>) -> Result<Html<String>, AppError> {
    let total_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tickets")
        .fetch_one(&db).await.map_err(db_error)?;
    let open_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tickets WHERE status IN ('new','in_progress')")
        .fetch_one(&db).await.map_err(db_error)?;
    let waiting_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tickets WHERE status = 'waiting'")
        .fetch_one(&db).await.map_err(db_error)?;
    let resolved_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tickets WHERE status = 'resolved'")
        .fetch_one(&db).await.map_err(db_error)?;
    let sql = format!("SELECT {TICKET_COLUMNS} FROM tickets ORDER BY id DESC LIMIT 8");
    let rows = sqlx::query_as::<_, TicketRow>(&sql).fetch_all(&db).await.map_err(db_error)?;
    let tickets: Vec<TicketView> = rows.into_iter().map(ticket_view).collect();
    let has_tickets = !tickets.is_empty();
    render(DashboardTemplate {
        page_title: "لوحة التحكم",
        active_page: "dashboard",
        today: today_string(),
        total_count,
        open_count,
        waiting_count,
        resolved_count,
        tickets,
        has_tickets,
    })
}

async fn ticket_list(State(db): State<SqlitePool>) -> Result<Html<String>, AppError> {
    let sql = format!("SELECT {TICKET_COLUMNS} FROM tickets ORDER BY id DESC LIMIT 200");
    let rows = sqlx::query_as::<_, TicketRow>(&sql).fetch_all(&db).await.map_err(db_error)?;
    let tickets: Vec<TicketView> = rows.into_iter().map(ticket_view).collect();
    let total_count = tickets.len() as i64;
    let has_tickets = !tickets.is_empty();
    render(TicketListTemplate {
        page_title: "جميع الطلبات",
        active_page: "tickets",
        today: today_string(),
        total_count,
        tickets,
        has_tickets,
    })
}

async fn new_ticket() -> Result<Html<String>, AppError> {
    render(NewTicketTemplate {
        page_title: "تسجيل طلب جديد",
        active_page: "new",
        today: today_string(),
    })
}

async fn create_ticket(
    State(db): State<SqlitePool>,
    Form(form): Form<NewTicketForm>,
) -> Result<Redirect, AppError> {
    let requester = form.requester.trim().to_string();
    let department = form.department.trim().to_string();
    let title = form.title.trim().to_string();
    let description = form.description.trim().to_string();
    let category = form.category.trim().to_string();
    let priority = form.priority.trim().to_string();
    let channel = form.channel.trim().to_string();

    if requester.is_empty() || department.is_empty() || title.is_empty() || description.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "يرجى تعبئة الحقول المطلوبة.".to_string()));
    }
    if requester.chars().count() > 120 || department.chars().count() > 120 || title.chars().count() > 180 || description.chars().count() > 5000 {
        return Err((StatusCode::BAD_REQUEST, "تجاوز أحد الحقول الحد المسموح به.".to_string()));
    }
    if !valid_choice(&category, &["hardware", "software", "network", "account", "other"])
        || !valid_choice(&priority, &["normal", "high", "urgent"])
        || !valid_choice(&channel, &["portal", "paper", "phone", "whatsapp"])
    {
        return Err((StatusCode::BAD_REQUEST, "قيمة أحد الحقول غير صالحة.".to_string()));
    }

    let created_at = now_string();
    let mut tx = db.begin().await.map_err(db_error)?;
    let result = sqlx::query(
        "INSERT INTO tickets (requester, department, title, description, category, priority, channel, status, resolution_note, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, 'new', '', ?, ?)",
    )
    .bind(&requester)
    .bind(&department)
    .bind(&title)
    .bind(&description)
    .bind(&category)
    .bind(&priority)
    .bind(&channel)
    .bind(&created_at)
    .bind(&created_at)
    .execute(&mut *tx)
    .await
    .map_err(db_error)?;
    let ticket_id = result.last_insert_rowid();
    sqlx::query("INSERT INTO ticket_events (ticket_id, actor, event_type, details, created_at) VALUES (?, ?, 'created', ?, ?)")
        .bind(ticket_id)
        .bind(ACTOR)
        .bind("تم تسجيل الطلب في النظام.")
        .bind(&created_at)
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;
    tx.commit().await.map_err(db_error)?;

    Ok(Redirect::to(&format!("/tickets/{ticket_id}")))
}

async fn ticket_detail(
    State(db): State<SqlitePool>,
    Path(id): Path<i64>,
) -> Result<Html<String>, AppError> {
    let sql = format!("SELECT {TICKET_COLUMNS} FROM tickets WHERE id = ?");
    let row = sqlx::query_as::<_, TicketRow>(&sql)
        .bind(id)
        .fetch_optional(&db)
        .await
        .map_err(db_error)?
        .ok_or((StatusCode::NOT_FOUND, "الطلب غير موجود.".to_string()))?;
    let ticket = ticket_view(row);
    let events_rows = sqlx::query_as::<_, EventRow>(
        "SELECT actor, event_type, details, created_at FROM ticket_events WHERE ticket_id = ? ORDER BY id DESC",
    )
    .bind(id)
    .fetch_all(&db)
    .await
    .map_err(db_error)?;
    let events: Vec<EventView> = events_rows
        .into_iter()
        .map(|event| EventView {
            actor: event.actor,
            label: if event.event_type == "created" { "إنشاء الطلب" } else { "تحديث الطلب" }.to_string(),
            details: event.details,
            created_at: event.created_at,
        })
        .collect();
    let has_events = !events.is_empty();
    render(TicketDetailTemplate {
        page_title: "تفاصيل الطلب",
        active_page: "tickets",
        today: today_string(),
        ticket,
        events,
        has_events,
    })
}

async fn update_ticket_status(
    State(db): State<SqlitePool>,
    Path(id): Path<i64>,
    Form(form): Form<UpdateStatusForm>,
) -> Result<Redirect, AppError> {
    let status = form.status.trim().to_string();
    let note = form.note.trim().to_string();
    if !valid_choice(&status, &["new", "in_progress", "waiting", "resolved"]) {
        return Err((StatusCode::BAD_REQUEST, "حالة الطلب غير صالحة.".to_string()));
    }
    if note.chars().count() > 2000 {
        return Err((StatusCode::BAD_REQUEST, "ملاحظة الإجراء طويلة جداً.".to_string()));
    }
    let current_status: Option<String> = sqlx::query_scalar("SELECT status FROM tickets WHERE id = ?")
        .bind(id)
        .fetch_optional(&db)
        .await
        .map_err(db_error)?;
    let current_status = current_status.ok_or((StatusCode::NOT_FOUND, "الطلب غير موجود.".to_string()))?;
    let timestamp = now_string();
    let status_change = format!("تغيّرت الحالة من «{}» إلى «{}».", status_label(&current_status), status_label(&status));
    let details = if note.is_empty() { status_change } else { format!("{status_change} ملاحظة: {note}") };

    let mut tx = db.begin().await.map_err(db_error)?;
    sqlx::query("UPDATE tickets SET status = ?, updated_at = ?, resolution_note = CASE WHEN ? = 'resolved' AND ? <> '' THEN ? ELSE resolution_note END WHERE id = ?")
        .bind(&status)
        .bind(&timestamp)
        .bind(&status)
        .bind(&note)
        .bind(&note)
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;
    sqlx::query("INSERT INTO ticket_events (ticket_id, actor, event_type, details, created_at) VALUES (?, ?, 'status_change', ?, ?)")
        .bind(id)
        .bind(ACTOR)
        .bind(details)
        .bind(&timestamp)
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;
    tx.commit().await.map_err(db_error)?;
    Ok(Redirect::to(&format!("/tickets/{id}")))
}

fn status_label(status: &str) -> &'static str {
    match status {
        "in_progress" => "قيد التنفيذ",
        "waiting" => "بانتظار رد",
        "resolved" => "مكتمل",
        _ => "جديد",
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .init();

    let db_path = env::var("DB_PATH").unwrap_or_else(|_| "data/tickets.db".to_string());
    if let Some(parent) = FsPath::new(&db_path).parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let options = SqliteConnectOptions::new()
        .filename(&db_path)
        .create_if_missing(true)
        .foreign_keys(true);
    let db = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await?;
    init_db(&db).await?;

    let app = Router::new()
        .route("/", get(dashboard))
        .route("/tickets", get(ticket_list).post(create_ticket))
        .route("/tickets/new", get(new_ticket))
        .route("/tickets/{id}", get(ticket_detail))
        .route("/tickets/{id}/status", post(update_ticket_status))
        .nest_service("/static", ServeDir::new("static"))
        .with_state(db);

    let port = env::var("PORT").ok().and_then(|value| value.parse::<u16>().ok()).unwrap_or(3000);
    let address = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(address).await?;
    tracing::info!(%address, "IT Desk prototype is listening");
    axum::serve(listener, app).await?;
    Ok(())
}
