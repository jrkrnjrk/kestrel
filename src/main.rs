mod db;
mod roblox;
mod scripts;

use askama::Template;
use axum::extract::{Form, Path, Request};
use axum::middleware::{self, Next};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use rusqlite::params;
use serde::Deserialize;
use serde_json::{json, Value};
use std::net::SocketAddr;
use tower_http::services::ServeDir;

#[derive(Template)]
#[template(path = "overview.html")]
struct OverviewPage {
    title: String, page: String, viewer: String, group_name: String, authed: bool, demo: bool, group_options: String,
    online: i64, tracked: usize, active: String, idle: String, typing: String, messages: i64, tickets: i64, apps: i64, offs: i64,
    bars: Vec<i64>, people: Vec<db::Person>, logs: Vec<Line>, shout: String,
}
#[derive(Template)]
#[template(path = "groups.html")]
struct GroupsPage {
    title: String, page: String, viewer: String, group_name: String, authed: bool, demo: bool, group_options: String,
    groups: Vec<db::Group>, error: String,
}
#[derive(Template)]
#[template(path = "people.html")]
struct PeoplePage {
    title: String, page: String, viewer: String, group_name: String, authed: bool, demo: bool, group_options: String,
    people: Vec<db::Person>,
}
#[derive(Template)]
#[template(path = "person.html")]
struct PersonPage {
    title: String, page: String, viewer: String, group_name: String, authed: bool, demo: bool, group_options: String,
    person: db::Person, roles: String, notes: Vec<Line>, error: String,
}
#[derive(Template)]
#[template(path = "activity.html")]
struct ActivityPage {
    title: String, page: String, viewer: String, group_name: String, authed: bool, demo: bool, group_options: String,
    rows: Vec<Quota>,
}
#[derive(Template)]
#[template(path = "live.html")]
struct LivePage {
    title: String, page: String, viewer: String, group_name: String, authed: bool, demo: bool, group_options: String,
    online: Vec<db::Person>, shout: String, tickets: Vec<Line>,
}
#[derive(Template)]
#[template(path = "chat.html")]
struct ChatPage {
    title: String, page: String, viewer: String, group_name: String, authed: bool, demo: bool, group_options: String,
    chat: Vec<ChatLine>,
}
#[derive(Template)]
#[template(path = "logs.html")]
struct LogsPage {
    title: String, page: String, viewer: String, group_name: String, authed: bool, demo: bool, group_options: String,
    logs: Vec<Line>,
}
#[derive(Template)]
#[template(path = "events.html")]
struct EventsPage {
    title: String, page: String, viewer: String, group_name: String, authed: bool, demo: bool, group_options: String,
    events: Vec<EventLine>,
}
#[derive(Template)]
#[template(path = "sessions.html")]
struct SessionsPage {
    title: String, page: String, viewer: String, group_name: String, authed: bool, demo: bool, group_options: String,
    shifts: Vec<Shift>, claims: Vec<Claim>,
}
#[derive(Template)]
#[template(path = "applications.html")]
struct AppsPage {
    title: String, page: String, viewer: String, group_name: String, authed: bool, demo: bool, group_options: String,
    forms: Vec<FormRow>, apps: Vec<AppRow>,
}
#[derive(Template)]
#[template(path = "punishments.html")]
struct PunishPage {
    title: String, page: String, viewer: String, group_name: String, authed: bool, demo: bool, group_options: String,
    items: Vec<Pun>,
}
#[derive(Template)]
#[template(path = "timeoff.html")]
struct OffPage {
    title: String, page: String, viewer: String, group_name: String, authed: bool, demo: bool, group_options: String,
    items: Vec<Off>,
}
#[derive(Template)]
#[template(path = "knowledge.html")]
struct DocsPage {
    title: String, page: String, viewer: String, group_name: String, authed: bool, demo: bool, group_options: String,
    docs: Vec<Doc>,
}
#[derive(Template)]
#[template(path = "leaderboard.html")]
struct BoardPage {
    title: String, page: String, viewer: String, group_name: String, authed: bool, demo: bool, group_options: String,
    people: Vec<db::Person>,
}
#[derive(Template)]
#[template(path = "ranking.html")]
struct RankPage {
    title: String, page: String, viewer: String, group_name: String, authed: bool, demo: bool, group_options: String,
    keys: Vec<Line>, roles: String, origin: String, error: String,
}
#[derive(Template)]
#[template(path = "webhooks.html")]
struct HooksPage {
    title: String, page: String, viewer: String, group_name: String, authed: bool, demo: bool, group_options: String,
    hooks: Vec<Hook>,
}
#[derive(Template)]
#[template(path = "scripts.html")]
struct ScriptsPage {
    title: String, page: String, viewer: String, group_name: String, authed: bool, demo: bool, group_options: String,
    loader: String, client: String,
}
#[derive(Template)]
#[template(path = "settings.html")]
struct SettingsPage {
    title: String, page: String, viewer: String, group_name: String, authed: bool, demo: bool, group_options: String,
    brand: String, accent: String, idle_seconds: i64, teams: Vec<Line>,
}
#[derive(Template)]
#[template(path = "login.html")]
struct LoginPage {
    title: String, page: String, viewer: String, group_name: String, authed: bool, demo: bool, group_options: String,
    code: String, display_name: String, username: String, profile_url: String, challenge_id: String, error: String,
}
#[derive(Template)]
#[template(path = "apply.html")]
struct ApplyPage {
    name: String, description: String, slug: String, questions: Vec<Question>, notice: String,
}

#[derive(Clone)]
struct Line { kind: String, actor: String, message: String }
#[derive(Clone)]
struct ChatLine { username: String, channel: String, message: String }
#[derive(Clone)]
struct EventLine { username: String, event_type: String, data_json: String }
#[derive(Clone)]
struct Shift { id: String, title: String, slots: i64, notes: String }
#[derive(Clone)]
struct Claim { username: String, role_name: String, shift_id: String }
#[derive(Clone)]
struct FormRow { id: String, slug: String, name: String }
#[derive(Clone)]
struct AppRow { id: String, username: String, status: String, score: i64 }
#[derive(Clone)]
struct Pun { kind: String, username: String, reason: String, actor: String }
#[derive(Clone)]
struct Off { id: String, username: String, status: String, starts_on: String, ends_on: String, reason: String }
#[derive(Clone)]
struct Doc { title: String, department: String, body: String }
#[derive(Clone)]
struct Hook { id: String, name: String, url: String, events: String }
#[derive(Clone)]
struct Question { id: String, prompt: String }
#[derive(Clone)]
struct Quota { username: String, active_min: i64, idle_min: i64, typing_min: i64, active_pct: i64, idle_pct: i64, typing_pct: i64, goal: i64, met: bool }

