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
            asr_model_path: resource_root.join("models/asr/ggml-large-v3-turbo.bin"),
            cleanup_model_path: resource_root
                .join("models/cleanup/qwen2.5-3b-instruct-q4_k_m.gguf"),
            resource_root,
        }
    }
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
            paths.asr_model_path,
            root.join("models/asr/ggml-large-v3-turbo.bin")
        );
        assert_eq!(
            paths.cleanup_model_path,
            root.join("models/cleanup/qwen2.5-3b-instruct-q4_k_m.gguf")
        );
    }
}
