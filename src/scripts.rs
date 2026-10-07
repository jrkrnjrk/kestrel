pub fn loader(base: &str, key: &str, group_id: i64, idle: i64) -> String {
    format!(r#"-- Kestrel loader. ServerScriptService.
-- Game Settings > Security > Allow HTTP Requests = On
local HttpService = game:GetService("HttpService")
local Players = game:GetService("Players")
local TextChatService = game:GetService("TextChatService")
local ReplicatedStorage = game:GetService("ReplicatedStorage")

local API = "{base}"
local KEY = "{key}"
local GROUP_ID = {group_id}
local IDLE_SECONDS = {idle}
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
	if not ok or not res.Success then return nil end
	local decoded = nil
	pcall(function() decoded = HttpService:JSONDecode(res.Body) end)
	return decoded
end

local function ensure(player)
	local row = state[player.UserId]
	if row then return row end
	row = {{ username = player.Name, displayName = player.DisplayName, pos = nil, moved = os.clock(), focused = true, typingOn = false, active = 0, idle = 0, typingMin = 0, messages = 0 }}
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
	request("POST", "/api/ingest/chat", {{ userId = player.UserId, username = player.Name, message = message, channel = channel or "all", placeId = game.PlaceId }})
end

Players.PlayerAdded:Connect(function(player)
	ensure(player)
	player.Chatted:Connect(function(message) onChat(player, message, "classic") end)
end)
for _, player in Players:GetPlayers() do ensure(player) end

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
			local root = player.Character and player.Character:FindFirstChild("HumanoidRootPart")
			if root then
				if row.pos and (root.Position - row.pos).Magnitude > 1.5 then row.moved = os.clock() end
				row.pos = root.Position
			end
			local idle = (not row.focused) or (os.clock() - row.moved) > IDLE_SECONDS
			local slice = FLUSH / 60
			if row.typingOn then row.typingMin += slice elseif idle then row.idle += slice else row.active += slice end
			table.insert(batch, {{
				userId = player.UserId, username = player.Name, displayName = player.DisplayName,
				status = idle and "idle" or "online",
				activeMin = math.floor(row.active + 0.5), idleMin = math.floor(row.idle + 0.5), typingMin = math.floor(row.typingMin + 0.5),
				messages = row.messages, sessions = 1, placeId = game.PlaceId, serverId = game.JobId,
			}})
			row.active, row.idle, row.typingMin, row.messages = 0, 0, 0, 0
		end
		if #batch > 0 then request("POST", "/api/ingest/presence", {{ players = batch }}) end
		task.wait(FLUSH)
	end
end)

local Kestrel = {{}}
function Kestrel.track(player, eventType, data)
	request("POST", "/api/ingest/event", {{ userId = player and player.UserId or 0, username = player and player.Name or "server", eventType = eventType, data = data or {{}} }})
end
function Kestrel.rank(userId, roleId)
	return request("POST", "/api/v1/rank", {{ userId = userId, roleId = roleId }})
end
function Kestrel.exile(userId)
	return request("POST", "/api/v1/exile", {{ userId = userId }})
end
_G.Kestrel = Kestrel
print("[Kestrel] loader online for group", GROUP_ID)
"#)
}

pub fn client() -> &'static str {
    r#"-- Kestrel client. StarterPlayer > StarterPlayerScripts.
local ReplicatedStorage = game:GetService("ReplicatedStorage")
local UserInputService = game:GetService("UserInputService")
local signal = ReplicatedStorage:WaitForChild("KestrelSignal")

UserInputService.WindowFocused:Connect(function() signal:FireServer("focus", true) end)
UserInputService.WindowFocusReleased:Connect(function() signal:FireServer("focus", false) end)
signal:FireServer("focus", UserInputService.WindowFocused)
UserInputService.TextBoxFocused:Connect(function() signal:FireServer("typing", true) end)
UserInputService.TextBoxFocusReleased:Connect(function() signal:FireServer("typing", false) end)

signal.OnClientEvent:Connect(function(kind, message)
	if kind == "notify" then
		pcall(function()
			game:GetService("StarterGui"):SetCore("ChatMakeSystemMessage", { Text = "[Staff] " .. tostring(message), Color = Color3.fromRGB(231, 255, 106) })
		end)
	end
end)
"#
}
