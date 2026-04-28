use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CloudFallbackMode {
    LocalOnly,
    PreferLocalAskBeforeCloud,
    PreferLocalAutomaticCloud,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderKind {
    Asr,
    Cleanup,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextKind {
    ActiveApp,
    WindowTitle,
    SelectedText,
    NearbyText,
    Dictionary,
    StyleProfile,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrivacyPolicy {
    pub fallback_mode: CloudFallbackMode,
    pub cloud_disabled_apps: Vec<String>,
    pub context_disabled_apps: Vec<String>,
    pub history_enabled: bool,
    pub store_audio: bool,
}

impl Default for PrivacyPolicy {
    fn default() -> Self {
        Self {
            fallback_mode: CloudFallbackMode::PreferLocalAskBeforeCloud,
            cloud_disabled_apps: Vec::new(),
            context_disabled_apps: Vec::new(),
            history_enabled: true,
            store_audio: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PrivacyPolicyEngine {
    policy: PrivacyPolicy,
}

impl Default for PrivacyPolicyEngine {
    fn default() -> Self {
        Self::new(PrivacyPolicy::default())
    }
}

impl PrivacyPolicyEngine {
    pub fn new(policy: PrivacyPolicy) -> Self {
        Self { policy }
    }

    pub fn policy(&self) -> &PrivacyPolicy {
        &self.policy
    }

    pub fn cloud_fallback_mode(&self) -> CloudFallbackMode {
        self.policy.fallback_mode
    }

    pub fn can_use_cloud(&self, app_id: &str, _provider: ProviderKind) -> bool {
        if self.policy.cloud_disabled_apps.iter().any(|id| id == app_id) {
            return false;
        }

        !matches!(self.policy.fallback_mode, CloudFallbackMode::LocalOnly)
    }

    pub fn can_collect_context(&self, app_id: &str, kind: ContextKind) -> bool {
        if matches!(
            kind,
            ContextKind::SelectedText | ContextKind::NearbyText | ContextKind::WindowTitle
        ) && self
            .policy
            .context_disabled_apps
            .iter()
            .any(|id| id == app_id)
        {
            return false;
        }

        true
    }

    pub fn can_store_history(&self) -> bool {
        self.policy.history_enabled
    }

    pub fn can_store_audio(&self) -> bool {
        self.policy.store_audio
    }
}
