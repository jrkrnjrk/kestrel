#!/usr/bin/env python3
"""Kestrel — Roblox group operations desk.

Bio-code login, activity, sessions, applications, punishments, time off,
knowledge, leaderboards, live commands, ranking API, webhooks, and the
in-game scripts. Railway: bind 0.0.0.0:$PORT. Persist with a volume at /data.
"""

from __future__ import annotations

import csv
import io
import json
import os
import secrets
import sqlite3
import threading
import time
import urllib.error
import urllib.parse
import urllib.request
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

ROOT = Path(__file__).resolve().parent
DATA_DIR = Path(os.environ.get("DATA_DIR") or ("/data" if Path("/data").is_dir() else ROOT / "data"))
DATA_DIR.mkdir(parents=True, exist_ok=True)
DB_PATH = DATA_DIR / "kestrel.db"
INDEX = ROOT / "index.html"
HOST = "0.0.0.0"
PORT = int(os.environ.get("PORT", "8787"))
CODE_TTL = 15 * 60
ALPHABET = "ABCDEFGHJKLMNPQRSTUVWXYZ23456789"
LOCK = threading.Lock()
ALLOWED = ("users.roblox.com", "groups.roblox.com", "thumbnails.roblox.com", "apis.roblox.com")

SCHEMA = """
CREATE TABLE IF NOT EXISTS challenges (
  id TEXT PRIMARY KEY, user_id INTEGER, username TEXT, display_name TEXT, code TEXT, expires_at INTEGER
);
CREATE TABLE IF NOT EXISTS sessions_auth (
  token TEXT PRIMARY KEY, user_id INTEGER, username TEXT, display_name TEXT, created_at INTEGER
);
CREATE TABLE IF NOT EXISTS groups (
  id INTEGER PRIMARY KEY, name TEXT, description TEXT, member_count INTEGER,
  owner_name TEXT, owner_id INTEGER, roles_json TEXT, open_cloud_key TEXT,
  added_by INTEGER, added_at INTEGER
);
CREATE TABLE IF NOT EXISTS people (
  group_id INTEGER, user_id INTEGER, username TEXT, display_name TEXT,
  role_name TEXT, role_rank INTEGER, status TEXT,
  active_min INTEGER DEFAULT 0, idle_min INTEGER DEFAULT 0, typing_min INTEGER DEFAULT 0,
  messages INTEGER DEFAULT 0, sessions INTEGER DEFAULT 0, last_seen TEXT,
  place_id INTEGER, server_id TEXT, birthday TEXT, department TEXT,
  PRIMARY KEY (group_id, user_id)
);
CREATE TABLE IF NOT EXISTS chat (
  id INTEGER PRIMARY KEY AUTOINCREMENT, group_id INTEGER, user_id INTEGER, username TEXT,
  message TEXT, channel TEXT, place_id INTEGER, created_at INTEGER, flagged INTEGER DEFAULT 0
);
CREATE TABLE IF NOT EXISTS logs (
  id INTEGER PRIMARY KEY AUTOINCREMENT, group_id INTEGER, kind TEXT, actor TEXT,
  message TEXT, meta_json TEXT, created_at INTEGER
);
CREATE TABLE IF NOT EXISTS webhooks (
  id TEXT PRIMARY KEY, group_id INTEGER, name TEXT, url TEXT, events_json TEXT,
  enabled INTEGER DEFAULT 1, created_at INTEGER
);
CREATE TABLE IF NOT EXISTS deliveries (
  id INTEGER PRIMARY KEY AUTOINCREMENT, group_id INTEGER, webhook_id TEXT, event TEXT,
  status INTEGER, detail TEXT, created_at INTEGER
);
CREATE TABLE IF NOT EXISTS api_keys (
  id TEXT PRIMARY KEY, group_id INTEGER, name TEXT, token TEXT UNIQUE, scopes TEXT, created_at INTEGER
);
CREATE TABLE IF NOT EXISTS shifts (
  id TEXT PRIMARY KEY, group_id INTEGER, title TEXT, place_id INTEGER, host TEXT,
  starts_at INTEGER, ends_at INTEGER, slots INTEGER, notes TEXT, created_at INTEGER
);
CREATE TABLE IF NOT EXISTS claims (
  id TEXT PRIMARY KEY, shift_id TEXT, group_id INTEGER, user_id INTEGER, username TEXT, role_name TEXT, created_at INTEGER
);
CREATE TABLE IF NOT EXISTS forms (
  id TEXT PRIMARY KEY, group_id INTEGER, slug TEXT UNIQUE, name TEXT, description TEXT,
  min_rank INTEGER DEFAULT 0, quiz INTEGER DEFAULT 0, pass_percent INTEGER DEFAULT 70,
  single INTEGER DEFAULT 1, created_at INTEGER
);
CREATE TABLE IF NOT EXISTS questions (
  id TEXT PRIMARY KEY, form_id TEXT, prompt TEXT, kind TEXT, options_json TEXT, answer TEXT, position INTEGER
);
CREATE TABLE IF NOT EXISTS applications (
  id TEXT PRIMARY KEY, form_id TEXT, group_id INTEGER, user_id INTEGER, username TEXT,
  answers_json TEXT, score INTEGER, status TEXT, review_note TEXT, created_at INTEGER
);
CREATE TABLE IF NOT EXISTS punishments (
  id TEXT PRIMARY KEY, group_id INTEGER, user_id INTEGER, username TEXT, kind TEXT,
  reason TEXT, expires_at INTEGER, actor TEXT, role_id INTEGER, created_at INTEGER
);
CREATE TABLE IF NOT EXISTS timeoff (
  id TEXT PRIMARY KEY, group_id INTEGER, user_id INTEGER, username TEXT, starts_on TEXT,
  ends_on TEXT, reason TEXT, status TEXT, created_at INTEGER
);
CREATE TABLE IF NOT EXISTS docs (
  id TEXT PRIMARY KEY, group_id INTEGER, title TEXT, body TEXT, department TEXT, updated_at INTEGER
);
CREATE TABLE IF NOT EXISTS goals (
  group_id INTEGER, role_name TEXT, weekly_minutes INTEGER, weekly_sessions INTEGER,
  PRIMARY KEY (group_id, role_name)
);
CREATE TABLE IF NOT EXISTS notes (
  id TEXT PRIMARY KEY, group_id INTEGER, user_id INTEGER, body TEXT, actor TEXT, created_at INTEGER
);
CREATE TABLE IF NOT EXISTS teams (
  id TEXT PRIMARY KEY, group_id INTEGER, name TEXT, rank_min INTEGER, permissions_json TEXT
);
CREATE TABLE IF NOT EXISTS tickets (
  id TEXT PRIMARY KEY, group_id INTEGER, user_id INTEGER, username TEXT, message TEXT,
  status TEXT, reply TEXT, created_at INTEGER
);
CREATE TABLE IF NOT EXISTS commands (
  id TEXT PRIMARY KEY, group_id INTEGER, kind TEXT, user_id INTEGER, payload TEXT,
  status TEXT, created_at INTEGER
);
CREATE TABLE IF NOT EXISTS events (
  id INTEGER PRIMARY KEY AUTOINCREMENT, group_id INTEGER, user_id INTEGER, username TEXT,
  event_type TEXT, data_json TEXT, created_at INTEGER
);
CREATE TABLE IF NOT EXISTS settings (
  group_id INTEGER PRIMARY KEY, brand TEXT, accent TEXT, week_start INTEGER DEFAULT 1,
  idle_seconds INTEGER DEFAULT 90, shout TEXT
);
CREATE TABLE IF NOT EXISTS bans (
  id TEXT PRIMARY KEY, group_id INTEGER, user_id INTEGER, username TEXT, reason TEXT,
  expires_at INTEGER, created_at INTEGER
);
"""


def db():
    conn = sqlite3.connect(DB_PATH)
    conn.row_factory = sqlite3.Row
    return conn


def init_db():
    with LOCK, db() as conn:
        conn.executescript(SCHEMA)
        cols = {r["name"] for r in conn.execute("PRAGMA table_info(people)").fetchall()}
        for name, decl in (("birthday", "TEXT"), ("department", "TEXT")):
            if name not in cols:
                conn.execute(f"ALTER TABLE people ADD COLUMN {name} {decl}")
        kcols = {r["name"] for r in conn.execute("PRAGMA table_info(api_keys)").fetchall()}
        if "scopes" not in kcols:
            conn.execute("ALTER TABLE api_keys ADD COLUMN scopes TEXT")


def now():
    return int(time.time())


