use super::graphql::GAME;
use crate::error::AppError;
use serde::Deserialize;
use std::{fs, io::Write, path::Path, time::Duration};

const APPLICATION_NAME: &str = "Lingrid";
const APPLICATION_VERSION: &str = env!("CARGO_PKG_VERSION");

/// A parsed `nxm://` link, as produced by the "Mod Manager Download" button.
///
/// Free Nexus accounts can only obtain a download through these links: the `key` and
/// `expires` values are what the site hands out, and the API refuses the request without
/// them. Premium accounts leave them out.
#[derive(Debug, PartialEq, Eq)]
pub struct NxmLink {
    pub mod_id: u64,
    pub file_id: u64,
    pub key: Option<String>,
    pub expires: Option<i64>,
}

/// `nxm://stardewvalley/mods/2400/files/1234?key=…&expires=…&user_id=…`
pub fn parse(url: &str) -> Result<NxmLink, AppError> {
    let rest = url
        .trim()
        .strip_prefix("nxm://")
        .ok_or_else(|| AppError::new("nxmInvalid"))?;
    let (path, query) = match rest.split_once('?') {
        Some((path, query)) => (path, Some(query)),
        None => (rest, None),
    };
    let parts: Vec<&str> = path.split('/').collect();
    // A link that names only the mod is the "open the mod page" form; it carries no file
    // and no download key, so say that rather than calling it invalid.
    if parts.len() >= 3 && parts[1] == "mods" && parts.len() < 5 {
        return Err(AppError::with_detail("nxmNeedsFile", parts[2]));
    }
    if parts.len() < 5 || parts[1] != "mods" || parts[3] != "files" {
        return Err(AppError::new("nxmInvalid"));
    }
    // Only the game this application manages may be requested, so a crafted link cannot
    // redirect the API call somewhere else.
    if !parts[0].eq_ignore_ascii_case(GAME) {
        return Err(AppError::with_detail("nxmUnsupportedGame", parts[0]));
    }
    let mod_id = parts[2].parse().map_err(|_| AppError::new("nxmInvalid"))?;
    let file_id = parts[4].parse().map_err(|_| AppError::new("nxmInvalid"))?;

    let mut link = NxmLink {
        mod_id,
        file_id,
        key: None,
        expires: None,
    };
    for pair in query.unwrap_or_default().split('&') {
        let Some((name, value)) = pair.split_once('=') else {
            continue;
        };
        match name {
            "key" => link.key = Some(value.to_owned()),
            "expires" => link.expires = value.parse().ok(),
            _ => {}
        }
    }
    Ok(link)
}

#[derive(Deserialize)]
struct DownloadLink {
    #[serde(rename = "URI")]
    uri: String,
}

/// Exchange a link for a temporary download address.
pub fn resolve(link: &NxmLink, api_key: &str) -> Result<String, AppError> {
    let mut url = format!(
        "https://api.nexusmods.com/v1/games/{GAME}/mods/{}/files/{}/download_link.json",
        link.mod_id, link.file_id
    );
    if let (Some(key), Some(expires)) = (&link.key, link.expires) {
        url.push_str(&format!("?key={key}&expires={expires}"));
    }
    let client = reqwest::blocking::Client::builder()
        .user_agent(format!("{APPLICATION_NAME}/{APPLICATION_VERSION}"))
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| AppError::with_detail("nexusUnavailable", e))?;
    let response = client
        .get(&url)
        .header("apikey", api_key)
        .header("Application-Name", APPLICATION_NAME)
        .header("Application-Version", APPLICATION_VERSION)
        .send()
        .map_err(|e| AppError::with_detail("nexusUnavailable", e))?;
    let status = response.status();
    if status == reqwest::StatusCode::FORBIDDEN || status == reqwest::StatusCode::UNAUTHORIZED {
        // Nexus reports both a wrong key and a premium-only request this way.
        return Err(AppError::with_detail("nexusDownloadDenied", status));
    }
    if !status.is_success() {
        return Err(AppError::with_detail("nexusUnavailable", status));
    }
    let links: Vec<DownloadLink> = response
        .json()
        .map_err(|e| AppError::with_detail("nexusDownloadDenied", e))?;
    links
        .into_iter()
        .next()
        .map(|link| link.uri)
        .ok_or_else(|| AppError::with_detail("nexusDownloadDenied", "no download server offered"))
}

/// Download to `dest`, writing to a temporary file first so a failure leaves nothing behind.
pub fn fetch(url: &str, dest: &Path) -> Result<(), AppError> {
    let client = reqwest::blocking::Client::builder()
        .user_agent(format!("{APPLICATION_NAME}/{APPLICATION_VERSION}"))
        .timeout(Duration::from_secs(900))
        .build()
        .map_err(|e| AppError::with_detail("nexusUnavailable", e))?;
    let mut response = client
        .get(url)
        .header("Application-Name", APPLICATION_NAME)
        .header("Application-Version", APPLICATION_VERSION)
        .send()
        .and_then(|response| response.error_for_status())
        .map_err(|e| AppError::with_detail("nexusUnavailable", e))?;
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| AppError::with_detail("modInstallFailed", e))?;
    }
    let temporary = dest.with_extension("partial");
    let result = (|| -> Result<(), AppError> {
        let mut file = fs::File::create(&temporary)
            .map_err(|e| AppError::with_detail("modInstallFailed", e))?;
        let mut buffer = [0u8; 64 * 1024];
        loop {
            let read = std::io::Read::read(&mut response, &mut buffer)
                .map_err(|e| AppError::with_detail("nexusUnavailable", e))?;
            if read == 0 {
                break;
            }
            file.write_all(&buffer[..read])
                .map_err(|e| AppError::with_detail("modInstallFailed", e))?;
        }
        file.flush()
            .map_err(|e| AppError::with_detail("modInstallFailed", e))
    })();
    match result {
        Ok(()) => {
            fs::rename(&temporary, dest).map_err(|e| AppError::with_detail("modInstallFailed", e))
        }
        Err(error) => {
            let _ = fs::remove_file(&temporary);
            Err(error)
        }
    }
}
