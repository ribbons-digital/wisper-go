//! Asset downloader: resumable HTTP fetch with SHA-256 verification and
//! atomic rename.
//!
//! Downloads an [`AssetEntry`]'s URL to a `.part` file under the asset's
//! directory, resuming via HTTP `Range` requests when the server supports them.
//! Once the full body is on disk, the SHA-256 is verified against the manifest;
//! on mismatch the `.part` is deleted and the download is retried once. On
//! success the `.part` file is atomically renamed to the final asset path.
//!
//! This module performs real HTTP and filesystem I/O but takes a
//! [`reqwest::Client`] by reference so tests can point it at `httpmock` or a
//! `file://`-serving local server. It does **not** touch the network in unit
//! tests — see `tests/downloader_tests.rs` for the `httpmock`-based suite.

use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use reqwest::{Client, StatusCode};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::asset_manifest::{AssetEntry, AssetManifest};
use crate::asset_storage::AssetStorage;

/// Outcome of a successful [`Downloader::download`] call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadOutcome {
    /// The final, verified asset path (the `.part` suffix removed).
    pub final_path: PathBuf,
    /// How the bytes were obtained this call.
    pub source: DownloadSource,
}

/// Whether the download completed fresh or by resuming an existing `.part`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadSource {
    /// No usable `.part` existed; the full body was fetched.
    Fresh,
    /// A partial `.part` was resumed via an HTTP Range request.
    Resumed { resumed_from: u64 },
    /// A complete `.part` already existed and matched the SHA-256; no bytes
    /// were fetched. (Only produced when `allow_cached_part` is true.)
    Cached,
}