fn page(title: &str, page: &str, headers: &HeaderMap) -> (String, String, String, bool, bool, String, i64) {
    let conn = db::open();
    let token = cookie(headers, "kestrel");
    let viewer = db::viewer(&conn, &token);
    let groups = db::groups(&conn);
    let wanted = cookie(headers, "kestrel_group").parse::<i64>().unwrap_or(0);
    let gid = groups.iter().find(|g| g.id == wanted).map(|g| g.id).unwrap_or_else(|| groups.first().map(|g| g.id).unwrap_or(0));
    let group = db::group(&conn, gid);
    let options = groups.iter().map(|g| format!("<option value=\"{}\" {}>{}</option>", g.id, if g.id == gid { "selected" } else { "" }, g.name)).collect::<String>();
    (
        title.into(),
        page.into(),
        viewer.as_ref().map(|v| v.display_name.clone()).unwrap_or_else(|| "Not connected".into()),
        viewer.is_some(),
        group.as_ref().map(|g| g.demo).unwrap_or(true),
        options,
        gid,
    )
}

fn cookie(headers: &HeaderMap, name: &str) -> String {
    headers.get(header::COOKIE).and_then(|v| v.to_str().ok()).unwrap_or("")
        .split(';')
        .find_map(|part| {
            let part = part.trim();
            let prefix = format!("{name}=");
            part.strip_prefix(&prefix).map(|s| s.to_string())
        })
        .unwrap_or_default()
}

fn set_cookie(res: &mut Response, name: &str, value: &str) {
    let cookie = format!("{name}={value}; Path=/; HttpOnly; SameSite=Lax");
    res.headers_mut().append(header::SET_COOKIE, cookie.parse().unwrap());
}

fn render(t: impl Template) -> Response {
    Html(t.render().unwrap_or_else(|e| e.to_string())).into_response()
}

fn mins(n: i64) -> String { if n < 60 { format!("{n}m") } else { format!("{}h {}m", n / 60, n % 60) } }

fn roles_html(conn: &rusqlite::Connection, gid: i64) -> String {
    let raw = conn.query_row("SELECT roles_json FROM groups WHERE id=?1", [gid], |r| r.get::<_, String>(0)).unwrap_or_else(|_| "[]".into());
    let roles: Vec<Value> = serde_json::from_str(&raw).unwrap_or_default();
    roles.iter().filter_map(|r| {
        let id = r.get("id")?.as_i64()?;
        let name = r.get("name").and_then(|v| v.as_str()).unwrap_or("role");
        let rank = r.get("rank").and_then(|v| v.as_i64()).unwrap_or(0);
        Some(format!("<option value=\"{id}\">{name} · {rank}</option>"))
    }).collect()
}

async fn overview(headers: HeaderMap) -> Response {
    let (title, page, viewer, authed, demo, group_options, gid) = page("Overview", "overview", &headers);
    let conn = db::open();
    let people = db::people(&conn, gid);
    let online = people.iter().filter(|p| p.status == "online" || p.status == "idle").count() as i64;
    let max = people.iter().map(|p| p.active_min).max().unwrap_or(1).max(1);
    let bars = people.iter().map(|p| (p.active_min * 100 / max).max(4)).collect();
    let logs = lines(&conn, "SELECT kind, actor, message FROM logs WHERE group_id=?1 ORDER BY id DESC LIMIT 8", gid);
    let shout = conn.query_row("SELECT shout FROM settings WHERE group_id=?1", [gid], |r| r.get::<_, String>(0)).unwrap_or_default();
    render(OverviewPage {
        title, page, viewer, group_name: name_of(&conn, gid), authed, demo, group_options, online, tracked: people.len(),
        active: mins(people.iter().map(|p| p.active_min).sum()), idle: mins(people.iter().map(|p| p.idle_min).sum()),
        typing: mins(people.iter().map(|p| p.typing_min).sum()), messages: people.iter().map(|p| p.messages).sum(),
        tickets: count(&conn, "SELECT COUNT(*) FROM tickets WHERE group_id=?1 AND status='open'", gid),
        apps: count(&conn, "SELECT COUNT(*) FROM applications WHERE group_id=?1 AND status='pending'", gid),
        offs: count(&conn, "SELECT COUNT(*) FROM timeoff WHERE group_id=?1 AND status='pending'", gid),
        bars, people, logs, shout,
    })
}

fn name_of(conn: &rusqlite::Connection, gid: i64) -> String {
    db::group(conn, gid).map(|g| g.name).unwrap_or_else(|| "No group".into())
}
fn count(conn: &rusqlite::Connection, sql: &str, gid: i64) -> i64 {
    conn.query_row(sql, [gid], |r| r.get(0)).unwrap_or(0)
}
fn lines(conn: &rusqlite::Connection, sql: &str, gid: i64) -> Vec<Line> {
    let mut stmt = conn.prepare(sql).unwrap();
    stmt.query_map([gid], |r| Ok(Line { kind: r.get(0)?, actor: r.get(1)?, message: r.get(2)? })).unwrap().filter_map(|r| r.ok()).collect()
}

async fn groups(headers: HeaderMap) -> Response {
    let (title, page, viewer, authed, demo, group_options, gid) = page("Groups", "groups", &headers);
    let conn = db::open();
    render(GroupsPage { title, page, viewer, group_name: name_of(&conn, gid), authed, demo, group_options, groups: db::groups(&conn), error: String::new() })
}

#[derive(Deserialize)]
struct GroupForm { group_id: i64 }

async fn add_group(headers: HeaderMap, Form(form): Form<GroupForm>) -> Response {
    if cookie(&headers, "kestrel").is_empty() {
        return Redirect::to("/login").into_response();
    }
    match roblox::group_info(form.group_id).await {
        Ok((name, description, members, owner, roles)) => {
            let conn = db::open();
            conn.execute("INSERT INTO groups (id, name, description, member_count, owner_name, roles_json, open_cloud_key, demo) VALUES (?1,?2,?3,?4,?5,?6,NULL,0) ON CONFLICT(id) DO UPDATE SET name=excluded.name, description=excluded.description, member_count=excluded.member_count, owner_name=excluded.owner_name, roles_json=excluded.roles_json", params![form.group_id, name, description, members, owner, roles]).ok();
            conn.execute("INSERT OR IGNORE INTO settings (group_id, brand, accent, idle_seconds, shout) VALUES (?1,?2,'#e7ff6a',90,'')", params![form.group_id, name]).ok();
            let mut res = Redirect::to("/people").into_response();
            set_cookie(&mut res, "kestrel_group", &form.group_id.to_string());
            res
        }
        Err(error) => {
            let (title, page, viewer, authed, demo, group_options, gid) = page("Groups", "groups", &headers);
            let conn = db::open();
            render(GroupsPage { title, page, viewer, group_name: name_of(&conn, gid), authed, demo, group_options, groups: db::groups(&conn), error })
        }
    }
}

#[derive(Deserialize)]
struct SwitchForm { group_id: i64 }
async fn switch_group(Form(form): Form<SwitchForm>) -> Response {
    let mut res = Redirect::to("/").into_response();
    set_cookie(&mut res, "kestrel_group", &form.group_id.to_string());
    res
}