def code():
    raw = "".join(secrets.choice(ALPHABET) for _ in range(8))
    return f"kestrel-{raw[:4]}-{raw[4:]}"


def roblox(method, url, body=None, headers=None, timeout=20):
    host = urllib.parse.urlparse(url).netloc
    if host not in ALLOWED:
        raise ValueError("host not allowed")
    data = None if body is None else json.dumps(body).encode()
    hdrs = {"User-Agent": "KestrelPanel/1.0", "Accept": "application/json"}
    if data is not None:
        hdrs["Content-Type"] = "application/json"
    if headers:
        hdrs.update(headers)
    req = urllib.request.Request(url, data=data, headers=hdrs, method=method)
    try:
        with urllib.request.urlopen(req, timeout=timeout) as res:
            raw = res.read()
            return res.status, json.loads(raw.decode() or "{}")
    except urllib.error.HTTPError as err:
        raw = err.read().decode("utf-8", "replace")
        try:
            payload = json.loads(raw) if raw else {}
        except json.JSONDecodeError:
            payload = {"error": raw[:400]}
        return err.code, payload


def lookup_username(username):
    status, payload = roblox("POST", "https://users.roblox.com/v1/usernames/users", {"usernames": [username], "excludeBannedUsers": False})
    if status != 200 or not payload.get("data"):
        return None
    hit = payload["data"][0]
    return {"id": hit["id"], "username": hit["name"], "displayName": hit.get("displayName") or hit["name"]}


def fetch_user(user_id):
    status, payload = roblox("GET", f"https://users.roblox.com/v1/users/{int(user_id)}")
    return payload if status == 200 else None


def fetch_group(group_id):
    status, payload = roblox("GET", f"https://groups.roblox.com/v1/groups/{int(group_id)}")
    if status != 200 or "id" not in payload:
        return None, payload
    rstatus, roles = roblox("GET", f"https://groups.roblox.com/v1/groups/{int(group_id)}/roles")
    role_list = roles.get("roles", []) if rstatus == 200 else []
    owner = payload.get("owner") or {}
    return {
        "id": payload["id"],
        "name": payload.get("name") or f"Group {group_id}",
        "description": payload.get("description") or "",
        "memberCount": payload.get("memberCount") or 0,
        "ownerName": owner.get("username") or "",
        "ownerId": owner.get("userId") or 0,
        "roles": [{"id": r.get("id"), "name": r.get("name"), "rank": r.get("rank"), "memberCount": r.get("memberCount") or 0} for r in role_list],
    }, None


def bio_has_code(description, token):
    return token in (description or "").replace("\u200b", "").replace("\ufeff", "")


def write_log(conn, group_id, kind, actor, message, meta=None):
    conn.execute(
        "INSERT INTO logs (group_id, kind, actor, message, meta_json, created_at) VALUES (?,?,?,?,?,?)",
        (group_id, kind, actor, message, json.dumps(meta or {}), now()),
    )


def fire_webhooks(group_id, event, payload, only_id=None):
    with db() as conn:
        hooks = conn.execute("SELECT * FROM webhooks WHERE group_id=? AND enabled=1", (group_id,)).fetchall()
    for hook in hooks:
        if only_id and hook["id"] != only_id:
            continue
        events = json.loads(hook["events_json"] or "[]")
        if events and event not in events and "*" not in events and not only_id:
            continue
        body = {
            "username": "Kestrel",
            "embeds": [{
                "title": event,
                "description": str(payload.get("message") or event)[:1800],
                "color": 14083850,
                "fields": [{"name": k, "value": str(v)[:200], "inline": True} for k, v in payload.items() if k != "message"][:6],
            }],
        }
        status, detail = 0, "ok"
        try:
            req = urllib.request.Request(hook["url"], data=json.dumps(body).encode(), headers={"Content-Type": "application/json", "User-Agent": "KestrelPanel/1.0"}, method="POST")
            with urllib.request.urlopen(req, timeout=8) as res:
                status = res.status
        except urllib.error.HTTPError as err:
            status, detail = err.code, err.read().decode("utf-8", "replace")[:300]
        except Exception as err:
            detail = str(err)[:300]
        with LOCK, db() as conn:
            conn.execute(
                "INSERT INTO deliveries (group_id, webhook_id, event, status, detail, created_at) VALUES (?,?,?,?,?,?)",
                (group_id, hook["id"], event, status, detail, now()),
            )


def public_group(row):
    return {
        "id": row["id"], "name": row["name"], "description": row["description"],
        "memberCount": row["member_count"], "ownerName": row["owner_name"], "ownerId": row["owner_id"],
        "roles": json.loads(row["roles_json"] or "[]"), "hasOpenCloud": bool(row["open_cloud_key"]), "addedAt": row["added_at"],
    }


def person_row(row):
    return {
        "userId": row["user_id"], "username": row["username"], "displayName": row["display_name"],
        "roleName": row["role_name"], "roleRank": row["role_rank"], "status": row["status"],
        "activeMin": row["active_min"], "idleMin": row["idle_min"], "typingMin": row["typing_min"],
        "messages": row["messages"], "sessions": row["sessions"], "lastSeen": row["last_seen"],
        "placeId": row["place_id"], "serverId": row["server_id"], "birthday": row["birthday"],
        "department": row["department"],
    }


def loader_script(base, key, group_id, idle):
    return f'''-- Kestrel loader. Put this in ServerScriptService.
-- Game Settings > Security > Allow HTTP Requests = On
local HttpService = game:GetService("HttpService")
local Players = game:GetService("Players")
local TextChatService = game:GetService("TextChatService")
local ReplicatedStorage = game:GetService("ReplicatedStorage")

local API = "{base}"
local KEY = "{key}"
local GROUP_ID = {int(group_id)}
local IDLE_SECONDS = {int(idle)}
local FLUSH = 60

local signal = ReplicatedStorage:FindFirstChild("KestrelSignal")
if not signal then
	signal = Instance.new("RemoteEvent")
	signal.Name = "KestrelSignal"
	signal.Parent = ReplicatedStorage
end

local state = {{}}

local function request(method, path, body)
	local ok, res = pcall(function()
		return HttpService:RequestAsync({{
			Url = API .. path,
			Method = method,
			Headers = {{ Authorization = "Bearer " .. KEY, ["Content-Type"] = "application/json" }},
			Body = body and HttpService:JSONEncode(body) or nil,
		}})
	end)
	if not ok or not res.Success then
		return nil
	end
	local decoded = nil
	pcall(function()
		decoded = HttpService:JSONDecode(res.Body)
	end)
	return decoded
end

local function ensure(player)
	local row = state[player.UserId]
	if row then return row end
	row = {{
		username = player.Name,
		displayName = player.DisplayName,
		pos = nil,
		moved = os.clock(),
		focused = true,
		typingOn = false,
		active = 0, idle = 0, typingMin = 0,
		messages = 0,
	}}
	state[player.UserId] = row
	return row
end

signal.OnServerEvent:Connect(function(player, kind, value)
	local row = ensure(player)
	if kind == "focus" then row.focused = value == true end
	if kind == "typing" then row.typingOn = value == true end
end)

local function onChat(player, message, channel)
	local row = ensure(player)
	row.messages += 1
	row.moved = os.clock()
	request("POST", "/api/ingest/chat", {{
		userId = player.UserId,
		username = player.Name,
		message = message,
		channel = channel or "all",
		placeId = game.PlaceId,
	}})
end

Players.PlayerAdded:Connect(function(player)
	ensure(player)
	player.Chatted:Connect(function(message)
		onChat(player, message, "classic")
	end)
end)
for _, player in Players:GetPlayers() do
	ensure(player)
end

pcall(function()
	TextChatService.MessageReceived:Connect(function(msg)
		local src = msg.TextSource
		if not src then return end
		local player = Players:GetPlayerByUserId(src.UserId)
		if player then onChat(player, msg.Text, msg.TextChannel and msg.TextChannel.Name or "text") end
	end)
end)

local function applyCommand(cmd)
	local player = Players:GetPlayerByUserId(cmd.userId or 0)
	if cmd.kind == "notify" and player then
		signal:FireClient(player, "notify", cmd.payload and cmd.payload.message or "Staff notice")
	elseif cmd.kind == "kick" and player then
		player:Kick(cmd.payload and cmd.payload.reason or "Removed by staff")
	elseif cmd.kind == "ban" and player then
		player:Kick(cmd.payload and cmd.payload.reason or "Banned")
	end
end

task.spawn(function()
	while true do
		local data = request("GET", "/api/ingest/commands")
		if data and data.commands then
			for _, cmd in data.commands do applyCommand(cmd) end
		end
		if data and data.bans then
			for _, ban in data.bans do
				local player = Players:GetPlayerByUserId(ban.userId)
				if player then player:Kick(ban.reason or "Banned") end
			end
		end
		task.wait(8)
	end
end)

task.spawn(function()
	while true do
		local batch = {{}}
		for _, player in Players:GetPlayers() do
			local row = ensure(player)
			local char = player.Character
			local root = char and char:FindFirstChild("HumanoidRootPart")
			local moved = false
			if root then
				if row.pos and (root.Position - row.pos).Magnitude > 1.5 then moved = true end
				row.pos = root.Position
			end
			if moved then row.moved = os.clock() end
			local idle = (not row.focused) or (os.clock() - row.moved) > IDLE_SECONDS
			local slice = FLUSH / 60
			if row.typingOn then row.typingMin += slice
			elseif idle then row.idle += slice
			else row.active += slice end
			table.insert(batch, {{
				userId = player.UserId,
				username = player.Name,
				displayName = player.DisplayName,
				status = idle and "idle" or "online",
				activeMin = math.floor(row.active + 0.5),
				idleMin = math.floor(row.idle + 0.5),
				typingMin = math.floor(row.typingMin + 0.5),
				messages = row.messages,
				sessions = 1,
				placeId = game.PlaceId,
				serverId = game.JobId,
			}})
			row.active, row.idle, row.typingMin, row.messages = 0, 0, 0, 0
		end
		if #batch > 0 then request("POST", "/api/ingest/presence", {{ players = batch }}) end
		task.wait(FLUSH)
	end
end)

-- Admin log helper other scripts can call:
-- require(path).track(player, "Admin Logs", {{ command = ":kick name" }})
local Kestrel = {{}}
function Kestrel.track(player, eventType, data)
	request("POST", "/api/ingest/event", {{
		userId = player and player.UserId or 0,
		username = player and player.Name or "server",
		eventType = eventType,
		data = data or {{}},
	}})
end
function Kestrel.rank(userId, roleId)
	return request("POST", "/api/v1/rank", {{ userId = userId, roleId = roleId }})
end
function Kestrel.exile(userId)
	return request("POST", "/api/v1/exile", {{ userId = userId }})
end
_G.Kestrel = Kestrel
print("[Kestrel] loader online for group", GROUP_ID)
'''