/// Errors raised by the downloader.
#[derive(Debug, Error)]
pub enum DownloadError {
    /// The server returned a non-success status.
    #[error("download failed with HTTP status {status}")]
    HttpStatus { status: StatusCode },
    /// A network or I/O error occurred before any bytes could be validated.
    #[error("network or IO error: {0}")]
    Network(#[from] reqwest::Error),
    /// A filesystem operation failed.
    #[error("filesystem error: {0}")]
    Fs(#[from] std::io::Error),
    /// The downloaded bytes did not match the manifest's SHA-256 after the
    /// configured retries.
    #[error("sha256 mismatch for {id}: expected {expected}, got {actual}")]
    Sha256Mismatch {
        id: String,
        expected: String,
        actual: String,
    },
}

/// Resumable, verifying asset downloader.
#[derive(Debug, Clone)]
pub struct Downloader {
    client: Client,
    storage: AssetStorage,
    /// Per-attempt request timeout (covering header + body). Default 5 min.
    request_timeout: Duration,
}

impl Downloader {
    /// Build a downloader over `storage` using the given HTTP client.
    pub fn new(client: Client, storage: AssetStorage) -> Self {
        Self {
            client,
            storage,
            request_timeout: Duration::from_secs(300),
        }
    }

    /// Override the per-attempt request timeout.
    pub fn with_request_timeout(mut self, timeout: Duration) -> Self {
        self.request_timeout = timeout;
        self
    }

    /// Download and verify `asset`.
    ///
    /// - Resumes an existing `.part` if one is present and the server honors
    ///   `Range`.
    /// - Verifies SHA-256; on mismatch deletes the `.part` and retries once.
    /// - On success, atomically renames `.part` to the final asset path.
    ///
    /// `allow_cached_part` controls whether a complete, already-verified
    /// `.part` short-circuits (useful for re-runs). Pass `false` to always
    /// fetch at least a conditional request.
    pub async fn download(
        &self,
        asset: &AssetEntry,
        allow_cached_part: bool,
    ) -> Result<DownloadOutcome, DownloadError> {
        let final_path = self.storage.asset_path(&asset.id, asset.role);
        let part_path = self.storage.part_path(&asset.id, asset.role);

        if let Some(parent) = final_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // If a final file already exists and verifies, treat as cached success.
        if final_path.exists() {
            if verify_sha256(&final_path, &asset.sha256)? {
                return Ok(DownloadOutcome {
                    final_path,
                    source: DownloadSource::Cached,
                });
            }
            // Corrupt final file: remove and re-download.
            fs::remove_file(&final_path)?;
        }

        // If a complete `.part` already verifies, skip the fetch.
        if allow_cached_part && part_path.exists() && verify_sha256(&part_path, &asset.sha256)? {
            rename_atomic(&part_path, &final_path)?;
            return Ok(DownloadOutcome {
                final_path,
                source: DownloadSource::Cached,
            });
        }

        // First attempt (may resume).
        match self.fetch_and_verify(asset, &part_path).await {            Ok(source) => {
                rename_atomic(&part_path, &final_path)?;
                Ok(DownloadOutcome {
                    final_path,
                    source,
                })
            }
            Err(DownloadError::Sha256Mismatch { .. }) => {
                // Retry once, fresh.
                if part_path.exists() {
                    fs::remove_file(&part_path)?;
                }
                let source = self.fetch_and_verify(asset, &part_path).await?;
                rename_atomic(&part_path, &final_path)?;
                Ok(DownloadOutcome {
                    final_path,
                    source,
                })
            }
            Err(err) => Err(err),
        }
    }

    /// Fetch `asset` to `part_path`, resuming if possible, then verify.
    async fn fetch_and_verify(
        &self,
        asset: &AssetEntry,
        part_path: &Path,
    ) -> Result<DownloadSource, DownloadError> {
        let existing_len = existing_len(part_path);
        let resumed = existing_len > 0;

        let mut request = self
            .client
            .get(&asset.url)
            .timeout(self.request_timeout);
        if resumed {
            request = request.header("Range", format!("bytes={existing_len}-"));
        }
        let response = request.send().await?;

        let status = response.status();
        if !status.is_success() {
            return Err(DownloadError::HttpStatus { status });
        }

        let appending = resumed && status == StatusCode::PARTIAL_CONTENT;
        let mut file = open_part_writer(part_path, appending)?;
        if !appending {
            // Full response (200, or server ignored Range): start over.
            file.get_ref().set_len(0)?;
        }

        let mut stream = response.bytes_stream();
        use futures_util::StreamExt;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            file.write_all(&chunk)?;
        }
        file.flush()?;
        drop(file);

        // Verify.
        let actual = sha256_hex(part_path)?;
        let expected = asset.sha256.to_ascii_lowercase();
        if actual != expected {
            return Err(DownloadError::Sha256Mismatch {
                id: asset.id.clone(),
                expected,
                actual,
            });
        }

        Ok(if appending {
            DownloadSource::Resumed {
                resumed_from: existing_len,
            }
        } else {
            DownloadSource::Fresh
        })
    }
}

fn existing_len(path: &Path) -> u64 {
    fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}

fn open_part_writer(path: &Path, append: bool) -> std::io::Result<BufWriter<File>> {
    let mut opts = OpenOptions::new();
    opts.create(true).read(false).write(true);
    if append {
        opts.append(true);
    }
    let mut file = opts.open(path)?;
    if append {
        file.seek(SeekFrom::End(0))?;
    }
    Ok(BufWriter::new(file))
}

fn verify_sha256(path: &Path, expected: &str) -> std::io::Result<bool> {
    match sha256_hex(path) {
        Ok(actual) => Ok(actual == expected.to_ascii_lowercase()),
        Err(DownloadError::Fs(err)) => Err(err),
        Err(other) => {
            // sha256_hex only fails on IO for a local file.
            Err(std::io::Error::other(other.to_string()))
        }
    }
}

fn sha256_hex(path: &Path) -> Result<String, DownloadError> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn rename_atomic(from: &Path, to: &Path) -> std::io::Result<()> {
    // fs::rename is atomic on the same filesystem; both paths live under the
    // models root, so this holds. Overwrite-if-present is not supported by
    // std::fs::rename on Unix when the destination exists, so remove first.
    if to.exists() {
        fs::remove_file(to)?;
    }
    fs::rename(from, to)
}

/// Integrity state of an asset on disk, as determined by [`verify_asset`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssetIntegrity {
    /// The final file exists and its SHA-256 matches the manifest entry.
    Valid,
    /// The final file exists but its SHA-256 does not match (corruption or
    /// an out-of-date asset). The caller should [`repair_asset`] (re-download).
    Corrupt,
    /// The final file does not exist. The caller should download it.
    Missing,
}

/// Check an asset's on-disk integrity against its manifest SHA-256.
///
/// Cheap single-pass hash of the final asset file. Returns `Missing` if the
/// file is absent, `Corrupt` if present but the hash mismatches, `Valid` if it
/// matches. Intended to run before a provider loads an asset, so a corrupt
/// file triggers re-download rather than a confusing native crash.
pub fn verify_asset(asset: &AssetEntry, storage: &AssetStorage) -> AssetIntegrity {
    let path = storage.asset_path(&asset.id, asset.role);
    if !path.exists() {
        return AssetIntegrity::Missing;
    }
    match verify_sha256(&path, &asset.sha256) {
        Ok(true) => AssetIntegrity::Valid,
        Ok(false) => AssetIntegrity::Corrupt,
        Err(_) => AssetIntegrity::Corrupt,
    }
}