async fn people(headers: HeaderMap) -> Response {
    let (title, page, viewer, authed, demo, group_options, gid) = page("People", "people", &headers);
    let conn = db::open();
    render(PeoplePage { title, page, viewer, group_name: name_of(&conn, gid), authed, demo, group_options, people: db::people(&conn, gid) })
}

async fn person(headers: HeaderMap, Path(id): Path<i64>) -> Response {
    let (title, page, viewer, authed, demo, group_options, gid) = page("Record", "people", &headers);
    let conn = db::open();
    let person = db::people(&conn, gid).into_iter().find(|p| p.user_id == id);
    let Some(person) = person else { return Redirect::to("/people").into_response(); };
    let mut stmt = conn.prepare("SELECT actor, body FROM notes WHERE group_id=?1 AND user_id=?2").unwrap();
    let notes = stmt.query_map(params![gid, id], |r| Ok(Line { kind: "note".into(), actor: r.get(0)?, message: r.get(1)? })).unwrap().filter_map(|r| r.ok()).collect();
    render(PersonPage { title, page, viewer, group_name: name_of(&conn, gid), authed, demo, group_options, person, roles: roles_html(&conn, gid), notes, error: String::new() })
}

#[derive(Deserialize)]
struct RankForm { role_id: i64 }
async fn person_rank(headers: HeaderMap, Path(id): Path<i64>, Form(form): Form<RankForm>) -> Response {
    rank_user(&headers, id, form.role_id, "").await;
    Redirect::to(&format!("/people/{id}")).into_response()
}
#[derive(Deserialize)]
struct ReasonForm { reason: String, kind: Option<String> }
async fn person_punish(headers: HeaderMap, Path(id): Path<i64>, Form(form): Form<ReasonForm>) -> Response {
    let conn = db::open();
    let gid = page("", "", &headers).6;
    let username = db::people(&conn, gid).into_iter().find(|p| p.user_id == id).map(|p| p.username).unwrap_or_default();
    let actor = db::viewer(&conn, &cookie(&headers, "kestrel")).map(|v| v.username).unwrap_or_else(|| "desk".into());
    let kind = form.kind.unwrap_or_else(|| "warning".into());
    conn.execute("INSERT INTO punishments (id, group_id, username, kind, reason, actor, created_at) VALUES (?1,?2,?3,?4,?5,?6,?7)", params![format!("p{}", db::now()), gid, username, kind, form.reason, actor, db::now()]).ok();
    if kind == "ban" {
        conn.execute("INSERT INTO bans (id, group_id, user_id, reason) VALUES (?1,?2,?3,?4)", params![format!("b{}", db::now()), gid, id, "banned"]).ok();
    }
    db::log(&conn, gid, "punishment", &actor, &format!("{username} punished"));
    Redirect::to("/punishments").into_response()
}
async fn person_exile(headers: HeaderMap, Path(id): Path<i64>) -> Response {
    let conn = db::open();
    let gid = page("", "", &headers).6;
    let key = conn.query_row("SELECT open_cloud_key FROM groups WHERE id=?1", [gid], |r| r.get::<_, Option<String>>(0)).ok().flatten().unwrap_or_default();
    if !key.is_empty() { let _ = roblox::exile(&key, gid, id).await; }
    db::log(&conn, gid, "ranking", "desk", &format!("Exiled {id}"));
    Redirect::to("/people").into_response()
}
#[derive(Deserialize)]
struct NoteForm { body: String }
async fn person_note(headers: HeaderMap, Path(id): Path<i64>, Form(form): Form<NoteForm>) -> Response {
    let conn = db::open();
    let gid = page("", "", &headers).6;
    let actor = db::viewer(&conn, &cookie(&headers, "kestrel")).map(|v| v.username).unwrap_or_else(|| "desk".into());
    conn.execute("INSERT INTO notes (id, group_id, user_id, body, actor) VALUES (?1,?2,?3,?4,?5)", params![format!("n{}", db::now()), gid, id, form.body, actor]).ok();
    Redirect::to(&format!("/people/{id}")).into_response()
}

async fn activity(headers: HeaderMap) -> Response {
    let (title, page, viewer, authed, demo, group_options, gid) = page("Activity", "activity", &headers);
    let conn = db::open();
    let people = db::people(&conn, gid);
    let rows = people.into_iter().map(|p| {
        let goal = conn.query_row("SELECT weekly_minutes FROM goals WHERE group_id=?1 AND role_name=?2", params![gid, p.role_name], |r| r.get::<_, i64>(0)).unwrap_or(180);
        let total = (p.active_min + p.idle_min + p.typing_min).max(1);
        Quota { username: p.username, active_min: p.active_min, idle_min: p.idle_min, typing_min: p.typing_min, active_pct: p.active_min * 100 / total, idle_pct: p.idle_min * 100 / total, typing_pct: p.typing_min * 100 / total, goal, met: p.active_min >= goal }
    }).collect();
    render(ActivityPage { title, page, viewer, group_name: name_of(&conn, gid), authed, demo, group_options, rows })
}
#[derive(Deserialize)]
struct GoalForm { role_name: String, weekly_minutes: i64 }
async fn save_goal(headers: HeaderMap, Form(form): Form<GoalForm>) -> Response {
    let conn = db::open();
    let gid = page("", "", &headers).6;
    conn.execute("INSERT INTO goals (group_id, role_name, weekly_minutes) VALUES (?1,?2,?3) ON CONFLICT(group_id, role_name) DO UPDATE SET weekly_minutes=excluded.weekly_minutes", params![gid, form.role_name, form.weekly_minutes]).ok();
    Redirect::to("/activity").into_response()
}

async fn live(headers: HeaderMap) -> Response {
    let (title, page, viewer, authed, demo, group_options, gid) = page("Live desk", "live", &headers);
    let conn = db::open();
    let online = db::people(&conn, gid).into_iter().filter(|p| p.status == "online" || p.status == "idle").collect();
    let shout = conn.query_row("SELECT shout FROM settings WHERE group_id=?1", [gid], |r| r.get(0)).unwrap_or_default();
    let mut stmt = conn.prepare("SELECT username, status, message FROM tickets WHERE group_id=?1").unwrap();
    let tickets = stmt.query_map([gid], |r| Ok(Line { kind: r.get(1)?, actor: r.get(0)?, message: r.get(2)? })).unwrap().filter_map(|r| r.ok()).collect();
    render(LivePage { title, page, viewer, group_name: name_of(&conn, gid), authed, demo, group_options, online, shout, tickets })
}
#[derive(Deserialize)]
struct CommandForm { kind: String, user_id: i64, message: Option<String>, reason: Option<String> }
async fn command(headers: HeaderMap, Form(form): Form<CommandForm>) -> Response {
    let conn = db::open();
    let gid = page("", "", &headers).6;
    let payload = json!({"message": form.message.unwrap_or_default(), "reason": form.reason.unwrap_or_default()}).to_string();
    conn.execute("INSERT INTO commands (id, group_id, kind, user_id, payload, status) VALUES (?1,?2,?3,?4,?5,'pending')", params![format!("c{}", db::now()), gid, form.kind, form.user_id, payload]).ok();
    if form.kind == "ban" {
        conn.execute("INSERT INTO bans (id, group_id, user_id, reason) VALUES (?1,?2,?3,?4)", params![format!("ban{}", db::now()), gid, form.user_id, "ban"]).ok();
    }
    db::log(&conn, gid, "remote", "desk", &format!("Queued {} for {}", form.kind, form.user_id));
    Redirect::to("/live").into_response()
}
#[derive(Deserialize)]
struct ShoutForm { message: String }
async fn shout(headers: HeaderMap, Form(form): Form<ShoutForm>) -> Response {
    let conn = db::open();
    let gid = page("", "", &headers).6;
    conn.execute("UPDATE settings SET shout=?1 WHERE group_id=?2", params![form.message, gid]).ok();
    db::log(&conn, gid, "shout", "desk", &form.message);
    Redirect::to("/live").into_response()
}

