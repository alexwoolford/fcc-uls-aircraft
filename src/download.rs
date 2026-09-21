//! Blocking GET of FCC ULS complete aircraft zip (`l_aircr.zip`).
//!
//! Default UA is this crate. Never send `FAA_USER_AGENT` to FCC.

use std::io::{Cursor, Read, Write};
use std::path::Path;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, SERVER};
use zip::ZipArchive;

pub const DEFAULT_ZIP_URL: &str = "https://data.fcc.gov/download/pub/uls/complete/l_aircr.zip";

/// Declared User-Agent for `data.fcc.gov`. Override with `FCC_USER_AGENT`.
pub const DEFAULT_FCC_USER_AGENT: &str =
    "fcc-uls-aircraft/0.1 (+https://github.com/alexwoolford/fcc-uls-aircraft)";

const GET_ATTEMPTS: u32 = 4;
const CACHE_NAME: &str = "l_aircr.zip";

pub fn resolve_user_agent(cli: Option<&str>) -> String {
    if let Some(ua) = cli.map(str::trim).filter(|s| !s.is_empty()) {
        return ua.to_string();
    }
    std::env::var("FCC_USER_AGENT")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| DEFAULT_FCC_USER_AGENT.to_string())
}

fn default_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(ACCEPT, HeaderValue::from_static("*/*"));
    headers
}

fn server_name(resp: &reqwest::blocking::Response) -> String {
    resp.headers()
        .get(SERVER)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown")
        .to_string()
}

pub fn download_zip(url: &str, user_agent: &str) -> Result<Vec<u8>> {
    tracing::info!(url, "downloading FCC ULS aircraft zip");
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(300))
        .user_agent(user_agent)
        .gzip(true)
        .default_headers(default_headers())
        .build()
        .context("build HTTP client")?;

    let mut delay = Duration::from_secs(2);
    let mut last: Option<(reqwest::StatusCode, String)> = None;
    for attempt in 1..=GET_ATTEMPTS {
        let resp = client
            .get(url)
            .send()
            .with_context(|| format!("GET {url}"))?;
        let status = resp.status();
        let server = server_name(&resp);
        if status.as_u16() == 503 && attempt < GET_ATTEMPTS {
            tracing::warn!(attempt, %status, server = %server, url, "origin 503; retrying");
            std::thread::sleep(delay);
            delay *= 2;
            last = Some((status, server));
            continue;
        }
        if !status.is_success() {
            anyhow::bail!("HTTP {status} for {url} (Server: {server})");
        }
        let bytes = resp.bytes().context("read zip body")?;
        tracing::info!(bytes = bytes.len(), "download complete");
        return Ok(bytes.to_vec());
    }
    let (status, server) = last.expect("503 retry always records status");
    anyhow::bail!("HTTP {status} for {url} (Server: {server}) after {GET_ATTEMPTS} attempts")
}

pub fn read_zip_file(path: &Path) -> Result<Vec<u8>> {
    std::fs::read(path).with_context(|| format!("read {}", path.display()))
}

/// Atomically write `l_aircr.zip` under `dir` for later `--zip` reruns.
pub fn write_zip_cache(dir: &Path, bytes: &[u8]) -> Result<()> {
    std::fs::create_dir_all(dir).with_context(|| format!("create {}", dir.display()))?;
    let dest = dir.join(CACHE_NAME);
    let tmp = dir.join(format!("{CACHE_NAME}.tmp"));
    std::fs::write(&tmp, bytes).with_context(|| format!("write {}", tmp.display()))?;
    std::fs::rename(&tmp, &dest).with_context(|| format!("rename {}", dest.display()))?;
    Ok(())
}

pub fn extract_named(zip_bytes: &[u8], filename: &str) -> Result<Vec<u8>> {
    let mut archive = ZipArchive::new(Cursor::new(zip_bytes)).context("open zip archive")?;
    for i in 0..archive.len() {
        let mut file = archive.by_index(i).context("read zip entry")?;
        let name = file.name().replace('\\', "/");
        let base = name.rsplit('/').next().unwrap_or(name.as_str());
        if base.eq_ignore_ascii_case(filename) {
            let mut buf = Vec::new();
            file.read_to_end(&mut buf)
                .with_context(|| format!("extract {filename}"))?;
            return Ok(buf);
        }
    }
    Err(anyhow!("{filename} not found in zip"))
}

pub fn zip_hash(zip_bytes: &[u8]) -> String {
    blake3::hash(zip_bytes).to_hex().to_string()
}

pub fn write_test_zip(files: &[(&str, &[u8])]) -> Result<Vec<u8>> {
    let mut cursor = Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut cursor);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        for (name, data) in files {
            zip.start_file(*name, options)?;
            zip.write_all(data)?;
        }
        zip.finish()?;
    }
    Ok(cursor.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_by_basename_case_insensitive() {
        let zip = write_test_zip(&[("folder/HD.dat", b"hello")]).unwrap();
        let extracted = extract_named(&zip, "hd.dat").unwrap();
        assert_eq!(extracted, b"hello");
    }

    #[test]
    fn resolve_user_agent_prefers_cli_not_faa() {
        assert_eq!(resolve_user_agent(Some("CustomUA/1")), "CustomUA/1");
        assert_eq!(resolve_user_agent(Some("")), DEFAULT_FCC_USER_AGENT);
        assert!(!DEFAULT_FCC_USER_AGENT.contains("Safari"));
        assert!(DEFAULT_FCC_USER_AGENT.contains("fcc-uls-aircraft"));
    }

    #[test]
    fn write_zip_cache_replaces_atomically() {
        let dir = std::env::temp_dir().join(format!(
            "fcc-zip-cache-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        write_zip_cache(&dir, b"first").unwrap();
        write_zip_cache(&dir, b"second").unwrap();
        assert_eq!(std::fs::read(dir.join(CACHE_NAME)).unwrap(), b"second");
        assert!(!dir.join(format!("{CACHE_NAME}.tmp")).exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
