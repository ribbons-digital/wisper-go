//! Integration tests for the asset downloader. Uses `httpmock` so no real
//! network is touched in CI. Each test builds a temp models root, serves bytes
//! from a local mock, and asserts on disk state + SHA-256 verification.

use std::fs;
use std::path::PathBuf;

use httpmock::prelude::*;
use reqwest::Client;
use sha2::{Digest, Sha256};
use tempfile::TempDir;
use wispergo_core::asset_manifest::{AssetEntry, AssetManifest, AssetRole};
use wispergo_core::asset_storage::AssetStorage;
use wispergo_core::downloader::{
    download_defaults, missing_defaults, DownloadError, DownloadSource, Downloader,
};

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn asset(url: String, bytes: &[u8], role: AssetRole) -> AssetEntry {
    AssetEntry {
        id: "test-asset".to_string(),
        role,
        display_name: "Test".to_string(),
        url,
        size: bytes.len() as u64,
        sha256: sha256_hex(bytes),
        default: false,
    }
}

fn default_asset(id: &str, url: String, bytes: &[u8], role: AssetRole) -> AssetEntry {
    AssetEntry {
        id: id.to_string(),
        role,
        display_name: id.to_string(),
        url,
        size: bytes.len() as u64,
        sha256: sha256_hex(bytes),
        default: true,
    }
}

fn manifest_with_assets(assets: Vec<AssetEntry>) -> AssetManifest {
    AssetManifest {
        schema_version: 1,
        assets,
    }
}

fn storage(tmp: &TempDir) -> AssetStorage {
    AssetStorage::new(tmp.path().join("models"))
}

fn client() -> Client {
    Client::builder()
        .build()
        .expect("client")
}

#[tokio::test]
async fn fresh_download_verifies_and_renames_to_final() {
    let server = MockServer::start();
    let bytes = b"hello world this is a model file";
    server.mock(|when, then| {
        when.method(GET).path("/model.bin");
        then.status(200).body(bytes);
    });

    let tmp = TempDir::new().unwrap();
    let storage = storage(&tmp);
    let dl = Downloader::new(client(), storage.clone());
    let asset = asset(server.url("/model.bin"), bytes, AssetRole::Asr);

    let outcome = dl.download(&asset, false).await.expect("download ok");

    assert_eq!(outcome.source, DownloadSource::Fresh);
    let final_path = storage.asset_path("test-asset", AssetRole::Asr);
    assert_eq!(outcome.final_path, final_path);
    assert_eq!(fs::read(&final_path).unwrap(), bytes);
    // The .part file must be gone.
    assert!(!storage.part_path("test-asset", AssetRole::Asr).exists());
}

#[tokio::test]
async fn resumes_from_existing_part_via_range_request() {
    let server = MockServer::start();
    let full = b"0123456789ABCDEFGHIJ"; // 20 bytes
    let prefix_len = 10usize;
    let suffix = &full[prefix_len..];

    // Seed a .part with the first 10 bytes.
    let tmp = TempDir::new().unwrap();
    let storage = storage(&tmp);
    let part_path = storage.part_path("test-asset", AssetRole::Asr);
    fs::create_dir_all(part_path.parent().unwrap()).unwrap();
    fs::write(&part_path, &full[..prefix_len]).unwrap();

    // The server should receive a Range request and respond 206 with the suffix.
    server.mock(|when, then| {
        when.method(GET)
            .path("/model.bin")
            .header("Range", "bytes=10-");
        then.status(206).body(suffix);
    });

    let dl = Downloader::new(client(), storage.clone());
    let asset = asset(server.url("/model.bin"), full, AssetRole::Asr);

    let outcome = dl.download(&asset, true).await.expect("resume ok");

    assert_eq!(
        outcome.source,
        DownloadSource::Resumed {
            resumed_from: prefix_len as u64
        }
    );
    let final_path = storage.asset_path("test-asset", AssetRole::Asr);
    assert_eq!(fs::read(&final_path).unwrap(), full);
}

#[tokio::test]
async fn server_ignores_range_and_returns_full_body() {
    let server = MockServer::start();
    let full = b"complete body";
    let tmp = TempDir::new().unwrap();
    let storage = storage(&tmp);
    // A stale .part that should be overwritten, not appended to.
    let part_path = storage.part_path("test-asset", AssetRole::Asr);
    fs::create_dir_all(part_path.parent().unwrap()).unwrap();
    fs::write(&part_path, b"STALE").unwrap();

    // Server ignores Range and returns 200 with the full body.
    server.mock(|when, then| {
        when.method(GET).path("/model.bin");
        then.status(200).body(full);
    });

    let dl = Downloader::new(client(), storage.clone());
    let asset = asset(server.url("/model.bin"), full, AssetRole::Asr);

    let outcome = dl.download(&asset, true).await.expect("full-body ok");

    assert_eq!(outcome.source, DownloadSource::Fresh);
    let final_path = storage.asset_path("test-asset", AssetRole::Asr);
    assert_eq!(fs::read(&final_path).unwrap(), full);
}

