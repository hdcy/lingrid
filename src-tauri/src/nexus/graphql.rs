use super::model::NexusModSummary;
use crate::error::AppError;
use serde::Deserialize;
use std::time::Duration;

/// Nexus Mods' GraphQL endpoint. It answers anonymous queries, which is what the mod
/// browser uses: only downloading needs the user's own API key.
const ENDPOINT: &str = "https://api.nexusmods.com/v2/graphql";

/// Stardew Valley's game domain on Nexus Mods.
pub const GAME: &str = "stardewvalley";

/// Nexus' API terms require headers that identify the application.
const APPLICATION_NAME: &str = "Lingrid";
const APPLICATION_VERSION: &str = env!("CARGO_PKG_VERSION");

const BROWSE_QUERY: &str = "query Browse($game: String!, $count: Int!) { \
     mods(filter: { gameDomainName: [{ value: $game }] }, count: $count) { nodes { modId name summary version downloads author } } }";

const SEARCH_QUERY: &str = "query Search($game: String!, $name: String!, $count: Int!) { \
     mods(filter: { gameDomainName: [{ value: $game }], name: [{ value: $name, op: WILDCARD }] }, count: $count) { nodes { modId name summary version downloads author } } }";

/// Stardew Valley's numeric game id. Looking a mod up by its id needs this instead of the
/// domain, and the id itself has to be sent as a string.
const GAME_ID: &str = "1303";

const BY_ID_QUERY: &str = "query ById($game: String!, $id: String!, $count: Int!) { \
     mods(filter: { gameId: [{ value: $game }], modId: [{ value: $id }] }, count: $count) { nodes { modId name summary version downloads author } } }";

/// A bare number is a mod id — the form people use when sharing mods — and anything else
/// is treated as a name to search for.
pub(super) fn is_mod_id(query: &str) -> bool {
    !query.is_empty() && query.chars().all(|c| c.is_ascii_digit())
}

#[derive(Deserialize)]
struct Response {
    data: Option<Data>,
    errors: Option<Vec<GraphQlError>>,
}

#[derive(Deserialize)]
struct GraphQlError {
    message: String,
}

#[derive(Deserialize)]
struct Data {
    mods: Mods,
}

#[derive(Deserialize)]
struct Mods {
    nodes: Vec<Node>,
}

#[derive(Deserialize)]
struct Node {
    #[serde(rename = "modId")]
    mod_id: u64,
    name: String,
    summary: Option<String>,
    version: Option<String>,
    downloads: Option<u64>,
    author: Option<String>,
}

/// Search Nexus Mods. An empty query browses the newest mods, and a bare number looks up
/// that mod id instead of searching names.
pub fn search(query: &str, count: u32) -> Result<Vec<NexusModSummary>, AppError> {
    let query = query.trim();
    let (document, variables) = if query.is_empty() {
        (
            BROWSE_QUERY,
            serde_json::json!({ "game": GAME, "count": count }),
        )
    } else if is_mod_id(query) {
        (
            BY_ID_QUERY,
            serde_json::json!({ "game": GAME_ID, "id": query, "count": count }),
        )
    } else {
        (
            SEARCH_QUERY,
            serde_json::json!({ "game": GAME, "name": query, "count": count }),
        )
    };
    let body = serde_json::json!({ "query": document, "variables": variables });
    let client = reqwest::blocking::Client::builder()
        .user_agent(format!("{APPLICATION_NAME}/{APPLICATION_VERSION}"))
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| AppError::with_detail("nexusUnavailable", e))?;
    let response = client
        .post(ENDPOINT)
        .header("Application-Name", APPLICATION_NAME)
        .header("Application-Version", APPLICATION_VERSION)
        .json(&body)
        .send()
        .and_then(|response| response.error_for_status())
        .map_err(|e| AppError::with_detail("nexusUnavailable", e))?;
    let text = response
        .text()
        .map_err(|e| AppError::with_detail("nexusUnavailable", e))?;
    parse(&text)
}

/// Turn a GraphQL response into summaries; split out so it is testable offline.
pub fn parse(body: &str) -> Result<Vec<NexusModSummary>, AppError> {
    let response: Response =
        serde_json::from_str(body).map_err(|e| AppError::with_detail("nexusQueryFailed", e))?;
    if let Some(errors) = response.errors {
        if let Some(first) = errors.into_iter().next() {
            return Err(AppError::with_detail("nexusQueryFailed", first.message));
        }
    }
    let nodes = response
        .data
        .ok_or_else(|| AppError::with_detail("nexusQueryFailed", "empty response"))?
        .mods
        .nodes;
    Ok(nodes
        .into_iter()
        .map(|node| NexusModSummary {
            url: format!("https://www.nexusmods.com/{GAME}/mods/{}", node.mod_id),
            mod_id: node.mod_id,
            name: node.name,
            summary: node.summary,
            version: node.version,
            downloads: node.downloads,
            author: node.author,
        })
        .collect())
}