async fn chat(headers: HeaderMap) -> Response {
    let (title, page, viewer, authed, demo, group_options, gid) = page("Chat logs", "chat", &headers);
    let conn = db::open();
    let mut stmt = conn.prepare("SELECT username, channel, message FROM chat WHERE group_id=?1 ORDER BY id DESC LIMIT 200").unwrap();
    let chat = stmt.query_map([gid], |r| Ok(ChatLine { username: r.get(0)?, channel: r.get(1)?, message: r.get(2)? })).unwrap().filter_map(|r| r.ok()).collect();
    render(ChatPage { title, page, viewer, group_name: name_of(&conn, gid), authed, demo, group_options, chat })
}
async fn logs(headers: HeaderMap) -> Response {
    let (title, page, viewer, authed, demo, group_options, gid) = page("All logs", "logs", &headers);
    let conn = db::open();
    render(LogsPage { title, page, viewer, group_name: name_of(&conn, gid), authed, demo, group_options, logs: lines(&conn, "SELECT kind, actor, message FROM logs WHERE group_id=?1 ORDER BY id DESC LIMIT 200", gid) })
}
async fn events(headers: HeaderMap) -> Response {
    let (title, page, viewer, authed, demo, group_options, gid) = page("Events", "events", &headers);
    let conn = db::open();
    let mut stmt = conn.prepare("SELECT username, event_type, data_json FROM events WHERE group_id=?1 ORDER BY id DESC").unwrap();
    let events = stmt.query_map([gid], |r| Ok(EventLine { username: r.get(0)?, event_type: r.get(1)?, data_json: r.get(2)? })).unwrap().filter_map(|r| r.ok()).collect();
    render(EventsPage { title, page, viewer, group_name: name_of(&conn, gid), authed, demo, group_options, events })
}
async fn sessions(headers: HeaderMap) -> Response {
    let (title, page, viewer, authed, demo, group_options, gid) = page("Sessions", "sessions", &headers);
    let conn = db::open();
    let mut stmt = conn.prepare("SELECT id, title, slots, notes FROM shifts WHERE group_id=?1").unwrap();
    let shifts = stmt.query_map([gid], |r| Ok(Shift { id: r.get(0)?, title: r.get(1)?, slots: r.get(2)?, notes: r.get(3)? })).unwrap().filter_map(|r| r.ok()).collect();
    let mut stmt = conn.prepare("SELECT username, role_name, shift_id FROM claims WHERE group_id=?1").unwrap();
    let claims = stmt.query_map([gid], |r| Ok(Claim { username: r.get(0)?, role_name: r.get(1)?, shift_id: r.get(2)? })).unwrap().filter_map(|r| r.ok()).collect();
    render(SessionsPage { title, page, viewer, group_name: name_of(&conn, gid), authed, demo, group_options, shifts, claims })
}
#[derive(Deserialize)]
struct ShiftForm { title: String, place_id: Option<i64>, slots: Option<i64> }
async fn add_shift(headers: HeaderMap, Form(form): Form<ShiftForm>) -> Response {
    let conn = db::open();
    let gid = page("", "", &headers).6;
    conn.execute("INSERT INTO shifts (id, group_id, title, place_id, slots, notes, created_at) VALUES (?1,?2,?3,?4,?5,'',?6)", params![format!("s{}", db::now()), gid, form.title, form.place_id.unwrap_or(0), form.slots.unwrap_or(6), db::now()]).ok();
    Redirect::to("/sessions").into_response()
}
#[derive(Deserialize)]
struct ClaimForm { shift_id: String }
async fn claim(headers: HeaderMap, Form(form): Form<ClaimForm>) -> Response {
    let conn = db::open();
    let gid = page("", "", &headers).6;
    let username = db::viewer(&conn, &cookie(&headers, "kestrel")).map(|v| v.username).unwrap_or_else(|| "desk".into());
    conn.execute("INSERT INTO claims (id, shift_id, group_id, username, role_name, created_at) VALUES (?1,?2,?3,?4,'Staff',?5)", params![format!("cl{}", db::now()), form.shift_id, gid, username, db::now()]).ok();
    Redirect::to("/sessions").into_response()
}

async fn applications(headers: HeaderMap) -> Response {
    let (title, page, viewer, authed, demo, group_options, gid) = page("Applications", "applications", &headers);
    let conn = db::open();
    let mut stmt = conn.prepare("SELECT id, slug, name FROM forms WHERE group_id=?1").unwrap();
    let forms = stmt.query_map([gid], |r| Ok(FormRow { id: r.get(0)?, slug: r.get(1)?, name: r.get(2)? })).unwrap().filter_map(|r| r.ok()).collect();
    let mut stmt = conn.prepare("SELECT id, username, status, score FROM applications WHERE group_id=?1 ORDER BY created_at DESC").unwrap();
    let apps = stmt.query_map([gid], |r| Ok(AppRow { id: r.get(0)?, username: r.get(1)?, status: r.get(2)?, score: r.get(3)? })).unwrap().filter_map(|r| r.ok()).collect();
    render(AppsPage { title, page, viewer, group_name: name_of(&conn, gid), authed, demo, group_options, forms, apps })
}
#[derive(Deserialize)]
struct NewForm { name: String, slug: String, description: String, quiz: Option<String>, pass_percent: Option<i64> }
async fn add_form(headers: HeaderMap, Form(form): Form<NewForm>) -> Response {
    let conn = db::open();
    let gid = page("", "", &headers).6;
    conn.execute("INSERT INTO forms (id, group_id, slug, name, description, quiz, pass_percent) VALUES (?1,?2,?3,?4,?5,?6,?7)", params![format!("f{}", db::now()), gid, form.slug, form.name, form.description, if form.quiz.is_some() { 1 } else { 0 }, form.pass_percent.unwrap_or(70)]).ok();
    Redirect::to("/applications").into_response()
}
#[derive(Deserialize)]
struct QForm { form_id: String, prompt: String, answer: String }
async fn add_question(headers: HeaderMap, Form(form): Form<QForm>) -> Response {
    let conn = db::open();
    conn.execute("INSERT INTO questions (id, form_id, prompt, answer) VALUES (?1,?2,?3,?4)", params![format!("q{}", db::now()), form.form_id, form.prompt, form.answer]).ok();
    Redirect::to("/applications").into_response()
}
#[derive(Deserialize)]
struct ReviewForm { id: String, status: String }
async fn review_app(headers: HeaderMap, Form(form): Form<ReviewForm>) -> Response {
    let conn = db::open();
    let gid = page("", "", &headers).6;
    conn.execute("UPDATE applications SET status=?1 WHERE id=?2", params![form.status, form.id]).ok();
    db::log(&conn, gid, "application", "desk", &format!("{} {}", form.status, form.id));
    Redirect::to("/applications").into_response()
}

