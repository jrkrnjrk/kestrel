use rusqlite::{params, Connection, OptionalExtension};
use std::path::PathBuf;

pub fn path() -> PathBuf {
    if let Ok(dir) = std::env::var("DATA_DIR") {
        return PathBuf::from(dir).join("kestrel.db");
    }
    if PathBuf::from("/data").is_dir() {
        return PathBuf::from("/data/kestrel.db");
    }
    PathBuf::from("data/kestrel.db")
}

pub fn open() -> Connection {
    let p = path();
    if let Some(parent) = p.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let conn = Connection::open(p).expect("open db");
    conn.execute_batch(SCHEMA).expect("schema");
    seed(&conn);
    conn
}

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS challenges (id TEXT PRIMARY KEY, user_id INTEGER, username TEXT, display_name TEXT, code TEXT, expires_at INTEGER);
CREATE TABLE IF NOT EXISTS auth (token TEXT PRIMARY KEY, user_id INTEGER, username TEXT, display_name TEXT, created_at INTEGER);
CREATE TABLE IF NOT EXISTS groups (id INTEGER PRIMARY KEY, name TEXT, description TEXT, member_count INTEGER, owner_name TEXT, roles_json TEXT, open_cloud_key TEXT, demo INTEGER DEFAULT 0);
CREATE TABLE IF NOT EXISTS people (group_id INTEGER, user_id INTEGER, username TEXT, display_name TEXT, role_name TEXT, role_rank INTEGER, status TEXT, active_min INTEGER, idle_min INTEGER, typing_min INTEGER, messages INTEGER, sessions INTEGER, last_seen TEXT, department TEXT, birthday TEXT, server_id TEXT, PRIMARY KEY (group_id, user_id));
CREATE TABLE IF NOT EXISTS chat (id INTEGER PRIMARY KEY AUTOINCREMENT, group_id INTEGER, user_id INTEGER, username TEXT, message TEXT, channel TEXT, created_at INTEGER);
CREATE TABLE IF NOT EXISTS logs (id INTEGER PRIMARY KEY AUTOINCREMENT, group_id INTEGER, kind TEXT, actor TEXT, message TEXT, created_at INTEGER);
CREATE TABLE IF NOT EXISTS events (id INTEGER PRIMARY KEY AUTOINCREMENT, group_id INTEGER, username TEXT, event_type TEXT, data_json TEXT, created_at INTEGER);
CREATE TABLE IF NOT EXISTS shifts (id TEXT PRIMARY KEY, group_id INTEGER, title TEXT, place_id INTEGER, slots INTEGER, notes TEXT, created_at INTEGER);
CREATE TABLE IF NOT EXISTS claims (id TEXT PRIMARY KEY, shift_id TEXT, group_id INTEGER, username TEXT, role_name TEXT, created_at INTEGER);
CREATE TABLE IF NOT EXISTS forms (id TEXT PRIMARY KEY, group_id INTEGER, slug TEXT UNIQUE, name TEXT, description TEXT, quiz INTEGER, pass_percent INTEGER);
CREATE TABLE IF NOT EXISTS questions (id TEXT PRIMARY KEY, form_id TEXT, prompt TEXT, answer TEXT);
CREATE TABLE IF NOT EXISTS applications (id TEXT PRIMARY KEY, form_id TEXT, group_id INTEGER, username TEXT, answers TEXT, score INTEGER, status TEXT, created_at INTEGER);
CREATE TABLE IF NOT EXISTS punishments (id TEXT PRIMARY KEY, group_id INTEGER, username TEXT, kind TEXT, reason TEXT, actor TEXT, created_at INTEGER);
CREATE TABLE IF NOT EXISTS timeoff (id TEXT PRIMARY KEY, group_id INTEGER, username TEXT, starts_on TEXT, ends_on TEXT, reason TEXT, status TEXT, created_at INTEGER);
CREATE TABLE IF NOT EXISTS docs (id TEXT PRIMARY KEY, group_id INTEGER, title TEXT, body TEXT, department TEXT);
CREATE TABLE IF NOT EXISTS goals (group_id INTEGER, role_name TEXT, weekly_minutes INTEGER, PRIMARY KEY (group_id, role_name));
CREATE TABLE IF NOT EXISTS webhooks (id TEXT PRIMARY KEY, group_id INTEGER, name TEXT, url TEXT, events TEXT);
CREATE TABLE IF NOT EXISTS keys (id TEXT PRIMARY KEY, group_id INTEGER, name TEXT, token TEXT UNIQUE);
CREATE TABLE IF NOT EXISTS tickets (id TEXT PRIMARY KEY, group_id INTEGER, username TEXT, message TEXT, status TEXT, reply TEXT, created_at INTEGER);
CREATE TABLE IF NOT EXISTS commands (id TEXT PRIMARY KEY, group_id INTEGER, kind TEXT, user_id INTEGER, payload TEXT, status TEXT);
CREATE TABLE IF NOT EXISTS bans (id TEXT PRIMARY KEY, group_id INTEGER, user_id INTEGER, reason TEXT);
CREATE TABLE IF NOT EXISTS settings (group_id INTEGER PRIMARY KEY, brand TEXT, accent TEXT, idle_seconds INTEGER, shout TEXT);
CREATE TABLE IF NOT EXISTS teams (id TEXT PRIMARY KEY, group_id INTEGER, name TEXT, rank_min INTEGER);
CREATE TABLE IF NOT EXISTS notes (id TEXT PRIMARY KEY, group_id INTEGER, user_id INTEGER, body TEXT, actor TEXT);
"#;