#[tokio::test]
async fn sha256_mismatch_retries_once_then_fails() {
    let server = MockServer::start();
    let good = b"correct bytes";
    let bad = b"corrupted bytes";

    let mock = server.mock(|when, then| {
        when.method(GET).path("/model.bin");
        // First call returns bad, second returns good — but the asset's
        // sha256 is for `good`, so the first mismatches and retries.
        then.status(200).body(bad);
    });

    // Make the asset expect the *good* hash, so the first (bad) response
    // mismatches. Then swap the mock body to good for the retry.
    let tmp = TempDir::new().unwrap();
    let storage = storage(&tmp);
    let asset = asset(server.url("/model.bin"), good, AssetRole::Asr);
    let dl = Downloader::new(client(), storage.clone());

    let result = dl.download(&asset, false).await;

    // Both attempts returned `bad`, so both mismatch -> final error.
    assert!(matches!(result, Err(DownloadError::Sha256Mismatch { .. })));
    assert_eq!(mock.hits(), 2, "should retry once on mismatch");
    // No final file should exist.
    assert!(!storage.asset_path("test-asset", AssetRole::Asr).exists());
}

#[tokio::test]
async fn already_verified_final_file_short_circuits() {
    let server = MockServer::start();
    let bytes = b"already present";
    let tmp = TempDir::new().unwrap();
    let storage = storage(&tmp);
    let final_path = storage.asset_path("test-asset", AssetRole::Asr);
    fs::create_dir_all(final_path.parent().unwrap()).unwrap();
    fs::write(&final_path, bytes).unwrap();

    let mock = server.mock(|when, then| {
        when.method(GET).path("/model.bin");
        then.status(200).body(bytes);
    });

    let asset = asset(server.url("/model.bin"), bytes, AssetRole::Asr);
    let dl = Downloader::new(client(), storage.clone());

    let outcome = dl.download(&asset, false).await.expect("cached ok");

    assert_eq!(outcome.source, DownloadSource::Cached);
    assert_eq!(mock.hits(), 0, "must not fetch when final verifies");
}

#[tokio::test]
async fn corrupt_final_file_is_redownloaded() {
    let server = MockServer::start();
    let good = b"good bytes";
    let tmp = TempDir::new().unwrap();
    let storage = storage(&tmp);
    let final_path = storage.asset_path("test-asset", AssetRole::Asr);
    fs::create_dir_all(final_path.parent().unwrap()).unwrap();
    fs::write(&final_path, b"corrupt").unwrap();

    server.mock(|when, then| {
        when.method(GET).path("/model.bin");
        then.status(200).body(good);
    });

    let asset = asset(server.url("/model.bin"), good, AssetRole::Asr);
    let dl = Downloader::new(client(), storage.clone());

    let outcome = dl.download(&asset, false).await.expect("redownload ok");

    assert_eq!(outcome.source, DownloadSource::Fresh);
    assert_eq!(fs::read(&final_path).unwrap(), good);
}

#[tokio::test]
async fn http_error_is_returned_without_retries() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(GET).path("/model.bin");
        then.status(404);
    });

    let tmp = TempDir::new().unwrap();
    let storage = storage(&tmp);
    let asset = asset(server.url("/model.bin"), b"unused", AssetRole::Asr);
    let dl = Downloader::new(client(), storage.clone());

    let result = dl.download(&asset, false).await;
    assert!(matches!(result, Err(DownloadError::HttpStatus { .. })));
    assert_eq!(mock.hits(), 1, "no retry on non-sha errors");
}

#[tokio::test]
async fn cleanup_role_uses_gguf_path() {
    let server = MockServer::start();
    let bytes = b"cleanup model weights";
    server.mock(|when, then| {
        when.method(GET).path("/q.gguf");
        then.status(200).body(bytes);
    });

    let tmp = TempDir::new().unwrap();
    let storage = storage(&tmp);
    let mut asset = asset(server.url("/q.gguf"), bytes, AssetRole::CleanupPunctuation);
    asset.id = "qwen0.5b".to_string();
    let dl = Downloader::new(client(), storage.clone());

    let outcome = dl.download(&asset, false).await.expect("cleanup ok");

    let expected: PathBuf = storage
        .models_root()
        .join("cleanup")
        .join("qwen0.5b.gguf");
    assert_eq!(outcome.final_path, expected);
}

