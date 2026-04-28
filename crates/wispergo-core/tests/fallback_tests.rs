use wispergo_core::fallback::{FallbackDecision, FallbackEngine, FallbackRequest};
use wispergo_core::privacy::{CloudFallbackMode, PrivacyPolicy, ProviderKind};
use wispergo_core::providers::ProviderError;

#[test]
fn local_only_fails_closed_on_provider_timeout() {
    let engine = FallbackEngine::new(PrivacyPolicy {
        fallback_mode: CloudFallbackMode::LocalOnly,
        ..PrivacyPolicy::default()
    });

    let decision = engine.decide(FallbackRequest {
        app_id: "com.apple.Notes".to_string(),
        provider_kind: ProviderKind::Asr,
        error: ProviderError::Timeout {
            provider: "local_asr".to_string(),
        },
    });

    assert_eq!(decision, FallbackDecision::FailLocalOnly);
}

#[test]
fn ask_before_cloud_returns_confirmation_decision() {
    let engine = FallbackEngine::new(PrivacyPolicy {
        fallback_mode: CloudFallbackMode::PreferLocalAskBeforeCloud,
        ..PrivacyPolicy::default()
    });

    let decision = engine.decide(FallbackRequest {
        app_id: "com.apple.Notes".to_string(),
        provider_kind: ProviderKind::Cleanup,
        error: ProviderError::Unavailable {
            provider: "ollama".to_string(),
        },
    });

    assert_eq!(decision, FallbackDecision::AskBeforeCloud);
}

#[test]
fn automatic_cloud_respects_app_deny_list() {
    let engine = FallbackEngine::new(PrivacyPolicy {
        fallback_mode: CloudFallbackMode::PreferLocalAutomaticCloud,
        cloud_disabled_apps: vec!["com.apple.Terminal".to_string()],
        ..PrivacyPolicy::default()
    });

    let decision = engine.decide(FallbackRequest {
        app_id: "com.apple.Terminal".to_string(),
        provider_kind: ProviderKind::Cleanup,
        error: ProviderError::Timeout {
            provider: "ollama".to_string(),
        },
    });

    assert_eq!(decision, FallbackDecision::CloudBlockedForApp);
}