async fn punishments(headers: HeaderMap) -> Response {
    let (title, page, viewer, authed, demo, group_options, gid) = page("Punishments", "punishments", &headers);
    let conn = db::open();
    let mut stmt = conn.prepare("SELECT kind, username, reason, actor FROM punishments WHERE group_id=?1 ORDER BY created_at DESC").unwrap();
    let items = stmt.query_map([gid], |r| Ok(Pun { kind: r.get(0)?, username: r.get(1)?, reason: r.get(2)?, actor: r.get(3)? })).unwrap().filter_map(|r| r.ok()).collect();
    render(PunishPage { title, page, viewer, group_name: name_of(&conn, gid), authed, demo, group_options, items })
}
async fn timeoff(headers: HeaderMap) -> Response {
    let (title, page, viewer, authed, demo, group_options, gid) = page("Time off", "timeoff", &headers);
    let conn = db::open();
    let mut stmt = conn.prepare("SELECT id, username, status, starts_on, ends_on, reason FROM timeoff WHERE group_id=?1").unwrap();
    let items = stmt.query_map([gid], |r| Ok(Off { id: r.get(0)?, username: r.get(1)?, status: r.get(2)?, starts_on: r.get(3)?, ends_on: r.get(4)?, reason: r.get(5)? })).unwrap().filter_map(|r| r.ok()).collect();
    render(OffPage { title, page, viewer, group_name: name_of(&conn, gid), authed, demo, group_options, items })
}
#[derive(Deserialize)]
struct OffForm { username: String, starts_on: String, ends_on: String, reason: String }
async fn add_off(headers: HeaderMap, Form(form): Form<OffForm>) -> Response {
    let conn = db::open();
    let gid = page("", "", &headers).6;
    conn.execute("INSERT INTO timeoff (id, group_id, username, starts_on, ends_on, reason, status, created_at) VALUES (?1,?2,?3,?4,?5,?6,'pending',?7)", params![format!("o{}", db::now()), gid, form.username, form.starts_on, form.ends_on, form.reason, db::now()]).ok();
    Redirect::to("/timeoff").into_response()
}
async fn review_off(Form(form): Form<ReviewForm>) -> Response {
    let conn = db::open();
    conn.execute("UPDATE timeoff SET status=?1 WHERE id=?2", params![form.status, form.id]).ok();
    Redirect::to("/timeoff").into_response()
}
async fn knowledge(headers: HeaderMap) -> Response {
    let (title, page, viewer, authed, demo, group_options, gid) = page("Knowledge", "knowledge", &headers);
    let conn = db::open();
    let mut stmt = conn.prepare("SELECT title, department, body FROM docs WHERE group_id=?1").unwrap();
    let docs = stmt.query_map([gid], |r| Ok(Doc { title: r.get(0)?, department: r.get(1)?, body: r.get(2)? })).unwrap().filter_map(|r| r.ok()).collect();
    render(DocsPage { title, page, viewer, group_name: name_of(&conn, gid), authed, demo, group_options, docs })
}
#[derive(Deserialize)]
struct DocForm { title: String, department: String, body: String }
async fn add_doc(headers: HeaderMap, Form(form): Form<DocForm>) -> Response {
    let conn = db::open();
    let gid = page("", "", &headers).6;
    conn.execute("INSERT INTO docs (id, group_id, title, body, department) VALUES (?1,?2,?3,?4,?5)", params![format!("d{}", db::now()), gid, form.title, form.body, form.department]).ok();
    Redirect::to("/knowledge").into_response()
}
async fn leaderboard(headers: HeaderMap) -> Response {
    let (title, page, viewer, authed, demo, group_options, gid) = page("Leaderboard", "leaderboard", &headers);
    let conn = db::open();
    let mut people = db::people(&conn, gid);
    people.truncate(50);
    render(BoardPage { title, page, viewer, group_name: name_of(&conn, gid), authed, demo, group_options, people })
}
async fn ranking(headers: HeaderMap) -> Response {
    let (title, page, viewer, authed, demo, group_options, gid) = page("Ranking API", "ranking", &headers);
    let conn = db::open();
    let mut stmt = conn.prepare("SELECT name, token, token FROM keys WHERE group_id=?1").unwrap();
    let keys = stmt.query_map([gid], |r| Ok(Line { kind: r.get(0)?, actor: r.get(1)?, message: r.get(1)? })).unwrap().filter_map(|r| r.ok()).collect();
    render(RankPage { title, page, viewer, group_name: name_of(&conn, gid), authed, demo, group_options, keys, roles: roles_html(&conn, gid), origin: origin(&headers), error: String::new() })
}
#[derive(Deserialize)]
struct CloudForm { open_cloud_key: String }
async fn save_cloud(headers: HeaderMap, Form(form): Form<CloudForm>) -> Response {
    let conn = db::open();
    let gid = page("", "", &headers).6;
    conn.execute("UPDATE groups SET open_cloud_key=?1 WHERE id=?2", params![form.open_cloud_key, gid]).ok();
    Redirect::to("/ranking").into_response()
}
#[derive(Deserialize)]
struct KeyForm { name: String }
async fn issue_key(headers: HeaderMap, Form(form): Form<KeyForm>) -> Response {
    let conn = db::open();
    let gid = page("", "", &headers).6;
    let token = format!("kst_{}", db::now());
    conn.execute("INSERT INTO keys (id, group_id, name, token) VALUES (?1,?2,?3,?4)", params![format!("k{}", db::now()), gid, form.name, token]).ok();
    Redirect::to("/ranking").into_response()
}
#[derive(Deserialize)]
struct ApplyRank { user_id: i64, role_id: i64 }
async fn apply_rank(headers: HeaderMap, Form(form): Form<ApplyRank>) -> Response {
    rank_user(&headers, form.user_id, form.role_id, "").await;
    Redirect::to("/ranking").into_response()
}
async fn rank_user(headers: &HeaderMap, user_id: i64, role_id: i64, username: &str) {
    let conn = db::open();
    let gid = page("", "", headers).6;
    let key = conn.query_row("SELECT open_cloud_key FROM groups WHERE id=?1", [gid], |r| r.get::<_, Option<String>>(0)).ok().flatten().unwrap_or_default();
    if !key.is_empty() { let _ = roblox::rank(&key, gid, user_id, role_id).await; }
    db::log(&conn, gid, "ranking", "desk", &format!("Ranked {} / {username} to role {role_id}", user_id));
}
async fn webhooks(headers: HeaderMap) -> Response {
    let (title, page, viewer, authed, demo, group_options, gid) = page("Webhooks", "webhooks", &headers);
    let conn = db::open();
    let mut stmt = conn.prepare("SELECT id, name, url, events FROM webhooks WHERE group_id=?1").unwrap();
    let hooks = stmt.query_map([gid], |r| Ok(Hook { id: r.get(0)?, name: r.get(1)?, url: r.get(2)?, events: r.get(3)? })).unwrap().filter_map(|r| r.ok()).collect();
    render(HooksPage { title, page, viewer, group_name: name_of(&conn, gid), authed, demo, group_options, hooks })
}
#[derive(Deserialize)]
struct HookForm { name: String, url: String, events: String }
async fn add_hook(headers: HeaderMap, Form(form): Form<HookForm>) -> Response {
    let conn = db::open();
    let gid = page("", "", &headers).6;
    conn.execute("INSERT INTO webhooks (id, group_id, name, url, events) VALUES (?1,?2,?3,?4,?5)", params![format!("w{}", db::now()), gid, form.name, form.url, form.events]).ok();
    Redirect::to("/webhooks").into_response()
}
#[derive(Deserialize)]
struct IdForm { id: String }
async fn test_hook(Form(form): Form<IdForm>) -> Response {
    let conn = db::open();
    if let Ok(url) = conn.query_row("SELECT url FROM webhooks WHERE id=?1", [&form.id], |r| r.get::<_, String>(0)) {
        let _ = reqwest::Client::new().post(url).json(&json!({"username":"Kestrel","content":"Kestrel test ping"})).send().await;
    }
    Redirect::to("/webhooks").into_response()
}
async fn scripts(headers: HeaderMap) -> Response {
    let (title, page, viewer, authed, demo, group_options, gid) = page("Scripts", "scripts", &headers);
    let conn = db::open();
    let key = conn.query_row("SELECT token FROM keys WHERE group_id=?1 ORDER BY rowid DESC LIMIT 1", [gid], |r| r.get::<_, String>(0)).unwrap_or_else(|_| "PASTE_KEY".into());
    let idle = conn.query_row("SELECT idle_seconds FROM settings WHERE group_id=?1", [gid], |r| r.get(0)).unwrap_or(90);
    render(ScriptsPage { title, page, viewer, group_name: name_of(&conn, gid), authed, demo, group_options, loader: scripts::loader(&origin(&headers), &key, gid, idle), client: scripts::client().into() })
}
async fn loader_file(headers: HeaderMap) -> Response {
    let gid = page("", "", &headers).6;
    let conn = db::open();
    let key = conn.query_row("SELECT token FROM keys WHERE group_id=?1 ORDER BY rowid DESC LIMIT 1", [gid], |r| r.get::<_, String>(0)).unwrap_or_else(|_| "PASTE_KEY".into());
    let idle = conn.query_row("SELECT idle_seconds FROM settings WHERE group_id=?1", [gid], |r| r.get(0)).unwrap_or(90);
    ([(header::CONTENT_TYPE, "text/plain"), (header::CONTENT_DISPOSITION, "attachment; filename=KestrelLoader.server.lua")], scripts::loader(&origin(&headers), &key, gid, idle)).into_response()
}
async fn client_file() -> Response {
    ([(header::CONTENT_TYPE, "text/plain"), (header::CONTENT_DISPOSITION, "attachment; filename=KestrelClient.client.lua")], scripts::client()).into_response()
}
async fn settings(headers: HeaderMap) -> Response {
    let (title, page, viewer, authed, demo, group_options, gid) = page("Settings", "settings", &headers);
    let conn = db::open();
    let (brand, accent, idle_seconds) = conn.query_row("SELECT brand, accent, idle_seconds FROM settings WHERE group_id=?1", [gid], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap_or_else(|_| ("Kestrel".into(), "#e7ff6a".into(), 90));
    let mut stmt = conn.prepare("SELECT name, rank_min, name FROM teams WHERE group_id=?1").unwrap();
    let teams = stmt.query_map([gid], |r| Ok(Line { kind: r.get::<_, i64>(1)?.to_string(), actor: r.get(0)?, message: r.get(0)? })).unwrap().filter_map(|r| r.ok()).collect();
    render(SettingsPage { title, page, viewer, group_name: name_of(&conn, gid), authed, demo, group_options, brand, accent, idle_seconds, teams })
}
#[derive(Deserialize)]
struct SettingsForm { brand: String, accent: String, idle_seconds: i64 }
async fn save_settings(headers: HeaderMap, Form(form): Form<SettingsForm>) -> Response {
    let conn = db::open();
    let gid = page("", "", &headers).6;
    conn.execute("UPDATE settings SET brand=?1, accent=?2, idle_seconds=?3 WHERE group_id=?4", params![form.brand, form.accent, form.idle_seconds, gid]).ok();
    Redirect::to("/settings").into_response()
}
#[derive(Deserialize)]
struct TeamForm { name: String, rank_min: i64 }
async fn add_team(headers: HeaderMap, Form(form): Form<TeamForm>) -> Response {
    let conn = db::open();
    let gid = page("", "", &headers).6;
    conn.execute("INSERT INTO teams (id, group_id, name, rank_min) VALUES (?1,?2,?3,?4)", params![format!("t{}", db::now()), gid, form.name, form.rank_min]).ok();
    Redirect::to("/settings").into_response()
}

async fn login(headers: HeaderMap) -> Response {
    let (title, page, viewer, authed, demo, group_options, gid) = page("Connect", "login", &headers);
    let conn = db::open();
    render(LoginPage { title, page, viewer, group_name: name_of(&conn, gid), authed, demo, group_options, code: String::new(), display_name: String::new(), username: String::new(), profile_url: String::new(), challenge_id: String::new(), error: String::new() })
}
#[derive(Deserialize)]
struct UserForm { username: String }
async fn login_start(headers: HeaderMap, Form(form): Form<UserForm>) -> Response {
    let (title, page, viewer, authed, demo, group_options, gid) = page("Connect", "login", &headers);
    let conn = db::open();
    let group_name = name_of(&conn, gid);
    match roblox::lookup(form.username.trim().trim_start_matches('@')).await {
        Some((id, username, display_name)) => {
            let words = ["work", "love", "hello"];
            let code = format!("kestrel {}", words[(db::now() as usize) % 3]);
            let challenge_id = format!("ch{}", db::now());
            conn.execute("INSERT INTO challenges (id, user_id, username, display_name, code, expires_at) VALUES (?1,?2,?3,?4,?5,?6)", params![challenge_id, id, username, display_name, code, db::now() + 900]).ok();
            render(LoginPage { title, page, viewer, group_name, authed, demo, group_options, code, display_name, username, profile_url: format!("https://www.roblox.com/users/{id}/profile"), challenge_id, error: String::new() })
        }
        None => render(LoginPage { title, page, viewer, group_name, authed, demo, group_options, code: String::new(), display_name: String::new(), username: String::new(), profile_url: String::new(), challenge_id: String::new(), error: "Roblox user not found".into() }),
    }
}
#[derive(Deserialize)]
struct VerifyForm { challenge_id: String }
async fn login_verify(headers: HeaderMap, Form(form): Form<VerifyForm>) -> Response {
    let conn = db::open();
    let row = conn.query_row("SELECT user_id, username, display_name, code, expires_at FROM challenges WHERE id=?1", [&form.challenge_id], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?, r.get::<_, i64>(4)?)));
    let Ok((user_id, username, display_name, code, exp)) = row else { return Redirect::to("/login").into_response(); };
    if exp < db::now() { return Redirect::to("/login").into_response(); }
    match roblox::bio_has(user_id, &code).await {
        Ok(true) => {
            let token = format!("sess{}", db::now());
            conn.execute("INSERT INTO auth (token, user_id, username, display_name, created_at) VALUES (?1,?2,?3,?4,?5)", params![token, user_id, username, display_name, db::now()]).ok();
            let mut res = Redirect::to("/").into_response();
            set_cookie(&mut res, "kestrel", &token);
            let _ = headers;
            res
        }
        _ => {
            let (title, page, viewer, authed, demo, group_options, gid) = page("Connect", "login", &headers);
            let conn = db::open();
            render(LoginPage { title, page, viewer, group_name: name_of(&conn, gid), authed, demo, group_options, code, display_name, username: username.clone(), profile_url: format!("https://www.roblox.com/users/{user_id}/profile"), challenge_id: form.challenge_id, error: "Code is not in the About box yet.".into() })
        }
    }
}
async fn logout() -> Response {
    let mut res = Redirect::to("/").into_response();
    set_cookie(&mut res, "kestrel", "gone");
    res
}

