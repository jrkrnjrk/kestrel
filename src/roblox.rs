use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
struct NameHit {
    id: i64,
    name: String,
    #[serde(rename = "displayName")]
    display_name: String,
}

#[derive(Deserialize)]
struct NameResp {
    data: Vec<NameHit>,
}

#[derive(Deserialize)]
struct Profile {
    description: Option<String>,
}

pub async fn lookup(username: &str) -> Option<(i64, String, String)> {
    let client = reqwest::Client::new();
    let res = client
        .post("https://users.roblox.com/v1/usernames/users")
        .json(&json!({"usernames": [username], "excludeBannedUsers": false}))
        .send()
        .await
        .ok()?;
    let body: NameResp = res.json().await.ok()?;
    let hit = body.data.into_iter().next()?;
    Some((hit.id, hit.name, hit.display_name))
}

pub async fn bio_has(user_id: i64, code: &str) -> Result<bool, String> {
    let client = reqwest::Client::new();
    let res = client
        .get(format!("https://users.roblox.com/v1/users/{user_id}"))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let profile: Profile = res.json().await.map_err(|e| e.to_string())?;
    let text = profile.description.unwrap_or_default().replace('\u{200b}', "");
    Ok(text.contains(code))
}

pub async fn group_info(id: i64) -> Result<(String, String, i64, String, String), String> {
    let client = reqwest::Client::new();
    let res = client
        .get(format!("https://groups.roblox.com/v1/groups/{id}"))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let v: serde_json::Value = res.json().await.map_err(|e| e.to_string())?;
    if v.get("id").is_none() {
        return Err("group not found".into());
    }
    let roles = client
        .get(format!("https://groups.roblox.com/v1/groups/{id}/roles"))
        .send()
        .await
        .ok();
    let roles_json = if let Some(r) = roles {
        r.json::<serde_json::Value>().await.unwrap_or(json!({})).get("roles").cloned().unwrap_or(json!([])).to_string()
    } else {
        "[]".into()
    };
    let owner = v.get("owner").cloned().unwrap_or(json!({}));
    Ok((
        v.get("name").and_then(|x| x.as_str()).unwrap_or("Group").to_string(),
        v.get("description").and_then(|x| x.as_str()).unwrap_or("").to_string(),
        v.get("memberCount").and_then(|x| x.as_i64()).unwrap_or(0),
        owner.get("username").and_then(|x| x.as_str()).unwrap_or("").to_string(),
        roles_json,
    ))
}

pub async fn rank(key: &str, group_id: i64, user_id: i64, role_id: i64) -> Result<(), String> {
    let client = reqwest::Client::new();
    let res = client
        .patch(format!("https://apis.roblox.com/cloud/v2/groups/{group_id}/memberships/{user_id}"))
        .header("x-api-key", key)
        .json(&json!({"role": format!("groups/{group_id}/roles/{role_id}")}))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if res.status().is_success() {
        Ok(())
    } else {
        Err(res.text().await.unwrap_or_else(|_| "open cloud rejected the rank".into()))
    }
}

pub async fn exile(key: &str, group_id: i64, user_id: i64) -> Result<(), String> {
    let client = reqwest::Client::new();
    let res = client
        .delete(format!("https://apis.roblox.com/cloud/v2/groups/{group_id}/memberships/{user_id}"))
        .header("x-api-key", key)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if res.status().is_success() {
        Ok(())
    } else {
        Err(res.text().await.unwrap_or_else(|_| "open cloud rejected the exile".into()))
    }
}
