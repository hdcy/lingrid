use super::graphql::GAME;
use crate::error::AppError;
use serde::Deserialize;
use std::{fs, io::Write, path::Path, time::Duration};

const APPLICATION_NAME: &str = "Lingrid";
const APPLICATION_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Refuse mod archives larger than this; Nexus' biggest Stardew mods are well under it.
const MAX_DOWNLOAD_BYTES: u64 = 4 * 1024 * 1024 * 1024;

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
    if mod_id == 0 || file_id == 0 {
        return Err(AppError::new("nxmInvalid"));
    }

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
            // The key goes into the download URL, so only the shape Nexus uses is accepted;
            // anything else would let a link smuggle extra query parameters into the request.
            "key" => {
                let value = percent_decode(value).ok_or_else(|| AppError::new("nxmInvalid"))?;
                if !is_valid_key(&value) {
                    return Err(AppError::with_detail("nxmInvalid", "download key"));
                }
                link.key = Some(value);
            }
            "expires" => {
                let value = value.parse().map_err(|_| AppError::new("nxmInvalid"))?;
                if value <= 0 {
                    return Err(AppError::with_detail("nxmInvalid", "download expiry"));
                }
                link.expires = Some(value);
            }
            _ => {}
        }
    }
    // A free-account link always carries both; a key without an expiry would be sent alone
    // and rejected by the API with a far less helpful message.
    if link.key.is_some() && link.expires.is_none() {
        return Err(AppError::with_detail(
            "nxmInvalid",
            "download key without expiry",
        ));
    }
    Ok(link)
}

/// Nexus download keys are hexadecimal, so a conservative allowlist is enough.
fn is_valid_key(key: &str) -> bool {
    !key.is_empty() && key.len() <= 128 && key.bytes().all(|byte| byte.is_ascii_alphanumeric())
}

/// Decode `%XX` escapes; links copied from a browser may arrive encoded.
fn percent_decode(value: &str) -> Option<String> {
    let bytes = value.as_bytes();
    let mut out = String::with_capacity(value.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'%' => {
                let high = *bytes.get(index + 1)?;
                let low = *bytes.get(index + 2)?;
                let byte = (hex_value(high)? << 4) | hex_value(low)?;
                if !byte.is_ascii() {
                    return None;
                }
                out.push(byte as char);
                index += 3;
            }
            byte if byte.is_ascii() => {
                out.push(byte as char);
                index += 1;
            }
            _ => return None,
        }
    }
    Some(out)
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
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
///
/// The transfer is bounded and only accepted from Nexus' own hosts: the address comes from
/// the API, but a compromised or misconfigured response must not be able to stream an
/// arbitrary amount of data from an arbitrary server into the game folder.
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
    let final_url = response.url().clone();
    if final_url.scheme() != "https" || !final_url.host_str().is_some_and(is_allowed_host) {
        return Err(AppError::with_detail(
            "nexusDownloadDenied",
            format!("unexpected download host {}", final_url),
        ));
    }
    if response
        .content_length()
        .is_some_and(|length| length > MAX_DOWNLOAD_BYTES)
    {
        return Err(AppError::new("downloadTooLarge"));
    }
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| AppError::with_detail("modInstallFailed", e))?;
    }
    let temporary = dest.with_extension("partial");
    let result = (|| -> Result<(), AppError> {
        let mut file = fs::File::create(&temporary)
            .map_err(|e| AppError::with_detail("modInstallFailed", e))?;
        let mut buffer = [0u8; 64 * 1024];
        let mut downloaded = 0u64;
        loop {
            let read = std::io::Read::read(&mut response, &mut buffer)
                .map_err(|e| AppError::with_detail("nexusUnavailable", e))?;
            if read == 0 {
                break;
            }
            downloaded += read as u64;
            if downloaded > MAX_DOWNLOAD_BYTES {
                return Err(AppError::new("downloadTooLarge"));
            }
            file.write_all(&buffer[..read])
                .map_err(|e| AppError::with_detail("modInstallFailed", e))?;
        }
        file.flush()
            .map_err(|e| AppError::with_detail("modInstallFailed", e))?;
        if !starts_like_zip(&temporary)? {
            return Err(AppError::new("modArchiveInvalid"));
        }
        Ok(())
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

/// Accept only Nexus' download hosts; the API hands out `*.nexusmods.com` and
/// `*.nexus-cdn.com` addresses (and follows a redirect to them).
pub(super) fn is_allowed_host(host: &str) -> bool {
    let host = host.to_ascii_lowercase();
    ["nexusmods.com", "nexus-cdn.com"]
        .iter()
        .any(|domain| host == *domain || host.ends_with(&format!(".{domain}")))
}

/// The archive is handed to the mod installer next, so reject obvious non-zips early.
pub(super) fn starts_like_zip(path: &Path) -> Result<bool, AppError> {
    use std::io::Read;
    let mut header = [0u8; 4];
    let mut file =
        fs::File::open(path).map_err(|e| AppError::with_detail("modArchiveUnreadable", e))?;
    let read = file
        .read(&mut header)
        .map_err(|e| AppError::with_detail("modArchiveUnreadable", e))?;
    Ok(read == 4 && (&header == b"PK\x03\x04" || &header == b"PK\x05\x06"))
}
