use serde::Serialize;

/// One search result from Nexus Mods.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexusModSummary {
    pub mod_id: u64,
    pub name: String,
    pub summary: Option<String>,
    pub version: Option<String>,
    pub downloads: Option<u64>,
    pub author: Option<String>,
    /// Link to the mod page, built from the game domain and id.
    pub url: String,
}