async fn apply_page(Path(slug): Path<String>) -> Response {
    let conn = db::open();
    let form = conn.query_row("SELECT name, description FROM forms WHERE slug=?1", [&slug], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)));
    let Ok((name, description)) = form else { return (StatusCode::NOT_FOUND, "not found").into_response(); };
    let fid = conn.query_row("SELECT id FROM forms WHERE slug=?1", [&slug], |r| r.get::<_, String>(0)).unwrap_or_default();
    let mut stmt = conn.prepare("SELECT id, prompt FROM questions WHERE form_id=?1").unwrap();
    let questions = stmt.query_map([&fid], |r| Ok(Question { id: r.get(0)?, prompt: r.get(1)? })).unwrap().filter_map(|r| r.ok()).collect();
    render(ApplyPage { name, description, slug, questions, notice: String::new() })
}

fn origin(headers: &HeaderMap) -> String {
    let host = headers.get("x-forwarded-host").or_else(|| headers.get(header::HOST)).and_then(|v| v.to_str().ok()).unwrap_or("localhost");
    let proto = headers.get("x-forwarded-proto").and_then(|v| v.to_str().ok()).unwrap_or("http");
    format!("{proto}://{host}")
}

fn bearer(headers: &HeaderMap) -> String {
    headers.get(header::AUTHORIZATION).and_then(|v| v.to_str().ok()).unwrap_or("").trim_start_matches("Bearer ").trim().to_string()
}
fn key_group(token: &str) -> Option<i64> {
    if token.is_empty() { return None; }
    let conn = db::open();
    conn.query_row("SELECT group_id FROM keys WHERE token=?1", [token], |r| r.get(0)).ok()
}
async fn ingest_presence(headers: HeaderMap, Json(body): Json<Value>) -> Response {
    let Some(gid) = key_group(&bearer(&headers)) else { return (StatusCode::UNAUTHORIZED, "api key required").into_response(); };
    let conn = db::open();
    let players = body.get("players").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    for p in players {
        let uid = p.get("userId").and_then(|v| v.as_i64()).unwrap_or(0);
        if uid == 0 { continue; }
        conn.execute("INSERT INTO people (group_id, user_id, username, display_name, role_name, role_rank, status, active_min, idle_min, typing_min, messages, sessions, last_seen, department, birthday, server_id) VALUES (?1,?2,?3,?4,'',0,?5,?6,?7,?8,?9,?10,datetime('now'),'','',?11) ON CONFLICT(group_id, user_id) DO UPDATE SET username=excluded.username, status=excluded.status, active_min=people.active_min+excluded.active_min, idle_min=people.idle_min+excluded.idle_min, typing_min=people.typing_min+excluded.typing_min, messages=people.messages+excluded.messages, server_id=excluded.server_id, last_seen=excluded.last_seen", params![gid, uid, p.get("username").and_then(|v| v.as_str()).unwrap_or(""), p.get("displayName").and_then(|v| v.as_str()).unwrap_or(""), p.get("status").and_then(|v| v.as_str()).unwrap_or("online"), p.get("activeMin").and_then(|v| v.as_i64()).unwrap_or(0), p.get("idleMin").and_then(|v| v.as_i64()).unwrap_or(0), p.get("typingMin").and_then(|v| v.as_i64()).unwrap_or(0), p.get("messages").and_then(|v| v.as_i64()).unwrap_or(0), p.get("sessions").and_then(|v| v.as_i64()).unwrap_or(0), p.get("serverId").and_then(|v| v.as_str()).unwrap_or("")]).ok();
    }
    Json(json!({"ok": true})).into_response()
}
async fn ingest_chat(headers: HeaderMap, Json(body): Json<Value>) -> Response {
    let Some(gid) = key_group(&bearer(&headers)) else { return (StatusCode::UNAUTHORIZED, "api key required").into_response(); };
    let conn = db::open();
    conn.execute("INSERT INTO chat (group_id, user_id, username, message, channel, created_at) VALUES (?1,?2,?3,?4,?5,?6)", params![gid, body.get("userId").and_then(|v| v.as_i64()).unwrap_or(0), body.get("username").and_then(|v| v.as_str()).unwrap_or(""), body.get("message").and_then(|v| v.as_str()).unwrap_or(""), body.get("channel").and_then(|v| v.as_str()).unwrap_or("all"), db::now()]).ok();
    Json(json!({"ok": true})).into_response()
}
async fn ingest_event(headers: HeaderMap, Json(body): Json<Value>) -> Response {
    let Some(gid) = key_group(&bearer(&headers)) else { return (StatusCode::UNAUTHORIZED, "api key required").into_response(); };
    let conn = db::open();
    conn.execute("INSERT INTO events (group_id, username, event_type, data_json, created_at) VALUES (?1,?2,?3,?4,?5)", params![gid, body.get("username").and_then(|v| v.as_str()).unwrap_or(""), body.get("eventType").and_then(|v| v.as_str()).unwrap_or("event"), body.get("data").map(|v| v.to_string()).unwrap_or_else(|| "{}".into()), db::now()]).ok();
    Json(json!({"ok": true})).into_response()
}
async fn ingest_commands(headers: HeaderMap) -> Response {
    let Some(gid) = key_group(&bearer(&headers)) else { return (StatusCode::UNAUTHORIZED, "api key required").into_response(); };
    let conn = db::open();
    let mut stmt = conn.prepare("SELECT id, kind, user_id, payload FROM commands WHERE group_id=?1 AND status='pending'").unwrap();
    let rows: Vec<(String, String, i64, String)> = stmt.query_map([gid], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))).unwrap().filter_map(|r| r.ok()).collect();
    for (id, _, _, _) in &rows { conn.execute("UPDATE commands SET status='sent' WHERE id=?1", [id]).ok(); }
    let mut stmt = conn.prepare("SELECT user_id, reason FROM bans WHERE group_id=?1").unwrap();
    let bans: Vec<(i64, String)> = stmt.query_map([gid], |r| Ok((r.get(0)?, r.get(1)?))).unwrap().filter_map(|r| r.ok()).collect();
    Json(json!({
        "commands": rows.iter().map(|(id, kind, uid, payload)| json!({"id": id, "kind": kind, "userId": uid, "payload": serde_json::from_str::<Value>(payload).unwrap_or(json!({}))})).collect::<Vec<_>>(),
        "bans": bans.iter().map(|(uid, reason)| json!({"userId": uid, "reason": reason})).collect::<Vec<_>>()
    })).into_response()
}
async fn api_rank(headers: HeaderMap, Json(body): Json<Value>) -> Response {
    let Some(gid) = key_group(&bearer(&headers)) else { return (StatusCode::UNAUTHORIZED, "api key required").into_response(); };
    let uid = body.get("userId").and_then(|v| v.as_i64()).unwrap_or(0);
    let role = body.get("roleId").and_then(|v| v.as_i64()).unwrap_or(0);
    let conn = db::open();
    let key = conn.query_row("SELECT open_cloud_key FROM groups WHERE id=?1", [gid], |r| r.get::<_, Option<String>>(0)).ok().flatten().unwrap_or_default();
    if !key.is_empty() {
        if let Err(err) = roblox::rank(&key, gid, uid, role).await {
            return (StatusCode::BAD_GATEWAY, err).into_response();
        }
    }
    db::log(&conn, gid, "ranking", "api", &format!("Ranked {uid} to {role}"));
    Json(json!({"ok": true, "appliedOnRoblox": !key.is_empty()})).into_response()
}
async fn api_exile(headers: HeaderMap, Json(body): Json<Value>) -> Response {
    let Some(gid) = key_group(&bearer(&headers)) else { return (StatusCode::UNAUTHORIZED, "api key required").into_response(); };
    let uid = body.get("userId").and_then(|v| v.as_i64()).unwrap_or(0);
    let conn = db::open();
    let key = conn.query_row("SELECT open_cloud_key FROM groups WHERE id=?1", [gid], |r| r.get::<_, Option<String>>(0)).ok().flatten().unwrap_or_default();
    if !key.is_empty() {
        if let Err(err) = roblox::exile(&key, gid, uid).await {
            return (StatusCode::BAD_GATEWAY, err).into_response();
        }
    }
    db::log(&conn, gid, "ranking", "api", &format!("Exiled {uid}"));
    Json(json!({"ok": true})).into_response()
}

