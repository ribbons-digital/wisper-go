use wispergo_core::privacy::{
    CloudFallbackMode, ContextKind, PrivacyPolicy, PrivacyPolicyEngine, ProviderKind,
};

#[test]
fn local_only_never_allows_cloud_asr_or_cleanup() {
    let policy = PrivacyPolicy {
        fallback_mode: CloudFallbackMode::LocalOnly,
        ..PrivacyPolicy::default()
    };
    let engine = PrivacyPolicyEngine::new(policy);

    assert!(!engine.can_use_cloud("com.apple.Notes", ProviderKind::Asr));
    assert!(!engine.can_use_cloud("com.apple.Notes", ProviderKind::Cleanup));
}

#[test]
fn app_cloud_deny_list_overrides_automatic_fallback() {
    let policy = PrivacyPolicy {
        fallback_mode: CloudFallbackMode::PreferLocalAutomaticCloud,
        cloud_disabled_apps: vec!["com.apple.Terminal".to_string()],
        ..PrivacyPolicy::default()
    };
    let engine = PrivacyPolicyEngine::new(policy);

    assert!(!engine.can_use_cloud("com.apple.Terminal", ProviderKind::Cleanup));
    assert!(engine.can_use_cloud("com.apple.Notes", ProviderKind::Cleanup));
}

#[test]
fn context_disabled_for_app_blocks_selected_and_nearby_text() {
    let policy = PrivacyPolicy {
        context_disabled_apps: vec!["com.company.SecretApp".to_string()],
        ..PrivacyPolicy::default()
    };
    let engine = PrivacyPolicyEngine::new(policy);

    assert!(!engine.can_collect_context("com.company.SecretApp", ContextKind::SelectedText));
    assert!(!engine.can_collect_context("com.company.SecretApp", ContextKind::NearbyText));
    assert!(engine.can_collect_context("com.apple.Notes", ContextKind::ActiveApp));
}

#[test]
fn history_and_audio_defaults_are_private() {
    let engine = PrivacyPolicyEngine::default();

    assert!(engine.can_store_history());
    assert!(!engine.can_store_audio());
}