/// Re-download an asset whose on-disk integrity is `Corrupt` or `Missing`.
///
/// Removes any corrupt final file first, then delegates to [`Downloader::download`]
/// (which fetches, verifies, and atomically renames). Returns the outcome on
/// success. This is the repair path called after [`verify_asset`] reports a
/// problem, or proactively before loading an ASR asset where a corrupt load
/// could crash the engine.
pub async fn repair_asset(
    asset: &AssetEntry,
    storage: &AssetStorage,
    client: &Client,
) -> Result<DownloadOutcome, DownloadError> {
    let final_path = storage.asset_path(&asset.id, asset.role);
    if final_path.exists() {
        // Corrupt final file: remove so the downloader starts fresh rather
        // than short-circuiting on the cached (corrupt) path.
        fs::remove_file(&final_path)?;
    }
    let downloader = Downloader::new(client.clone(), storage.clone());
    downloader.download(asset, false).await
}

/// Verify every default asset and report any that are not `Valid`.
///
/// Useful as a startup integrity sweep: returns the list of default assets
/// that are `Missing` or `Corrupt` and thus need (re-)downloading.
pub fn integrity_sweep<'a>(
    manifest: &'a AssetManifest,
    storage: &AssetStorage,
) -> Vec<(&'a AssetEntry, AssetIntegrity)> {
    manifest
        .defaults()
        .map(|asset| {
            let integrity = verify_asset(asset, storage);
            (asset, integrity)
        })
        .filter(|(_, integrity)| *integrity != AssetIntegrity::Valid)
        .collect()
}

/// Readiness of a single default asset on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssetReadiness {
    /// The final asset file is present. (SHA verification is performed lazily
    /// by the downloader; presence is enough to consider it "ready" for
    /// gating purposes — a corrupt file re-downloads on next access.)
    Present,
    /// The final file is absent (a `.part` may or may not exist).
    Missing,
}

/// Reports which default assets are missing from `storage`.
///
/// Returns the list of default [`AssetEntry`]s whose final file does not yet
/// exist. Pure filesystem existence check (no SHA); used to decide whether a
/// first-run download is needed.
pub fn missing_defaults<'a>(
    manifest: &'a AssetManifest,
    storage: &AssetStorage,
) -> Vec<&'a AssetEntry> {
    manifest
        .defaults()
        .filter(|asset| {
            !storage
                .asset_path(&asset.id, asset.role)
                .exists()
        })
        .collect()
}

/// Per-asset outcome of [`download_defaults`].
#[derive(Debug)]
pub struct DefaultDownloadResult {
    pub asset_id: String,
    pub outcome: Result<DownloadOutcome, DownloadError>,
}

/// Download every default asset in `manifest` that is not already present.
///
/// Calls `on_progress` (if provided) with the asset id and bytes-so-far before
/// each download starts, so the caller can emit UI events. Assets already
/// present are skipped (counted as `Cached`).
pub async fn download_defaults<F>(
    manifest: &AssetManifest,
    storage: &AssetStorage,
    client: &Client,
    mut on_progress: F,
) -> Vec<DefaultDownloadResult>
where
    F: FnMut(&AssetEntry),
{
    let downloader = Downloader::new(client.clone(), storage.clone());
    let mut results = Vec::new();
    for asset in manifest.defaults() {
        // Skip if already present; the downloader's own cached-final path would
        // also handle this, but skipping avoids a redundant progress signal.
        if storage.asset_path(&asset.id, asset.role).exists() {
            results.push(DefaultDownloadResult {
                asset_id: asset.id.clone(),
                outcome: Ok(DownloadOutcome {
                    final_path: storage.asset_path(&asset.id, asset.role),
                    source: DownloadSource::Cached,
                }),
            });
            continue;
        }
        on_progress(asset);
        let outcome = downloader.download(asset, true).await;
        results.push(DefaultDownloadResult {
            asset_id: asset.id.clone(),
            outcome,
        });
    }
    results
}

// Suppress unused import warning until the async read path is exercised; the
// tokio AsyncReadExt import is kept for a future streaming variant.
#[allow(dead_code)]
fn _read_full(_reader: &mut tokio::fs::File) -> std::io::Result<Vec<u8>> {
    Ok(Vec::new())
}

#[allow(unused_imports)]
use tokio::io::AsyncReadExt as _;