def client_script():
    return '''-- Kestrel client. Put this in StarterPlayer > StarterPlayerScripts.
local ReplicatedStorage = game:GetService("ReplicatedStorage")
local UserInputService = game:GetService("UserInputService")
local TextChatService = game:GetService("TextChatService")
local signal = ReplicatedStorage:WaitForChild("KestrelSignal")

local function pushFocus()
	signal:FireServer("focus", UserInputService.WindowFocused)
end
UserInputService.WindowFocused:Connect(function() signal:FireServer("focus", true) end)
UserInputService.WindowFocusReleased:Connect(function() signal:FireServer("focus", false) end)
pushFocus()

UserInputService.TextBoxFocused:Connect(function() signal:FireServer("typing", true) end)
UserInputService.TextBoxFocusReleased:Connect(function() signal:FireServer("typing", false) end)

signal.OnClientEvent:Connect(function(kind, message)
	if kind == "notify" then
		pcall(function()
			game:GetService("StarterGui"):SetCore("ChatMakeSystemMessage", { Text = "[Staff] " .. tostring(message), Color = Color3.fromRGB(214, 242, 92) })
		end)
	end
end)
'''


class Handler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, fmt, *args):
        print(f"[{self.address_string()}] {fmt % args}")

    def _cors(self):
        return {
            "Access-Control-Allow-Origin": "*",
            "Access-Control-Allow-Headers": "Authorization, Content-Type",
            "Access-Control-Allow-Methods": "GET, POST, PATCH, DELETE, OPTIONS",
        }

    def _send(self, code, payload=None, content_type="application/json", extra=None):
        raw = b"" if payload is None else (payload if isinstance(payload, bytes) else json.dumps(payload).encode())
        self.send_response(code)
        self.send_header("Content-Type", content_type)
        self.send_header("Content-Length", str(len(raw)))
        self.send_header("Cache-Control", "no-store")
        headers = self._cors()
        if extra:
            headers.update(extra)
        for k, v in headers.items():
            self.send_header(k, v)
        self.end_headers()
        self.wfile.write(raw)

    def _read(self):
        length = int(self.headers.get("Content-Length") or 0)
        if not length:
            return {}
        raw = self.rfile.read(length)
        return json.loads(raw.decode() or "{}")

    def _session(self):
        header = self.headers.get("Authorization") or ""
        token = header[7:].strip() if header.lower().startswith("bearer ") else ""
        if not token:
            return None
        with db() as conn:
            return conn.execute("SELECT * FROM sessions_auth WHERE token=?", (token,)).fetchone()

    def _key(self):
        header = self.headers.get("Authorization") or ""
        token = header[7:].strip() if header.lower().startswith("bearer ") else ""
        if not token:
            return None
        with db() as conn:
            return conn.execute("SELECT * FROM api_keys WHERE token=?", (token,)).fetchone()

    def _origin(self):
        host = self.headers.get("X-Forwarded-Host") or self.headers.get("Host") or f"127.0.0.1:{PORT}"
        proto = self.headers.get("X-Forwarded-Proto") or "https"
        if host.startswith("127.") or host.startswith("localhost"):
            proto = "http"
        return f"{proto}://{host}"

    def do_OPTIONS(self):
        self._send(204, b"", "text/plain")

    def do_GET(self):
        parsed = urllib.parse.urlparse(self.path)
        path, qs = parsed.path, urllib.parse.parse_qs(parsed.query)
        if path in ("/", "/index.html"):
            return self._send(200, INDEX.read_bytes(), "text/html; charset=utf-8") if INDEX.exists() else self._send(500, {"error": "index missing"})
        if path == "/api/health":
            return self._send(200, {"ok": True, "service": "kestrel"})
        if path.startswith("/apply/"):
            return self._apply_page(path.split("/")[-1])
        if path == "/api/me":
            row = self._session()
            return self._send(200, {"userId": row["user_id"], "username": row["username"], "displayName": row["display_name"]}) if row else self._send(401, {"error": "login required"})
        if path == "/api/groups":
            if not self._session():
                return self._send(401, {"error": "login required"})
            with db() as conn:
                rows = conn.execute("SELECT * FROM groups ORDER BY added_at DESC").fetchall()
            return self._send(200, {"groups": [public_group(r) for r in rows]})
        if path == "/api/ingest/commands":
            return self._poll(qs)
        parts = [p for p in path.split("/") if p]
        if len(parts) >= 3 and parts[0] == "api" and parts[1] == "groups":
            return self._group_get(int(parts[2]), parts[3:], qs)
        self._send(404, {"error": "not found"})

    def do_POST(self):
        parsed = urllib.parse.urlparse(self.path)
        path = parsed.path
        if path.startswith("/apply/") and "application/json" not in (self.headers.get("Content-Type") or ""):
            length = int(self.headers.get("Content-Length") or 0)
            raw = self.rfile.read(length).decode() if length else ""
            body = {k: urllib.parse.unquote_plus(v[-1]) for k, v in urllib.parse.parse_qs(raw).items()}
            return self._apply_submit(path.split("/")[-1], body, html=True)
        try:
            body = self._read()
        except json.JSONDecodeError:
            return self._send(400, {"error": "invalid json"})
        if path == "/api/auth/start":
            return self._auth_start(body)
        if path == "/api/auth/verify":
            return self._auth_verify(body)
        if path == "/api/auth/logout":
            return self._logout()
        if path == "/api/groups":
            return self._add_group(body)
        if path == "/api/ingest/presence":
            return self._ingest_presence(body)
        if path == "/api/ingest/chat":
            return self._ingest_chat(body)
        if path == "/api/ingest/event":
            return self._ingest_event(body)
        if path == "/api/ingest/ticket":
            return self._ingest_ticket(body)
        if path == "/api/v1/rank":
            return self._rank(body, external=True)
        if path == "/api/v1/exile":
            return self._exile(body, external=True)
        if path == "/api/v1/shout":
            return self._shout(body, external=True)
        if path.startswith("/apply/"):
            return self._apply_submit(path.split("/")[-1], body, html=False)
        parts = [p for p in path.split("/") if p]
        if len(parts) >= 3 and parts[0] == "api" and parts[1] == "groups":
            return self._group_post(int(parts[2]), parts[3:], body)
        self._send(404, {"error": "not found"})

    def do_DELETE(self):
        parts = [p for p in urllib.parse.urlparse(self.path).path.split("/") if p]
        if not self._session():
            return self._send(401, {"error": "login required"})
        if len(parts) == 3 and parts[:2] == ["api", "groups"]:
            gid = int(parts[2])
            with LOCK, db() as conn:
                conn.execute("DELETE FROM groups WHERE id=?", (gid,))
                write_log(conn, gid, "group", self._session()["username"], f"Removed group {gid}")
            return self._send(200, {"ok": True})
        if len(parts) == 5 and parts[3] in ("webhooks", "keys", "docs", "forms", "shifts"):
            table = {"webhooks": "webhooks", "keys": "api_keys", "docs": "docs", "forms": "forms", "shifts": "shifts"}[parts[3]]
            with LOCK, db() as conn:
                conn.execute(f"DELETE FROM {table} WHERE id=? AND group_id=?", (parts[4], int(parts[2])))
            return self._send(200, {"ok": True})
        self._send(404, {"error": "not found"})

    def _group_get(self, gid, rest, qs):
        if not self._session():
            return self._send(401, {"error": "login required"})
        name = rest[0] if rest else ""
        with db() as conn:
            if name == "people" and len(rest) == 1:
                rows = conn.execute("SELECT * FROM people WHERE group_id=? ORDER BY active_min DESC", (gid,)).fetchall()
                return self._send(200, {"people": [person_row(r) for r in rows]})
            if name == "people" and len(rest) == 2:
                uid = int(rest[1])
                person = conn.execute("SELECT * FROM people WHERE group_id=? AND user_id=?", (gid, uid)).fetchone()
                notes = conn.execute("SELECT * FROM notes WHERE group_id=? AND user_id=? ORDER BY created_at DESC", (gid, uid)).fetchall()
                pun = conn.execute("SELECT * FROM punishments WHERE group_id=? AND user_id=? ORDER BY created_at DESC", (gid, uid)).fetchall()
                off = conn.execute("SELECT * FROM timeoff WHERE group_id=? AND user_id=? ORDER BY created_at DESC", (gid, uid)).fetchall()
                return self._send(200, {
                    "person": person_row(person) if person else None,
                    "notes": [dict(r) for r in notes],
                    "punishments": [dict(r) for r in pun],
                    "timeoff": [dict(r) for r in off],
                })
            if name == "chat":
                rows = conn.execute("SELECT * FROM chat WHERE group_id=? ORDER BY id DESC LIMIT 400", (gid,)).fetchall()
                return self._send(200, {"chat": [{"id": r["id"], "userId": r["user_id"], "username": r["username"], "message": r["message"], "channel": r["channel"], "placeId": r["place_id"], "createdAt": r["created_at"], "flagged": bool(r["flagged"])} for r in rows]})
            if name == "logs":
                rows = conn.execute("SELECT * FROM logs WHERE group_id=? OR group_id=0 ORDER BY id DESC LIMIT 500", (gid,)).fetchall()
                deliveries = conn.execute("SELECT * FROM deliveries WHERE group_id=? ORDER BY id DESC LIMIT 40", (gid,)).fetchall()
                return self._send(200, {
                    "logs": [{"id": r["id"], "kind": r["kind"], "actor": r["actor"], "message": r["message"], "meta": json.loads(r["meta_json"] or "{}"), "createdAt": r["created_at"]} for r in rows],
                    "deliveries": [dict(r) for r in deliveries],
                })
            if name == "webhooks":
                rows = conn.execute("SELECT * FROM webhooks WHERE group_id=?", (gid,)).fetchall()
                return self._send(200, {"webhooks": [{"id": r["id"], "name": r["name"], "url": r["url"], "events": json.loads(r["events_json"] or "[]"), "enabled": bool(r["enabled"])} for r in rows]})
            if name == "keys":
                rows = conn.execute("SELECT * FROM api_keys WHERE group_id=?", (gid,)).fetchall()
                return self._send(200, {"keys": [{"id": r["id"], "name": r["name"], "token": r["token"], "scopes": r["scopes"], "createdAt": r["created_at"]} for r in rows]})
            if name == "overview":
                return self._overview(conn, gid)
            if name == "shifts":
                rows = conn.execute("SELECT * FROM shifts WHERE group_id=? ORDER BY starts_at DESC", (gid,)).fetchall()
                claims = conn.execute("SELECT * FROM claims WHERE group_id=?", (gid,)).fetchall()
                return self._send(200, {"shifts": [dict(r) for r in rows], "claims": [dict(r) for r in claims]})
            if name == "forms":
                rows = conn.execute("SELECT * FROM forms WHERE group_id=?", (gid,)).fetchall()
                apps = conn.execute("SELECT * FROM applications WHERE group_id=? ORDER BY created_at DESC", (gid,)).fetchall()
                qs_rows = conn.execute("SELECT questions.* FROM questions JOIN forms ON forms.id=questions.form_id WHERE forms.group_id=?", (gid,)).fetchall()
                return self._send(200, {"forms": [dict(r) for r in rows], "applications": [dict(r) for r in apps], "questions": [dict(r) for r in qs_rows]})
            if name == "punishments":
                return self._send(200, {"punishments": [dict(r) for r in conn.execute("SELECT * FROM punishments WHERE group_id=? ORDER BY created_at DESC", (gid,))]})
            if name == "timeoff":
                return self._send(200, {"timeoff": [dict(r) for r in conn.execute("SELECT * FROM timeoff WHERE group_id=? ORDER BY created_at DESC", (gid,))]})
            if name == "docs":
                return self._send(200, {"docs": [dict(r) for r in conn.execute("SELECT * FROM docs WHERE group_id=? ORDER BY updated_at DESC", (gid,))]})
            if name == "goals":
                return self._send(200, {"goals": [dict(r) for r in conn.execute("SELECT * FROM goals WHERE group_id=?", (gid,))]})
            if name == "teams":
                return self._send(200, {"teams": [dict(r) for r in conn.execute("SELECT * FROM teams WHERE group_id=?", (gid,))]})
            if name == "tickets":
                return self._send(200, {"tickets": [dict(r) for r in conn.execute("SELECT * FROM tickets WHERE group_id=? ORDER BY created_at DESC", (gid,))]})
            if name == "events":
                return self._send(200, {"events": [dict(r) for r in conn.execute("SELECT * FROM events WHERE group_id=? ORDER BY id DESC LIMIT 300", (gid,))]})
            if name == "bans":
                return self._send(200, {"bans": [dict(r) for r in conn.execute("SELECT * FROM bans WHERE group_id=? ORDER BY created_at DESC", (gid,))]})
            if name == "settings":
                row = conn.execute("SELECT * FROM settings WHERE group_id=?", (gid,)).fetchone()
                return self._send(200, {"settings": dict(row) if row else {"brand": "Kestrel", "accent": "#d6f25c", "weekStart": 1, "idleSeconds": 90, "shout": ""}})
            if name == "leaderboard":
                rows = conn.execute("SELECT * FROM people WHERE group_id=? ORDER BY active_min DESC LIMIT 50", (gid,)).fetchall()
                return self._send(200, {"leaderboard": [person_row(r) for r in rows]})
            if name == "export":
                return self._export(conn, gid)
            if name == "script":
                return self._script(conn, gid)
        self._send(404, {"error": "not found"})

    def _overview(self, conn, gid):
        people = conn.execute("SELECT * FROM people WHERE group_id=?", (gid,)).fetchall()
        return self._send(200, {
            "online": sum(1 for p in people if p["status"] in ("online", "idle")),
            "tracked": len(people),
            "activeMin": sum(p["active_min"] or 0 for p in people),
            "idleMin": sum(p["idle_min"] or 0 for p in people),
            "typingMin": sum(p["typing_min"] or 0 for p in people),
            "messages": sum(p["messages"] or 0 for p in people),
            "chatRows": conn.execute("SELECT COUNT(*) c FROM chat WHERE group_id=?", (gid,)).fetchone()["c"],
            "logRows": conn.execute("SELECT COUNT(*) c FROM logs WHERE group_id=?", (gid,)).fetchone()["c"],
            "openTickets": conn.execute("SELECT COUNT(*) c FROM tickets WHERE group_id=? AND status='open'", (gid,)).fetchone()["c"],
            "pendingOff": conn.execute("SELECT COUNT(*) c FROM timeoff WHERE group_id=? AND status='pending'", (gid,)).fetchone()["c"],
            "pendingApps": conn.execute("SELECT COUNT(*) c FROM applications WHERE group_id=? AND status='pending'", (gid,)).fetchone()["c"],
        })

    def _export(self, conn, gid):
        rows = conn.execute("SELECT * FROM people WHERE group_id=? ORDER BY active_min DESC", (gid,)).fetchall()
        buf = io.StringIO()
        writer = csv.writer(buf)
        writer.writerow(["userId", "username", "role", "status", "activeMin", "idleMin", "typingMin", "messages", "sessions", "lastSeen", "department"])
        for r in rows:
            writer.writerow([r["user_id"], r["username"], r["role_name"], r["status"], r["active_min"], r["idle_min"], r["typing_min"], r["messages"], r["sessions"], r["last_seen"], r["department"]])
        return self._send(200, buf.getvalue().encode(), "text/csv", {"Content-Disposition": f"attachment; filename=kestrel-{gid}.csv"})

    def _script(self, conn, gid):
        key = conn.execute("SELECT token FROM api_keys WHERE group_id=? ORDER BY created_at DESC LIMIT 1", (gid,)).fetchone()
        settings = conn.execute("SELECT * FROM settings WHERE group_id=?", (gid,)).fetchone()
        idle = settings["idle_seconds"] if settings else 90
        token = key["token"] if key else "PASTE_KEY"
        return self._send(200, {
            "loader": loader_script(self._origin(), token, gid, idle),
            "client": client_script(),
            "hasKey": bool(key),
        })

    def _group_post(self, gid, rest, body):
        session = self._session()
        if not session and rest != ["rank"] :
            if rest and rest[0] in ("rank",):
                pass
            else:
                return self._send(401, {"error": "login required"})
        actor = session["username"] if session else "api"
        name = rest[0] if rest else ""
        if name == "refresh":
            return self._refresh_group(gid)
        if name == "cloud":
            return self._set_cloud(gid, body, actor)
        if name == "webhooks" and len(rest) == 1:
            return self._add_webhook(gid, body, actor)
        if name == "webhooks" and len(rest) == 2:
            fire_webhooks(gid, "webhook.test", {"message": "Kestrel test ping", "groupId": gid}, only_id=rest[1])
            return self._send(200, {"ok": True})
        if name == "keys":
            return self._add_key(gid, body, actor)
        if name == "rank":
            return self._rank({**body, "groupId": gid}, external=False)
        if name == "exile":
            return self._exile({**body, "groupId": gid}, external=False)
        if name == "shout":
            return self._shout({**body, "groupId": gid}, external=False)
        if name == "shifts":
            return self._add_shift(gid, body, actor)
        if name == "claims":
            return self._claim(gid, body, actor)
        if name == "forms":
            return self._add_form(gid, body, actor)
        if name == "questions":
            return self._add_question(gid, body)
        if name == "review":
            return self._review(gid, body, actor)
        if name == "punishments":
            return self._punish(gid, body, actor)
        if name == "timeoff":
            return self._timeoff(gid, body, actor)
        if name == "docs":
            return self._add_doc(gid, body, actor)
        if name == "goals":
            return self._goal(gid, body)
        if name == "notes":
            return self._note(gid, body, actor)
        if name == "teams":
            return self._team(gid, body)
        if name == "tickets":
            return self._ticket_reply(gid, body, actor)
        if name == "commands":
            return self._command(gid, body, actor)
        if name == "settings":
            return self._settings(gid, body, actor)
        if name == "birthday":
            return self._birthday(gid, body)
        self._send(404, {"error": "not found"})

    def _auth_start(self, body):
        username = (body.get("username") or "").strip().lstrip("@")
        if not username or len(username) > 20:
            return self._send(400, {"error": "enter a Roblox username"})
        user = lookup_username(username)
        if not user:
            return self._send(404, {"error": "Roblox user not found"})
        token, cid = code(), secrets.token_hex(8)
        with LOCK, db() as conn:
            conn.execute("DELETE FROM challenges WHERE username=?", (user["username"],))
            conn.execute("INSERT INTO challenges (id, user_id, username, display_name, code, expires_at) VALUES (?,?,?,?,?,?)", (cid, user["id"], user["username"], user["displayName"], token, now() + CODE_TTL))
        return self._send(200, {
            "challengeId": cid, "userId": user["id"], "username": user["username"], "displayName": user["displayName"],
            "code": token, "expiresIn": CODE_TTL,
            "profileUrl": f"https://www.roblox.com/users/{user['id']}/profile",
            "avatar": f"https://www.roblox.com/headshot-thumbnail/image?userId={user['id']}&width=150&height=150&format=png",
        })

    def _auth_verify(self, body):
        with db() as conn:
            row = conn.execute("SELECT * FROM challenges WHERE id=?", (body.get("challengeId") or "",)).fetchone()
        if not row:
            return self._send(404, {"error": "start login again"})
        if row["expires_at"] < now():
            return self._send(410, {"error": "code expired"})
        user = fetch_user(row["user_id"])
        if not user:
            return self._send(502, {"error": "could not read Roblox profile"})
        if not bio_has_code(user.get("description"), row["code"]):
            return self._send(409, {"error": "code not in About yet", "hint": "Paste the code into About, save, then verify."})
        token = secrets.token_urlsafe(32)
        with LOCK, db() as conn:
            conn.execute("DELETE FROM challenges WHERE id=?", (row["id"],))
            conn.execute("INSERT INTO sessions_auth (token, user_id, username, display_name, created_at) VALUES (?,?,?,?,?)", (token, row["user_id"], row["username"], row["display_name"], now()))
            write_log(conn, 0, "auth", row["username"], "Signed in with bio code")
        return self._send(200, {"token": token, "userId": row["user_id"], "username": row["username"], "displayName": row["display_name"]})

    def _logout(self):
        header = self.headers.get("Authorization") or ""
        token = header[7:].strip() if header.lower().startswith("bearer ") else ""
        if token:
            with LOCK, db() as conn:
                conn.execute("DELETE FROM sessions_auth WHERE token=?", (token,))
        return self._send(200, {"ok": True})

    def _add_group(self, body):
        session = self._session()
        if not session:
            return self._send(401, {"error": "login required"})
        try:
            gid = int(body.get("groupId"))
        except (TypeError, ValueError):
            return self._send(400, {"error": "group id required"})
        group, err = fetch_group(gid)
        if not group:
            return self._send(404, {"error": "group not found", "detail": err})
        with LOCK, db() as conn:
            conn.execute(
                """INSERT INTO groups (id, name, description, member_count, owner_name, owner_id, roles_json, open_cloud_key, added_by, added_at)
                   VALUES (?,?,?,?,?,?,?,?,?,?)
                   ON CONFLICT(id) DO UPDATE SET name=excluded.name, description=excluded.description, member_count=excluded.member_count,
                   owner_name=excluded.owner_name, owner_id=excluded.owner_id, roles_json=excluded.roles_json""",
                (group["id"], group["name"], group["description"], group["memberCount"], group["ownerName"], group["ownerId"], json.dumps(group["roles"]), (body.get("openCloudKey") or "").strip() or None, session["user_id"], now()),
            )
            conn.execute("INSERT OR IGNORE INTO settings (group_id, brand, accent, week_start, idle_seconds, shout) VALUES (?,?,?,?,?,?)", (gid, group["name"], "#d6f25c", 1, 90, ""))
            write_log(conn, gid, "group", session["username"], f"Added group {group['name']}")
        return self._send(200, {"group": group})

    def _refresh_group(self, gid):
        if not self._session():
            return self._send(401, {"error": "login required"})
        group, err = fetch_group(gid)
        if not group:
            return self._send(404, {"error": "group not found", "detail": err})
        with LOCK, db() as conn:
            conn.execute("UPDATE groups SET name=?, description=?, member_count=?, owner_name=?, owner_id=?, roles_json=? WHERE id=?", (group["name"], group["description"], group["memberCount"], group["ownerName"], group["ownerId"], json.dumps(group["roles"]), gid))
        return self._send(200, {"group": group})

    def _set_cloud(self, gid, body, actor):
        key = (body.get("openCloudKey") or "").strip()
        with LOCK, db() as conn:
            cur = conn.execute("UPDATE groups SET open_cloud_key=? WHERE id=?", (key or None, gid))
            if cur.rowcount == 0:
                return self._send(404, {"error": "group not added"})
            write_log(conn, gid, "ranking", actor, "Updated Open Cloud key" if key else "Cleared Open Cloud key")
        return self._send(200, {"ok": True, "hasOpenCloud": bool(key)})

    def _add_webhook(self, gid, body, actor):
        url = (body.get("url") or "").strip()
        if not url.startswith("https://"):
            return self._send(400, {"error": "webhook url must be https"})
        wid = secrets.token_hex(6)
        events = body.get("events") or ["rank.changed", "chat.message", "presence.updated", "punishment.issued", "session.claimed", "application.reviewed"]
        with LOCK, db() as conn:
            conn.execute("INSERT INTO webhooks (id, group_id, name, url, events_json, enabled, created_at) VALUES (?,?,?,?,?,1,?)", (wid, gid, (body.get("name") or "Discord").strip(), url, json.dumps(events), now()))
            write_log(conn, gid, "webhook", actor, f"Added webhook {body.get('name') or 'Discord'}")
        return self._send(200, {"id": wid})

    def _add_key(self, gid, body, actor):
        token = "kst_" + secrets.token_urlsafe(24)
        kid = secrets.token_hex(4)
        scopes = ",".join(body.get("scopes") or ["rank", "exile", "shout", "ingest", "commands"])
        with LOCK, db() as conn:
            conn.execute("INSERT INTO api_keys (id, group_id, name, token, scopes, created_at) VALUES (?,?,?,?,?,?)", (kid, gid, (body.get("name") or "game").strip(), token, scopes, now()))
            write_log(conn, gid, "api", actor, f"Issued key {body.get('name') or 'game'}")
        return self._send(200, {"id": kid, "token": token})

    def _cloud_rank(self, group, uid, role_id):
        if not group["open_cloud_key"]:
            return None, None
        return roblox("PATCH", f"https://apis.roblox.com/cloud/v2/groups/{group['id']}/memberships/{uid}", {"role": f"groups/{group['id']}/roles/{role_id}"}, headers={"x-api-key": group["open_cloud_key"]})

    def _rank(self, body, external):
        if external:
            key = self._key()
            if not key:
                return self._send(401, {"error": "api key required"})
            gid, actor = key["group_id"], f"api:{key['name']}"
        else:
            session = self._session()
            if not session:
                return self._send(401, {"error": "login required"})
            gid, actor = int(body.get("groupId")), session["username"]
        try:
            uid, role_id = int(body.get("userId")), int(body.get("roleId"))
        except (TypeError, ValueError):
            return self._send(400, {"error": "userId and roleId required"})
        with db() as conn:
            group = conn.execute("SELECT * FROM groups WHERE id=?", (gid,)).fetchone()
        if not group:
            return self._send(404, {"error": "group not added"})
        role = next((r for r in json.loads(group["roles_json"] or "[]") if int(r["id"]) == role_id), None)
        if not role:
            return self._send(400, {"error": "role is not on this group — refresh roles"})
        status, detail = self._cloud_rank(group, uid, role_id)
        if status and status >= 400:
            with LOCK, db() as conn:
                write_log(conn, gid, "ranking", actor, f"Rank failed for {uid} → {role['name']}", {"status": status, "body": detail})
            return self._send(502, {"error": "Roblox Open Cloud rejected the rank", "status": status, "detail": detail})
        with LOCK, db() as conn:
            conn.execute(
                """INSERT INTO people (group_id, user_id, username, display_name, role_name, role_rank, status, last_seen)
                   VALUES (?,?,?,?,?,?, 'offline', ?)
                   ON CONFLICT(group_id, user_id) DO UPDATE SET role_name=excluded.role_name, role_rank=excluded.role_rank, username=excluded.username""",
                (gid, uid, body.get("username") or str(uid), body.get("username") or str(uid), role["name"], role["rank"], time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())),
            )
            write_log(conn, gid, "ranking", actor, f"Ranked {body.get('username') or uid} → {role['name']}", {"userId": uid, "roleId": role_id, "openCloud": bool(group["open_cloud_key"])})
        fire_webhooks(gid, "rank.changed", {"message": f"{actor} ranked {body.get('username') or uid} to {role['name']}", "userId": uid, "role": role["name"]})
        return self._send(200, {"ok": True, "appliedOnRoblox": bool(group["open_cloud_key"]), "role": role, "note": None if group["open_cloud_key"] else "Saved in Kestrel. Add an Open Cloud key to push the rank to Roblox."})

    def _exile(self, body, external):
        if external:
            key = self._key()
            if not key:
                return self._send(401, {"error": "api key required"})
            gid, actor = key["group_id"], f"api:{key['name']}"
        else:
            session = self._session()
            if not session:
                return self._send(401, {"error": "login required"})
            gid, actor = int(body.get("groupId")), session["username"]
        uid = int(body.get("userId"))
        with db() as conn:
            group = conn.execute("SELECT * FROM groups WHERE id=?", (gid,)).fetchone()
        if not group:
            return self._send(404, {"error": "group not added"})
        status = None
        if group["open_cloud_key"]:
            status, detail = roblox("DELETE", f"https://apis.roblox.com/cloud/v2/groups/{gid}/memberships/{uid}", headers={"x-api-key": group["open_cloud_key"]})
            if status >= 400:
                return self._send(502, {"error": "Open Cloud exile failed", "status": status, "detail": detail})
        with LOCK, db() as conn:
            write_log(conn, gid, "ranking", actor, f"Exiled {body.get('username') or uid}")
        fire_webhooks(gid, "rank.changed", {"message": f"{actor} exiled {body.get('username') or uid}", "userId": uid})
        return self._send(200, {"ok": True, "appliedOnRoblox": bool(group["open_cloud_key"])})

    def _shout(self, body, external):
        if external:
            key = self._key()
            if not key:
                return self._send(401, {"error": "api key required"})
            gid, actor = key["group_id"], f"api:{key['name']}"
        else:
            session = self._session()
            if not session:
                return self._send(401, {"error": "login required"})
            gid, actor = int(body.get("groupId")), session["username"]
        message = (body.get("message") or "")[:200]
        with LOCK, db() as conn:
            conn.execute("INSERT INTO settings (group_id, brand, accent, week_start, idle_seconds, shout) VALUES (?,?,?,?,?,?) ON CONFLICT(group_id) DO UPDATE SET shout=excluded.shout", (gid, "Kestrel", "#d6f25c", 1, 90, message))
            write_log(conn, gid, "shout", actor, message or "Cleared shout")
        fire_webhooks(gid, "shout.posted", {"message": message, "actor": actor})
        return self._send(200, {"ok": True, "shout": message})

    def _ingest_presence(self, body):
        key = self._key()
        if not key:
            return self._send(401, {"error": "api key required"})
        gid = key["group_id"]
        people = body.get("players") or []
        with LOCK, db() as conn:
            for p in people:
                try:
                    uid = int(p.get("userId"))
                except (TypeError, ValueError):
                    continue
                conn.execute(
                    """INSERT INTO people (group_id, user_id, username, display_name, role_name, role_rank, status, active_min, idle_min, typing_min, messages, sessions, last_seen, place_id, server_id)
                       VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)
                       ON CONFLICT(group_id, user_id) DO UPDATE SET
                         username=excluded.username, display_name=excluded.display_name,
                         role_name=COALESCE(excluded.role_name, people.role_name), role_rank=COALESCE(excluded.role_rank, people.role_rank),
                         status=excluded.status, active_min=people.active_min+excluded.active_min, idle_min=people.idle_min+excluded.idle_min,
                         typing_min=people.typing_min+excluded.typing_min, messages=people.messages+excluded.messages,
                         sessions=MAX(people.sessions, excluded.sessions), last_seen=excluded.last_seen, place_id=excluded.place_id, server_id=excluded.server_id""",
                    (gid, uid, p.get("username") or str(uid), p.get("displayName") or p.get("username") or str(uid), p.get("roleName"), p.get("roleRank"), p.get("status") or "online", int(p.get("activeMin") or 0), int(p.get("idleMin") or 0), int(p.get("typingMin") or 0), int(p.get("messages") or 0), int(p.get("sessions") or 0), p.get("lastSeen") or time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()), p.get("placeId"), p.get("serverId")),
                )
            write_log(conn, gid, "presence", "game", f"Presence batch · {len(people)} players")
        return self._send(200, {"ok": True, "count": len(people)})

    def _ingest_chat(self, body):
        key = self._key()
        if not key:
            return self._send(401, {"error": "api key required"})
        gid = key["group_id"]
        messages = body.get("messages") or ([body] if body.get("message") else [])
        with LOCK, db() as conn:
            for m in messages:
                conn.execute("INSERT INTO chat (group_id, user_id, username, message, channel, place_id, created_at, flagged) VALUES (?,?,?,?,?,?,?,?)", (gid, int(m.get("userId") or 0), m.get("username") or "unknown", (m.get("message") or "")[:500], m.get("channel") or "all", m.get("placeId"), now(), 1 if m.get("flagged") else 0))
            write_log(conn, gid, "chat", "game", f"Chat batch · {len(messages)} lines")
        if messages:
            fire_webhooks(gid, "chat.message", {"message": messages[-1].get("message", ""), "user": messages[-1].get("username", "")})
        return self._send(200, {"ok": True, "count": len(messages)})

    def _ingest_event(self, body):
        key = self._key()
        if not key:
            return self._send(401, {"error": "api key required"})
        with LOCK, db() as conn:
            conn.execute("INSERT INTO events (group_id, user_id, username, event_type, data_json, created_at) VALUES (?,?,?,?,?,?)", (key["group_id"], int(body.get("userId") or 0), body.get("username") or "server", body.get("eventType") or "event", json.dumps(body.get("data") or {}), now()))
            write_log(conn, key["group_id"], "event", body.get("username") or "game", f"{body.get('eventType') or 'event'}")
        return self._send(200, {"ok": True})

    def _ingest_ticket(self, body):
        key = self._key()
        if not key:
            return self._send(401, {"error": "api key required"})
        tid = secrets.token_hex(5)
        with LOCK, db() as conn:
            conn.execute("INSERT INTO tickets (id, group_id, user_id, username, message, status, reply, created_at) VALUES (?,?,?,?,?,'open','',?)", (tid, key["group_id"], int(body.get("userId") or 0), body.get("username") or "player", (body.get("message") or "")[:500], now()))
            write_log(conn, key["group_id"], "ticket", body.get("username") or "player", body.get("message") or "ticket")
        fire_webhooks(key["group_id"], "ticket.opened", {"message": body.get("message") or "", "user": body.get("username") or ""})
        return self._send(200, {"id": tid})

    def _poll(self, qs):
        key = self._key()
        if not key:
            return self._send(401, {"error": "api key required"})
        gid = key["group_id"]
        with LOCK, db() as conn:
            rows = conn.execute("SELECT * FROM commands WHERE group_id=? AND status='pending' ORDER BY created_at ASC LIMIT 20", (gid,)).fetchall()
            for r in rows:
                conn.execute("UPDATE commands SET status='sent' WHERE id=?", (r["id"],))
            bans = conn.execute("SELECT * FROM bans WHERE group_id=? AND (expires_at IS NULL OR expires_at=0 OR expires_at>?)", (gid, now())).fetchall()
        return self._send(200, {
            "commands": [{"id": r["id"], "kind": r["kind"], "userId": r["user_id"], "payload": json.loads(r["payload"] or "{}")} for r in rows],
            "bans": [{"userId": b["user_id"], "reason": b["reason"]} for b in bans],
        })

    def _add_shift(self, gid, body, actor):
        sid = secrets.token_hex(5)
        with LOCK, db() as conn:
            conn.execute("INSERT INTO shifts (id, group_id, title, place_id, host, starts_at, ends_at, slots, notes, created_at) VALUES (?,?,?,?,?,?,?,?,?,?)", (sid, gid, body.get("title") or "Session", int(body.get("placeId") or 0), body.get("host") or actor, int(body.get("startsAt") or now()), int(body.get("endsAt") or now() + 3600), int(body.get("slots") or 8), body.get("notes") or "", now()))
            write_log(conn, gid, "session", actor, f"Scheduled {body.get('title') or 'Session'}")
        return self._send(200, {"id": sid})

    def _claim(self, gid, body, actor):
        cid = secrets.token_hex(4)
        with LOCK, db() as conn:
            conn.execute("INSERT INTO claims (id, shift_id, group_id, user_id, username, role_name, created_at) VALUES (?,?,?,?,?,?,?)", (cid, body.get("shiftId"), gid, int(body.get("userId") or 0), body.get("username") or actor, body.get("roleName") or "Staff", now()))
            conn.execute("UPDATE people SET sessions=sessions+1 WHERE group_id=? AND user_id=?", (gid, int(body.get("userId") or 0)))
            write_log(conn, gid, "session", actor, f"Claimed shift {body.get('shiftId')}")
        fire_webhooks(gid, "session.claimed", {"message": f"{actor} claimed a session", "shift": body.get("shiftId")})
        return self._send(200, {"id": cid})

    def _add_form(self, gid, body, actor):
        fid, slug = secrets.token_hex(4), (body.get("slug") or secrets.token_hex(3)).strip()
        with LOCK, db() as conn:
            conn.execute("INSERT INTO forms (id, group_id, slug, name, description, min_rank, quiz, pass_percent, single, created_at) VALUES (?,?,?,?,?,?,?,?,?,?)", (fid, gid, slug, body.get("name") or "Application", body.get("description") or "", int(body.get("minRank") or 0), 1 if body.get("quiz") else 0, int(body.get("passPercent") or 70), 1 if body.get("single", True) else 0, now()))
            write_log(conn, gid, "application", actor, f"Created form {body.get('name') or slug}")
        return self._send(200, {"id": fid, "slug": slug, "url": f"{self._origin()}/apply/{slug}"})

    def _add_question(self, gid, body):
        qid = secrets.token_hex(4)
        with LOCK, db() as conn:
            form = conn.execute("SELECT * FROM forms WHERE id=? AND group_id=?", (body.get("formId"), gid)).fetchone()
            if not form:
                return self._send(404, {"error": "form not found"})
            conn.execute("INSERT INTO questions (id, form_id, prompt, kind, options_json, answer, position) VALUES (?,?,?,?,?,?,?)", (qid, form["id"], body.get("prompt") or "Question", body.get("kind") or "text", json.dumps(body.get("options") or []), body.get("answer") or "", int(body.get("position") or 0)))
        return self._send(200, {"id": qid})

    def _review(self, gid, body, actor):
        status = body.get("status") or "pending"
        with LOCK, db() as conn:
            app = conn.execute("SELECT * FROM applications WHERE id=? AND group_id=?", (body.get("id"), gid)).fetchone()
            if not app:
                return self._send(404, {"error": "application not found"})
            conn.execute("UPDATE applications SET status=?, review_note=? WHERE id=?", (status, body.get("note") or "", app["id"]))
            write_log(conn, gid, "application", actor, f"{status} application from {app['username']}")
            if status == "pass" and body.get("roleId"):
                pass
        if status == "pass" and body.get("roleId"):
            self._rank({"groupId": gid, "userId": app["user_id"], "roleId": body.get("roleId"), "username": app["username"]}, external=False)
        fire_webhooks(gid, "application.reviewed", {"message": f"{app['username']} marked {status}", "status": status})
        return self._send(200, {"ok": True})

    def _punish(self, gid, body, actor):
        pid = secrets.token_hex(4)
        kind = body.get("kind") or "warning"
        with LOCK, db() as conn:
            conn.execute("INSERT INTO punishments (id, group_id, user_id, username, kind, reason, expires_at, actor, role_id, created_at) VALUES (?,?,?,?,?,?,?,?,?,?)", (pid, gid, int(body.get("userId") or 0), body.get("username") or "", kind, body.get("reason") or "", int(body.get("expiresAt") or 0), actor, int(body.get("roleId") or 0), now()))
            write_log(conn, gid, "punishment", actor, f"{kind} for {body.get('username') or body.get('userId')}: {body.get('reason') or ''}")
            if kind == "ban":
                conn.execute("INSERT INTO bans (id, group_id, user_id, username, reason, expires_at, created_at) VALUES (?,?,?,?,?,?,?)", (pid, gid, int(body.get("userId") or 0), body.get("username") or "", body.get("reason") or "", int(body.get("expiresAt") or 0), now()))
        if body.get("roleId"):
            self._rank({"groupId": gid, "userId": body.get("userId"), "roleId": body.get("roleId"), "username": body.get("username")}, external=False)
        fire_webhooks(gid, "punishment.issued", {"message": f"{kind}: {body.get('reason') or ''}", "user": body.get("username") or body.get("userId")})
        return self._send(200, {"id": pid})

    def _timeoff(self, gid, body, actor):
        if body.get("id") and body.get("status"):
            with LOCK, db() as conn:
                conn.execute("UPDATE timeoff SET status=? WHERE id=? AND group_id=?", (body["status"], body["id"], gid))
                write_log(conn, gid, "timeoff", actor, f"{body['status']} time off {body['id']}")
            return self._send(200, {"ok": True})
        tid = secrets.token_hex(4)
        with LOCK, db() as conn:
            conn.execute("INSERT INTO timeoff (id, group_id, user_id, username, starts_on, ends_on, reason, status, created_at) VALUES (?,?,?,?,?,?,?,'pending',?)", (tid, gid, int(body.get("userId") or 0), body.get("username") or actor, body.get("startsOn") or "", body.get("endsOn") or "", body.get("reason") or "", now()))
            write_log(conn, gid, "timeoff", actor, f"Time off request {body.get('startsOn')} → {body.get('endsOn')}")
        return self._send(200, {"id": tid})

    def _add_doc(self, gid, body, actor):
        did = body.get("id") or secrets.token_hex(4)
        with LOCK, db() as conn:
            conn.execute("INSERT INTO docs (id, group_id, title, body, department, updated_at) VALUES (?,?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET title=excluded.title, body=excluded.body, department=excluded.department, updated_at=excluded.updated_at", (did, gid, body.get("title") or "Untitled", body.get("body") or "", body.get("department") or "All", now()))
            write_log(conn, gid, "knowledge", actor, f"Saved doc {body.get('title') or did}")
        return self._send(200, {"id": did})

    def _goal(self, gid, body):
        with LOCK, db() as conn:
            conn.execute("INSERT INTO goals (group_id, role_name, weekly_minutes, weekly_sessions) VALUES (?,?,?,?) ON CONFLICT(group_id, role_name) DO UPDATE SET weekly_minutes=excluded.weekly_minutes, weekly_sessions=excluded.weekly_sessions", (gid, body.get("roleName") or "Staff", int(body.get("weeklyMinutes") or 60), int(body.get("weeklySessions") or 1)))
        return self._send(200, {"ok": True})

    def _note(self, gid, body, actor):
        nid = secrets.token_hex(4)
        with LOCK, db() as conn:
            conn.execute("INSERT INTO notes (id, group_id, user_id, body, actor, created_at) VALUES (?,?,?,?,?,?)", (nid, gid, int(body.get("userId") or 0), body.get("body") or "", actor, now()))
        return self._send(200, {"id": nid})

    def _team(self, gid, body):
        tid = secrets.token_hex(4)
        with LOCK, db() as conn:
            conn.execute("INSERT INTO teams (id, group_id, name, rank_min, permissions_json) VALUES (?,?,?,?,?)", (tid, gid, body.get("name") or "Team", int(body.get("rankMin") or 1), json.dumps(body.get("permissions") or ["logs", "people"])))
        return self._send(200, {"id": tid})

    def _ticket_reply(self, gid, body, actor):
        with LOCK, db() as conn:
            conn.execute("UPDATE tickets SET status=?, reply=? WHERE id=? AND group_id=?", (body.get("status") or "closed", body.get("reply") or "", body.get("id"), gid))
            write_log(conn, gid, "ticket", actor, f"Updated ticket {body.get('id')}")
        return self._send(200, {"ok": True})

    def _command(self, gid, body, actor):
        cid = secrets.token_hex(5)
        kind = body.get("kind") or "notify"
        with LOCK, db() as conn:
            conn.execute("INSERT INTO commands (id, group_id, kind, user_id, payload, status, created_at) VALUES (?,?,?,?,?,'pending',?)", (cid, gid, kind, int(body.get("userId") or 0), json.dumps({"message": body.get("message") or "", "reason": body.get("reason") or ""}), now()))
            if kind == "ban":
                conn.execute("INSERT INTO bans (id, group_id, user_id, username, reason, expires_at, created_at) VALUES (?,?,?,?,?,?,?)", (cid, gid, int(body.get("userId") or 0), body.get("username") or "", body.get("reason") or "", int(body.get("expiresAt") or 0), now()))
            write_log(conn, gid, "remote", actor, f"{kind} queued for {body.get('userId')}")
        fire_webhooks(gid, "remote.command", {"message": f"{actor} queued {kind}", "userId": body.get("userId")})
        return self._send(200, {"id": cid})

    def _settings(self, gid, body, actor):
        with LOCK, db() as conn:
            conn.execute("INSERT INTO settings (group_id, brand, accent, week_start, idle_seconds, shout) VALUES (?,?,?,?,?,?) ON CONFLICT(group_id) DO UPDATE SET brand=excluded.brand, accent=excluded.accent, week_start=excluded.week_start, idle_seconds=excluded.idle_seconds", (gid, body.get("brand") or "Kestrel", body.get("accent") or "#d6f25c", int(body.get("weekStart") or 1), int(body.get("idleSeconds") or 90), body.get("shout") or ""))
            write_log(conn, gid, "settings", actor, "Updated workspace settings")
        return self._send(200, {"ok": True})

    def _birthday(self, gid, body):
        with LOCK, db() as conn:
            conn.execute("UPDATE people SET birthday=?, department=? WHERE group_id=? AND user_id=?", (body.get("birthday") or "", body.get("department") or "", gid, int(body.get("userId") or 0)))
        return self._send(200, {"ok": True})

    def _apply_page(self, slug):
        with db() as conn:
            form = conn.execute("SELECT * FROM forms WHERE slug=?", (slug,)).fetchone()
            if not form:
                return self._send(404, b"Application not found", "text/html")
            questions = conn.execute("SELECT * FROM questions WHERE form_id=? ORDER BY position", (form["id"],)).fetchall()
        fields = []
        for q in questions:
            opts = json.loads(q["options_json"] or "[]")
            if q["kind"] == "choice" and opts:
                controls = "".join(f"<label><input type='radio' name='q_{q['id']}' value='{urllib.parse.quote(o)}'> {o}</label>" for o in opts)
            else:
                controls = f"<input name='q_{q['id']}' />"
            fields.append(f"<label>{q['prompt']}</label>{controls}")
        html = f"""<!doctype html><meta charset="utf-8"><title>{form['name']}</title>
        <body style="font-family:Georgia,serif;background:#0e0f0c;color:#f3f0e6;max-width:640px;margin:40px auto;padding:20px">
        <h1>{form['name']}</h1><p>{form['description'] or ''}</p>
        <form method="post" action="/apply/{slug}">
        <label>Roblox username</label><input name="username" required />
        {''.join(fields)}
        <button style="margin-top:16px;padding:10px 14px;background:#d6f25c;border:0">Submit</button>
        </form></body>"""
        return self._send(200, html.encode(), "text/html; charset=utf-8")

    def _apply_submit(self, slug, body, html=False):
        with db() as conn:
            form = conn.execute("SELECT * FROM forms WHERE slug=?", (slug,)).fetchone()
            if not form:
                return self._send(404, {"error": "form not found"})
            questions = conn.execute("SELECT * FROM questions WHERE form_id=?", (form["id"],)).fetchall()
        username = (body.get("username") or "").strip()
        user = lookup_username(username) if username else None
        answers = {q["id"]: body.get(f"q_{q['id']}") or body.get(q["id"]) or "" for q in questions}
        graded = [q for q in questions if q["answer"]]
        score = round(100 * sum(1 for q in graded if str(answers.get(q["id"], "")).strip().lower() == str(q["answer"]).strip().lower()) / len(graded)) if graded else 0
        status = "pending"
        if form["quiz"] and graded:
            status = "pass" if score >= (form["pass_percent"] or 70) else "fail"
        aid = secrets.token_hex(5)
        with LOCK, db() as conn:
            conn.execute("INSERT INTO applications (id, form_id, group_id, user_id, username, answers_json, score, status, review_note, created_at) VALUES (?,?,?,?,?,?,?,?,?,?)", (aid, form["id"], form["group_id"], user["id"] if user else 0, user["username"] if user else username, json.dumps(answers), score, status, "", now()))
            write_log(conn, form["group_id"], "application", username or "applicant", f"Submitted {form['name']} · {status} · {score}%")
        fire_webhooks(form["group_id"], "application.submitted", {"message": f"{username} submitted {form['name']}", "score": score, "status": status})
        if html:
            return self._send(200, f"<body style='font-family:Georgia,serif;background:#0e0f0c;color:#f3f0e6'><h1>Received</h1><p>Status: {status}. Score: {score}%.</p></body>".encode(), "text/html")
        return self._send(200, {"id": aid, "status": status, "score": score})


def main():
    init_db()
    print(f"Kestrel listening on {HOST}:{PORT}  data={DB_PATH}")
    ThreadingHTTPServer((HOST, PORT), Handler).serve_forever()


if __name__ == "__main__":
    main()