async fn require_login(request: Request, next: Next) -> Response {
    let path = request.uri().path().to_string();
    let open = path == "/login"
        || path.starts_with("/login/")
        || path == "/health"
        || path == "/logout"
        || path.starts_with("/apply/")
        || path.starts_with("/api/")
        || path.starts_with("/static");
    if !open {
        let headers = request.headers();
        let conn = db::open();
        if db::viewer(&conn, &cookie(headers, "kestrel")).is_none() {
            return Redirect::to("/login").into_response();
        }
    }
    next.run(request).await
}

#[tokio::main]
async fn main() {
    let _ = db::open();
    let static_dir = std::env::var("STATIC_DIR").unwrap_or_else(|_| "static".into());
    let app = Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/", get(overview))
        .route("/groups", get(groups).post(add_group))
        .route("/group/switch", post(switch_group))
        .route("/people", get(people))
        .route("/people/:id", get(person))
        .route("/people/:id/rank", post(person_rank))
        .route("/people/:id/punish", post(person_punish))
        .route("/people/:id/exile", post(person_exile))
        .route("/people/:id/note", post(person_note))
        .route("/activity", get(activity))
        .route("/activity/goal", post(save_goal))
        .route("/live", get(live))
        .route("/live/command", post(command))
        .route("/live/shout", post(shout))
        .route("/chat", get(chat))
        .route("/logs", get(logs))
        .route("/events", get(events))
        .route("/sessions", get(sessions).post(add_shift))
        .route("/sessions/claim", post(claim))
        .route("/applications", get(applications).post(add_form))
        .route("/applications/question", post(add_question))
        .route("/applications/review", post(review_app))
        .route("/punishments", get(punishments))
        .route("/timeoff", get(timeoff).post(add_off))
        .route("/timeoff/review", post(review_off))
        .route("/knowledge", get(knowledge).post(add_doc))
        .route("/leaderboard", get(leaderboard))
        .route("/ranking", get(ranking))
        .route("/ranking/cloud", post(save_cloud))
        .route("/ranking/key", post(issue_key))
        .route("/ranking/apply", post(apply_rank))
        .route("/webhooks", get(webhooks).post(add_hook))
        .route("/webhooks/test", post(test_hook))
        .route("/scripts", get(scripts))
        .route("/scripts/loader.lua", get(loader_file))
        .route("/scripts/client.lua", get(client_file))
        .route("/settings", get(settings).post(save_settings))
        .route("/settings/team", post(add_team))
        .route("/login", get(login))
        .route("/login/start", post(login_start))
        .route("/login/verify", post(login_verify))
        .route("/logout", get(logout))
        .route("/apply/:slug", get(apply_page))
        .route("/api/ingest/presence", post(ingest_presence))
        .route("/api/ingest/chat", post(ingest_chat))
        .route("/api/ingest/event", post(ingest_event))
        .route("/api/ingest/commands", get(ingest_commands))
        .route("/api/v1/rank", post(api_rank))
        .route("/api/v1/exile", post(api_exile))
        .nest_service("/static", ServeDir::new(static_dir))
        .layer(middleware::from_fn(require_login));
    let port: u16 = std::env::var("PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(8080);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    println!("Kestrel listening on {addr}");
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