fn seed(conn: &Connection) {
    conn.execute_batch(
        "DELETE FROM people WHERE group_id=900001;
         DELETE FROM chat WHERE group_id=900001;
         DELETE FROM logs WHERE group_id=900001;
         DELETE FROM events WHERE group_id=900001;
         DELETE FROM shifts WHERE group_id=900001;
         DELETE FROM claims WHERE group_id=900001;
         DELETE FROM forms WHERE group_id=900001;
         DELETE FROM applications WHERE group_id=900001;
         DELETE FROM punishments WHERE group_id=900001;
         DELETE FROM timeoff WHERE group_id=900001;
         DELETE FROM docs WHERE group_id=900001;
         DELETE FROM goals WHERE group_id=900001;
         DELETE FROM webhooks WHERE group_id=900001;
         DELETE FROM keys WHERE group_id=900001;
         DELETE FROM tickets WHERE group_id=900001;
         DELETE FROM settings WHERE group_id=900001;
         DELETE FROM teams WHERE group_id=900001;
         DELETE FROM groups WHERE id=900001;",
    ).ok();
}

pub fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

#[derive(Clone)]
pub struct Viewer {
    pub username: String,
    pub display_name: String,
    pub user_id: i64,
}

pub fn viewer(conn: &Connection, token: &str) -> Option<Viewer> {
    conn.query_row("SELECT user_id, username, display_name FROM auth WHERE token=?1", [token], |r| {
        Ok(Viewer { user_id: r.get(0)?, username: r.get(1)?, display_name: r.get(2)? })
    }).optional().ok().flatten()
}

#[derive(Clone)]
pub struct Group {
    pub id: i64,
    pub name: String,
    pub description: String,
    pub member_count: i64,
    pub owner_name: String,
    pub roles_json: String,
    pub has_cloud: bool,
    pub demo: bool,
}

pub fn groups(conn: &Connection) -> Vec<Group> {
    let mut stmt = conn.prepare("SELECT id, name, description, member_count, owner_name, roles_json, open_cloud_key, demo FROM groups ORDER BY demo ASC, name").unwrap();
    let rows = stmt.query_map([], |r| {
        Ok(Group {
            id: r.get(0)?,
            name: r.get(1)?,
            description: r.get(2)?,
            member_count: r.get(3)?,
            owner_name: r.get(4)?,
            roles_json: r.get(5)?,
            has_cloud: r.get::<_, Option<String>>(6)?.map(|s| !s.is_empty()).unwrap_or(false),
            demo: r.get::<_, i64>(7)? == 1,
        })
    }).unwrap();
    rows.filter_map(|r| r.ok()).collect()
}

pub fn group(conn: &Connection, id: i64) -> Option<Group> {
    groups(conn).into_iter().find(|g| g.id == id)
}

#[derive(Clone)]
pub struct Person {
    pub user_id: i64,
    pub username: String,
    pub display_name: String,
    pub role_name: String,
    pub role_rank: i64,
    pub status: String,
    pub active_min: i64,
    pub idle_min: i64,
    pub typing_min: i64,
    pub messages: i64,
    pub sessions: i64,
    pub last_seen: String,
    pub department: String,
    pub birthday: String,
    pub server_id: String,
}

pub fn people(conn: &Connection, gid: i64) -> Vec<Person> {
    let mut stmt = conn.prepare("SELECT user_id, username, display_name, role_name, role_rank, status, active_min, idle_min, typing_min, messages, sessions, last_seen, department, birthday, server_id FROM people WHERE group_id=?1 ORDER BY active_min DESC").unwrap();
    stmt.query_map([gid], |r| {
        Ok(Person {
            user_id: r.get(0)?, username: r.get(1)?, display_name: r.get(2)?, role_name: r.get(3)?, role_rank: r.get(4)?,
            status: r.get(5)?, active_min: r.get(6)?, idle_min: r.get(7)?, typing_min: r.get(8)?, messages: r.get(9)?,
            sessions: r.get(10)?, last_seen: r.get(11)?, department: r.get(12)?, birthday: r.get(13)?, server_id: r.get(14)?,
        })
    }).unwrap().filter_map(|r| r.ok()).collect()
}

pub fn log(conn: &Connection, gid: i64, kind: &str, actor: &str, message: &str) {
    conn.execute("INSERT INTO logs (group_id, kind, actor, message, created_at) VALUES (?1,?2,?3,?4,?5)", params![gid, kind, actor, message, now()]).ok();
}
