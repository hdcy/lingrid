use super::model::SmapiRelease;
use crate::error::AppError;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    time::Duration,
};

const RELEASES_URL: &str = "https://api.github.com/repos/Pathoschild/SMAPI/releases/latest";
const USER_AGENT: &str = "Lingrid";

#[derive(Deserialize)]
struct ApiRelease {
    tag_name: String,
    assets: Vec<ApiAsset>,
}

#[derive(Deserialize)]
struct ApiAsset {
    name: String,
    browser_download_url: String,
    size: u64,
    digest: Option<String>,
}

/// Parse the GitHub releases payload. Split out so the contract is testable offline.
pub fn parse_release(body: &str) -> Result<SmapiRelease, AppError> {
    let release: ApiRelease =
        serde_json::from_str(body).map_err(|e| AppError::with_detail("smapiReleaseInvalid", e))?;
    let asset = release
        .assets
        .iter()
        .find(|asset| {
            asset.name.ends_with("-installer.zip") && !asset.name.contains("double-zipped")
        })
        .ok_or_else(|| {
            AppError::with_detail(
                "smapiReleaseInvalid",
                format!("no installer asset in release {}", release.tag_name),
            )
        })?;
    if release.tag_name.trim().is_empty() {
        return Err(AppError::new("smapiReleaseInvalid"));
    }
    Ok(SmapiRelease {
        version: release.tag_name,
        asset_name: asset.name.clone(),
        asset_url: asset.browser_download_url.clone(),
        sha256: asset
            .digest
            .as_deref()
            .and_then(|digest| digest.strip_prefix("sha256:"))
            .map(|hash| hash.to_ascii_lowercase()),
        size: asset.size,
    })
}

pub fn fetch_latest() -> Result<SmapiRelease, AppError> {
    let client = client(Duration::from_secs(30))?;
    let body = client
        .get(RELEASES_URL)
        .send()
        .and_then(|response| response.error_for_status())
        .and_then(|response| response.text())
        .map_err(|e| AppError::with_detail("smapiReleaseUnavailable", e))?;
    parse_release(&body)
}

/// Path the release archive is cached at, whether or not anything is there yet.
pub fn cached_path(cache_dir: &Path, release: &SmapiRelease) -> PathBuf {
    cache_dir.join(&release.asset_name)
}

/// Download the release archive into the cache.
///
/// `on_progress` receives the bytes written so far and, when the server reports it, the
/// total size. The file is written beside the target and moved into place afterwards, so
/// an interrupted download never looks like a cached archive.
pub fn download(
    cache_dir: &Path,
    release: &SmapiRelease,
    mut on_progress: impl FnMut(u64, Option<u64>),
) -> Result<PathBuf, AppError> {
    fs::create_dir_all(cache_dir).map_err(|e| AppError::with_detail("smapiDownloadFailed", e))?;
    let target = cached_path(cache_dir, release);
    let partial = cache_dir.join(format!("{}.partial", release.asset_name));
    match stream(&release.asset_url, &partial, &mut on_progress) {
        Ok(()) => {
            fs::rename(&partial, &target)
                .map_err(|e| AppError::with_detail("smapiDownloadFailed", e))?;
            Ok(target)
        }
        Err(error) => {
            let _ = fs::remove_file(&partial);
            Err(error)
        }
    }
}

/// Verify an archive against the SHA-256 published by GitHub. A release without a
/// published digest is accepted (the API started reporting digests during 4.x).
pub fn verify(path: &Path, expected: Option<&str>) -> Result<(), AppError> {
    let Some(expected) = expected else {
        return Ok(());
    };
    let actual = hash_file(path)?;
    if actual == expected {
        Ok(())
    } else {
        Err(AppError::with_detail(
            "smapiChecksumMismatch",
            format!("expected {expected}, got {actual}"),
        ))
    }
}

fn stream(
    url: &str,
    dest: &Path,
    on_progress: &mut impl FnMut(u64, Option<u64>),
) -> Result<(), AppError> {
    let client = client(Duration::from_secs(600))?;
    let mut response = client
        .get(url)
        .send()
        .and_then(|response| response.error_for_status())
        .map_err(|e| AppError::with_detail("smapiDownloadFailed", e))?;
    let total = response.content_length();
    let mut file =
        fs::File::create(dest).map_err(|e| AppError::with_detail("smapiDownloadFailed", e))?;
    let mut buffer = [0u8; 64 * 1024];
    let mut downloaded = 0u64;
    on_progress(0, total);
    loop {
        let read = response
            .read(&mut buffer)
            .map_err(|e| AppError::with_detail("smapiDownloadFailed", e))?;
        if read == 0 {
            break;
        }
        file.write_all(&buffer[..read])
            .map_err(|e| AppError::with_detail("smapiDownloadFailed", e))?;
        downloaded += read as u64;
        on_progress(downloaded, total);
    }
    file.flush()
        .map_err(|e| AppError::with_detail("smapiDownloadFailed", e))?;
    Ok(())
}

fn hash_file(path: &Path) -> Result<String, AppError> {
    let mut file =
        fs::File::open(path).map_err(|e| AppError::with_detail("smapiArchiveUnreadable", e))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|e| AppError::with_detail("smapiArchiveUnreadable", e))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex(hasher.finalize()))
}

fn hex(digest: impl AsRef<[u8]>) -> String {
    digest
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn client(timeout: Duration) -> Result<reqwest::blocking::Client, AppError> {
    reqwest::blocking::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(timeout)
        .build()
        .map_err(|e| AppError::with_detail("smapiUnavailable", e))
}