#[tokio::test]
async fn missing_defaults_lists_only_absent_defaults() {
    let tmp = TempDir::new().unwrap();
    let storage = storage(&tmp);
    let bytes_a = b"asr bytes";
    let bytes_b = b"cleanup bytes";
    let manifest = manifest_with_assets(vec![
        default_asset("medium", "http://x/a".to_string(), bytes_a, AssetRole::Asr),
        default_asset(
            "qwen0.5b",
            "http://x/b".to_string(),
            bytes_b,
            AssetRole::CleanupPunctuation,
        ),
    ]);

    // Nothing present yet -> both missing.
    assert_eq!(missing_defaults(&manifest, &storage).len(), 2);

    // Write the ASR asset to disk as if already downloaded.
    let asr_path = storage.asset_path("medium", AssetRole::Asr);
    fs::create_dir_all(asr_path.parent().unwrap()).unwrap();
    fs::write(&asr_path, bytes_a).unwrap();

    let missing = missing_defaults(&manifest, &storage);
    assert_eq!(missing.len(), 1);
    assert_eq!(missing[0].id, "qwen0.5b");
}

#[tokio::test]
async fn missing_defaults_ignores_non_default_assets() {
    let tmp = TempDir::new().unwrap();
    let storage = storage(&tmp);
    let manifest = manifest_with_assets(vec![
        default_asset("medium", "http://x/a".to_string(), b"a", AssetRole::Asr),
        AssetEntry {
            id: "large".to_string(),
            role: AssetRole::Asr,
            display_name: "large".to_string(),
            url: "http://x/l".to_string(),
            size: 1,
            sha256: sha256_hex(b"x"),
            default: false,
        },
    ]);
    // Non-default "large" is absent but should not be reported.
    assert_eq!(missing_defaults(&manifest, &storage).len(), 1);
    assert_eq!(missing_defaults(&manifest, &storage)[0].id, "medium");
}

#[tokio::test]
async fn download_defaults_fetches_all_missing_and_skips_present() {
    let server = MockServer::start();
    let bytes_a = b"asr bytes";
    let bytes_b = b"cleanup bytes";
    server.mock(|when, then| {
        when.method(GET).path("/a");
        then.status(200).body(bytes_a);
    });
    server.mock(|when, then| {
        when.method(GET).path("/b");
        then.status(200).body(bytes_b);
    });

    let tmp = TempDir::new().unwrap();
    let storage = storage(&tmp);
    let manifest = manifest_with_assets(vec![
        default_asset("medium", server.url("/a"), bytes_a, AssetRole::Asr),
        default_asset(
            "qwen0.5b",
            server.url("/b"),
            bytes_b,
            AssetRole::CleanupPunctuation,
        ),
    ]);

    let mut seen: Vec<String> = Vec::new();
    let results = download_defaults(&manifest, &storage, &client(), |a| {
        seen.push(a.id.clone());
    })
    .await;

    assert_eq!(results.len(), 2);
    assert!(results.iter().all(|r| r.outcome.is_ok()));
    // Progress fired for both (neither was present).
    assert_eq!(seen, vec!["medium", "qwen0.5b"]);
    // Both files now exist.
    assert!(storage.asset_path("medium", AssetRole::Asr).exists());
    assert!(storage
        .asset_path("qwen0.5b", AssetRole::CleanupPunctuation)
        .exists());
}

#[tokio::test]
async fn download_defaults_skips_already_present_without_progress() {
    let server = MockServer::start();
    let bytes = b"asr bytes";
    let mock = server.mock(|when, then| {
        when.method(GET).path("/a");
        then.status(200).body(bytes);
    });

    let tmp = TempDir::new().unwrap();
    let storage = storage(&tmp);
    // Pre-place the ASR asset.
    let asr_path = storage.asset_path("medium", AssetRole::Asr);
    fs::create_dir_all(asr_path.parent().unwrap()).unwrap();
    fs::write(&asr_path, bytes).unwrap();

    let manifest = manifest_with_assets(vec![default_asset(
        "medium",
        server.url("/a"),
        bytes,
        AssetRole::Asr,
    )]);

    let mut seen = 0;
    let results = download_defaults(&manifest, &storage, &client(), |_| {
        seen += 1;
    })
    .await;

    assert_eq!(results.len(), 1);
    assert!(results[0].outcome.is_ok());
    assert_eq!(mock.hits(), 0, "must not fetch present asset");
    assert_eq!(seen, 0, "must not emit progress for present asset");
}

#[tokio::test]
async fn download_defaults_records_failure_but_continues() {
    let server = MockServer::start();
    let bytes_b = b"cleanup bytes";
    server.mock(|when, then| {
        when.method(GET).path("/a");
        then.status(500);
    });
    server.mock(|when, then| {
        when.method(GET).path("/b");
        then.status(200).body(bytes_b);
    });

    let tmp = TempDir::new().unwrap();
    let storage = storage(&tmp);
    let manifest = manifest_with_assets(vec![
        default_asset("medium", server.url("/a"), b"asr", AssetRole::Asr),
        default_asset(
            "qwen0.5b",
            server.url("/b"),
            bytes_b,
            AssetRole::CleanupPunctuation,
        ),
    ]);

    let results = download_defaults(&manifest, &storage, &client(), |_| {}).await;

    assert_eq!(results.len(), 2);
    assert!(matches!(
        results[0].outcome,
        Err(DownloadError::HttpStatus { .. })
    ));
    assert!(results[1].outcome.is_ok());
}
