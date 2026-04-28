use crate::privacy::{CloudFallbackMode, PrivacyPolicy, PrivacyPolicyEngine, ProviderKind};
use crate::providers::ProviderError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FallbackRequest {
    pub app_id: String,
    pub provider_kind: ProviderKind,
    pub error: ProviderError,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FallbackDecision {
    FailLocalOnly,
    AskBeforeCloud,
    UseCloudAutomatically,
    CloudBlockedForApp,
    FailUnrecoverable,
}

#[derive(Debug, Clone)]
pub struct FallbackEngine {
    privacy: PrivacyPolicyEngine,
}

impl FallbackEngine {
    pub fn new(policy: PrivacyPolicy) -> Self {
        Self {
            privacy: PrivacyPolicyEngine::new(policy),
        }
    }

    pub fn decide(&self, request: FallbackRequest) -> FallbackDecision {
        if !request.error.is_recoverable() {
            return FallbackDecision::FailUnrecoverable;
        }

        if !self
            .privacy
            .can_use_cloud(&request.app_id, request.provider_kind)
        {
            return if matches!(
                self.privacy.cloud_fallback_mode(),
                CloudFallbackMode::LocalOnly
            ) {
                FallbackDecision::FailLocalOnly
            } else {
                FallbackDecision::CloudBlockedForApp
            };
        }

        match self.privacy.cloud_fallback_mode() {
            CloudFallbackMode::LocalOnly => FallbackDecision::FailLocalOnly,
            CloudFallbackMode::PreferLocalAskBeforeCloud => FallbackDecision::AskBeforeCloud,
            CloudFallbackMode::PreferLocalAutomaticCloud => FallbackDecision::UseCloudAutomatically,
        }
    }
}
