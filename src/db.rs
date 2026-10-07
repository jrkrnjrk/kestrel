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
    let n: i64 = conn.query_row("SELECT COUNT(*) FROM groups", [], |r| r.get(0)).unwrap_or(0);
    if n > 0 {
        return;
    }
    conn.execute("INSERT INTO groups (id, name, description, member_count, owner_name, roles_json, open_cloud_key, demo) VALUES (900001, 'Northline Bureau', 'Sample staff bureau so the desk is full before you bind a real group.', 1840, 'northline', ?, NULL, 1)", params![r#"[{"id":10,"name":"Recruit","rank":1},{"id":20,"name":"Officer","rank":10},{"id":30,"name":"Supervisor","rank":50},{"id":40,"name":"Command","rank":100},{"id":255,"name":"Owner","rank":255}]"#]).ok();
    let people = [
        (101, "ira.vale", "Ira Vale", "Command", 100, "online", 412, 38, 22, 186, 14, "Command", "03-18", "job-north-1"),
        (102, "moss.chen", "Moss Chen", "Supervisor", 50, "online", 366, 51, 19, 142, 11, "Patrol", "11-02", "job-north-1"),
        (103, "june.park", "June Park", "Supervisor", 50, "idle", 298, 90, 11, 97, 9, "Training", "07-29", "job-north-2"),
        (104, "theo.marc", "Theo Marc", "Officer", 10, "online", 254, 40, 16, 210, 8, "Dispatch", "01-09", "job-north-1"),
        (105, "silo.ade", "Silo Ade", "Officer", 10, "online", 221, 33, 8, 76, 7, "Patrol", "05-14", "job-north-3"),
        (106, "nori.beck", "Nori Beck", "Officer", 10, "offline", 188, 70, 6, 54, 6, "Patrol", "09-21", ""),
        (107, "wren.oto", "Wren Oto", "Recruit", 1, "online", 96, 22, 14, 41, 3, "Training", "12-04", "job-north-2"),
        (108, "cass.lin", "Cass Lin", "Recruit", 1, "idle", 74, 48, 4, 18, 2, "Training", "02-27", "job-north-2"),
    ];
    for p in people {
        conn.execute("INSERT INTO people (group_id, user_id, username, display_name, role_name, role_rank, status, active_min, idle_min, typing_min, messages, sessions, last_seen, department, birthday, server_id) VALUES (900001,?,?,?,?,?,?,?,?,?,?,?,datetime('now'),?,?,?)", params![p.0, p.1, p.2, p.3, p.4, p.5, p.6, p.7, p.8, p.9, p.10, p.11, p.12, p.13]).ok();
    }
    let chats = [
        ("theo.marc", "all", "Unit 4 on scene, requesting a supervisor."),
        ("moss.chen", "staff", "Copy. Ira is already en route."),
        ("ira.vale", "staff", "Hold the scene. Do not rank anyone until the log is written."),
        ("june.park", "training", "Recruit drill starts in ten. Wren, Cass, check in."),
        ("wren.oto", "training", "Checked in. Typing the incident form now."),
        ("silo.ade", "all", "Clear on the east gate."),
    ];
    for (i, c) in chats.iter().enumerate() {
        conn.execute("INSERT INTO chat (group_id, user_id, username, message, channel, created_at) VALUES (900001, 0, ?, ?, ?, ?)", params![c.0, c.2, c.1, 1_700_000_000 + i as i64]).ok();
    }
    let logs = [
        ("ranking", "ira.vale", "Ranked moss.chen → Supervisor"),
        ("session", "june.park", "Claimed evening patrol"),
        ("punishment", "ira.vale", "Warning for nori.beck: missed quota"),
        ("chat", "game", "Chat batch · 6 lines"),
        ("presence", "game", "Presence batch · 6 players"),
        ("application", "wren.oto", "Submitted Officer application · pending"),
    ];
    for (i, l) in logs.iter().enumerate() {
        conn.execute("INSERT INTO logs (group_id, kind, actor, message, created_at) VALUES (900001, ?, ?, ?, ?)", params![l.0, l.1, l.2, 1_700_000_100 + i as i64]).ok();
    }
    conn.execute_batch(r#"
INSERT INTO events (group_id, username, event_type, data_json, created_at) VALUES (900001, 'theo.marc', 'Admin Logs', '{"command":":bring silo.ade"}', 1700000200);
INSERT INTO events (group_id, username, event_type, data_json, created_at) VALUES (900001, 'june.park', 'Training', '{"drill":"radio check"}', 1700000300);
INSERT INTO shifts (id, group_id, title, place_id, slots, notes, created_at) VALUES ('shift-eve', 900001, 'Evening patrol', 0, 6, 'Claim a slot before 18:00.', 1700000000);
INSERT INTO shifts (id, group_id, title, place_id, slots, notes, created_at) VALUES ('shift-trn', 900001, 'Recruit drill', 0, 8, 'Training department only.', 1700000001);
INSERT INTO claims (id, shift_id, group_id, username, role_name, created_at) VALUES ('c1', 'shift-eve', 900001, 'june.park', 'Supervisor', 1700000400);
INSERT INTO forms (id, group_id, slug, name, description, quiz, pass_percent) VALUES ('form1', 900001, 'officer', 'Officer application', 'Patrol intake. Quiz mode scores the radio item.', 1, 70);
INSERT INTO questions (id, form_id, prompt, answer) VALUES ('q1', 'form1', 'Why this bureau?', '');
INSERT INTO questions (id, form_id, prompt, answer) VALUES ('q2', 'form1', 'Radio code for scene secure?', 'code 4');
INSERT INTO applications (id, form_id, group_id, username, answers, score, status, created_at) VALUES ('app1', 'form1', 900001, 'wren.oto', '{"q1":"I want patrol.","q2":"code 4"}', 100, 'pending', 1700000500);
INSERT INTO punishments (id, group_id, username, kind, reason, actor, created_at) VALUES ('pun1', 900001, 'nori.beck', 'warning', 'Missed weekly active quota', 'ira.vale', 1700000600);
INSERT INTO timeoff (id, group_id, username, starts_on, ends_on, reason, status, created_at) VALUES ('off1', 900001, 'cass.lin', '2026-10-12', '2026-10-14', 'Travel', 'pending', 1700000700);
INSERT INTO docs (id, group_id, title, body, department) VALUES ('doc1', 900001, 'Scene handbook', 'Do not rank from the game admin. Write the log, then rank from this desk.', 'All');
INSERT INTO docs (id, group_id, title, body, department) VALUES ('doc2', 900001, 'Training drill', 'Radio check, then a supervised stop. Claim the drill session before you start.', 'Training');
INSERT INTO goals (group_id, role_name, weekly_minutes) VALUES (900001, 'Officer', 180);
INSERT INTO goals (group_id, role_name, weekly_minutes) VALUES (900001, 'Supervisor', 240);
INSERT INTO webhooks (id, group_id, name, url, events) VALUES ('wh1', 900001, 'Staff Discord', 'https://discord.com/api/webhooks/example', 'rank.changed,chat.message,punishment.issued');
INSERT INTO keys (id, group_id, name, token) VALUES ('key1', 900001, 'game', 'kst_sample_replace_me');
INSERT INTO tickets (id, group_id, username, message, status, reply, created_at) VALUES ('t1', 900001, 'silo.ade', 'Player refusing orders at the gate.', 'open', '', 1700000800);
INSERT INTO settings (group_id, brand, accent, idle_seconds, shout) VALUES (900001, 'Northline Bureau', '#e7ff6a', 90, 'Evening patrol is live.');
INSERT INTO teams (id, group_id, name, rank_min) VALUES ('tm1', 900001, 'Command', 100);
INSERT INTO teams (id, group_id, name, rank_min) VALUES ('tm2', 900001, 'Patrol', 10);
"#).ok();
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
