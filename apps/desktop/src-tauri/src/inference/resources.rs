use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CpuArchitecture {
    Aarch64,
    X86_64,
}

impl CpuArchitecture {
    pub fn current() -> Self {
        if cfg!(target_arch = "aarch64") {
            Self::Aarch64
        } else {
            Self::X86_64
        }
    }

    pub fn resource_dir_name(self) -> &'static str {
        match self {
            Self::Aarch64 => "macos-aarch64",
            Self::X86_64 => "macos-x86_64",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InferenceResourcePaths {
    pub resource_root: PathBuf,
    pub whisper_binary_path: PathBuf,
    pub llama_server_binary_path: PathBuf,
    pub asr_model_path: PathBuf,
    pub cleanup_model_path: PathBuf,
}

impl InferenceResourcePaths {
    pub fn from_resource_root(resource_root: PathBuf) -> Self {
        Self::from_resource_root_for_arch(resource_root, CpuArchitecture::current())
    }

    pub fn from_resource_root_for_arch(
        resource_root: PathBuf,
        architecture: CpuArchitecture,
    ) -> Self {
        let bin_root = resource_root
            .join("bin")
            .join(architecture.resource_dir_name());

        Self {
            whisper_binary_path: bin_root.join("whisper-cli"),
            llama_server_binary_path: bin_root.join("llama-server"),
            asr_model_path: resource_root.join("models/asr/ggml-large-v3-turbo.bin"),
            cleanup_model_path: resource_root
                .join("models/cleanup/qwen2.5-3b-instruct-q4_k_m.gguf"),
            resource_root,
        }
    }

    pub fn validate_required_assets(&self) -> Result<(), String> {
        let required = [
            &self.whisper_binary_path,
            &self.llama_server_binary_path,
            &self.asr_model_path,
            &self.cleanup_model_path,
        ];
        let missing = required
            .iter()
            .filter(|path| !path.exists())
            .map(|path| display_relative_or_absolute(&self.resource_root, path))
            .collect::<Vec<_>>();

        if missing.is_empty() {
            Ok(())
        } else {
            Err(format!(
                "Wispergo installation is missing bundled inference assets: {}",
                missing.join(", ")
            ))
        }
    }
}

fn display_relative_or_absolute(root: &PathBuf, path: &PathBuf) -> String {
    path.strip_prefix(root)
        .map(|relative| relative.display().to_string())
        .unwrap_or_else(|_| path.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_aarch64_resource_paths() {
        let root = PathBuf::from("/Applications/Wispergo.app/Contents/Resources");
        let current_arch_paths = InferenceResourcePaths::from_resource_root(root.clone());
        assert_eq!(current_arch_paths.resource_root, root);

        let paths = InferenceResourcePaths::from_resource_root_for_arch(
            root.clone(),
            CpuArchitecture::Aarch64,
        );

        assert_eq!(paths.resource_root, root);
        assert_eq!(
            paths.whisper_binary_path,
            root.join("bin/macos-aarch64/whisper-cli")
        );
        assert_eq!(
            paths.llama_server_binary_path,
            root.join("bin/macos-aarch64/llama-server")
        );
        assert_eq!(
            paths.asr_model_path,
            root.join("models/asr/ggml-large-v3-turbo.bin")
        );
        assert_eq!(
            paths.cleanup_model_path,
            root.join("models/cleanup/qwen2.5-3b-instruct-q4_k_m.gguf")
        );
    }

    #[test]
    fn resolves_x86_64_resource_paths() {
        let root = PathBuf::from("/Applications/Wispergo.app/Contents/Resources");
        let paths = InferenceResourcePaths::from_resource_root_for_arch(
            root.clone(),
            CpuArchitecture::X86_64,
        );

        assert_eq!(paths.resource_root, root);
        assert_eq!(
            paths.whisper_binary_path,
            root.join("bin/macos-x86_64/whisper-cli")
        );
        assert_eq!(
            paths.llama_server_binary_path,
            root.join("bin/macos-x86_64/llama-server")
        );
        assert_eq!(
            paths.asr_model_path,
            root.join("models/asr/ggml-large-v3-turbo.bin")
        );
        assert_eq!(
            paths.cleanup_model_path,
            root.join("models/cleanup/qwen2.5-3b-instruct-q4_k_m.gguf")
        );
    }

    #[test]
    fn missing_resource_validation_lists_exact_missing_paths() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("wispergo-missing-assets-{unique}"));
        std::fs::create_dir_all(&root).expect("create root");
        let paths = InferenceResourcePaths::from_resource_root_for_arch(
            root.clone(),
            CpuArchitecture::Aarch64,
        );

        let error = paths
            .validate_required_assets()
            .expect_err("missing assets");

        assert_eq!(
            error,
            "Wispergo installation is missing bundled inference assets: bin/macos-aarch64/whisper-cli, bin/macos-aarch64/llama-server, models/asr/ggml-large-v3-turbo.bin, models/cleanup/qwen2.5-3b-instruct-q4_k_m.gguf"
        );

        let _ = std::fs::remove_dir_all(root);
    }
}
