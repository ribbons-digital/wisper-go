use wispergo_core::privacy::{CloudFallbackMode, PrivacyPolicy};
use wispergo_core::store::LocalStore;

#[test]
fn saves_and_loads_privacy_policy() {
    let store = LocalStore::open_in_memory().expect("open store");
    store.migrate().expect("migrate");

    let policy = PrivacyPolicy {
        fallback_mode: CloudFallbackMode::PreferLocalAutomaticCloud,
        cloud_disabled_apps: vec!["com.apple.Terminal".to_string()],
        context_disabled_apps: vec!["com.company.SecretApp".to_string()],
        history_enabled: false,
        store_audio: false,
    };

    store.save_privacy_policy(&policy).expect("save policy");
    let loaded = store.load_privacy_policy().expect("load policy");

    assert_eq!(loaded, policy);
}

#[test]
fn history_respects_enabled_flag_at_call_site() {
    let store = LocalStore::open_in_memory().expect("open store");
    store.migrate().expect("migrate");

    store
        .insert_history("hello world", "local")
        .expect("insert history");

    let rows = store.history_count().expect("history count");
    assert_eq!(rows, 1);
}
