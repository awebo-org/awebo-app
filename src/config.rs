use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Persistent application configuration stored as TOML.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
#[derive(Default)]
pub struct AppConfig {
    pub appearance: AppearanceConfig,
    pub ai: AiConfig,
    pub general: GeneralConfig,
    pub sandbox: SandboxDefaultsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppearanceConfig {
    pub input_type: String,
    pub font_family: String,
    pub font_size: f32,
    pub line_height: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AiConfig {
    pub models_path: String,
    pub last_model: String,
    pub web_search: bool,
    pub context_lines: usize,
    pub auto_load: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
#[derive(Default)]
pub struct GeneralConfig {
    pub default_shell: String,
    pub hint_banner_dismissed: bool,
    #[serde(default)]
    pub hints: HintsConfig,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct HintsConfig {
    pub seen_agent: bool,
    pub seen_sandbox: bool,
    pub seen_git: bool,
    pub seen_ask: bool,
}

impl Default for AppearanceConfig {
    fn default() -> Self {
        Self {
            input_type: "smart".into(),
            font_family: "JetBrains Mono".into(),
            font_size: 16.0,
            line_height: 22.0,
        }
    }
}

impl Default for AiConfig {
    fn default() -> Self {
        let models_path = crate::ai::model_manager::models_dir()
            .to_string_lossy()
            .into_owned();
        Self {
            models_path,
            last_model: String::new(),
            web_search: false,
            context_lines: 30,
            auto_load: true,
        }
    }
}

/// Persisted sandbox defaults — used when creating new sandboxes.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SandboxDefaultsConfig {
    /// Default vCPU count for new sandboxes.
    pub default_cpus: u32,
    /// Default memory in MiB for new sandboxes.
    pub default_memory_mib: u32,
    /// Default volume mounts applied to every new sandbox.
    pub volumes: Vec<VolumeMountConfig>,
    /// User-added custom OCI images (beyond the built-in trusted list).
    pub custom_images: Vec<CustomImageConfig>,
}

/// A persisted volume mount definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeMountConfig {
    /// Human-readable label for this volume.
    pub label: String,
    /// Host path to bind-mount.
    pub host_path: String,
    /// Guest path inside the sandbox.
    pub guest_path: String,
}

/// A user-added custom OCI image.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomImageConfig {
    /// OCI image reference (e.g. "myregistry/myimage:v1").
    pub oci_ref: String,
    /// Human-readable display name.
    pub display_name: String,
    /// Image tag (e.g. "latest", "v1.2").
    pub tag: String,
    /// Default shell inside the image.
    pub default_shell: String,
    /// Default working directory inside the image.
    pub default_workdir: String,
    /// ISO-8601 timestamp of last successful pull (empty if never pulled).
    pub last_pulled: String,
}

impl Default for SandboxDefaultsConfig {
    fn default() -> Self {
        Self {
            default_cpus: 1,
            default_memory_mib: 512,
            volumes: Vec::new(),
            custom_images: Vec::new(),
        }
    }
}

fn config_path() -> PathBuf {
    let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    base.join("awebo").join("config.toml")
}

impl AppConfig {
    /// Load config from disk, falling back to defaults on any error.
    pub fn load() -> Self {
        let path = config_path();
        match std::fs::read_to_string(&path) {
            Ok(content) => toml::from_str(&content).unwrap_or_else(|e| {
                log::warn!("Failed to parse config {}: {e}", path.display());
                Self::default()
            }),
            Err(_) => Self::default(),
        }
    }

    /// Save config to disk, creating parent directories as needed.
    pub fn save(&self) {
        let path = config_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        match toml::to_string_pretty(self) {
            Ok(content) => {
                if let Err(e) = std::fs::write(&path, content) {
                    log::error!("Failed to write config {}: {e}", path.display());
                }
            }
            Err(e) => {
                log::error!("Failed to serialize config: {e}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_round_trips() {
        let config = AppConfig::default();
        let serialized = toml::to_string_pretty(&config).unwrap();
        let deserialized: AppConfig = toml::from_str(&serialized).unwrap();
        assert_eq!(deserialized.appearance.font_size, 16.0);
        assert_eq!(deserialized.appearance.input_type, "smart");
        assert!(!deserialized.ai.web_search);
    }

    #[test]
    fn partial_toml_fills_defaults() {
        let partial = r#"
[appearance]
font_size = 20.0
"#;
        let config: AppConfig = toml::from_str(partial).unwrap();
        assert_eq!(config.appearance.font_size, 20.0);
        assert_eq!(config.appearance.font_family, "JetBrains Mono");
        assert_eq!(config.appearance.input_type, "smart");
    }

    #[test]
    fn empty_toml_gives_defaults() {
        let config: AppConfig = toml::from_str("").unwrap();
        assert_eq!(config.appearance.line_height, 22.0);
    }

    #[test]
    fn config_path_is_absolute() {
        let path = config_path();
        assert!(path.is_absolute());
        assert!(path.to_string_lossy().contains("config.toml"));
    }

    #[test]
    fn sandbox_defaults_round_trip() {
        let mut config = AppConfig::default();
        config.sandbox.default_cpus = 4;
        config.sandbox.default_memory_mib = 2048;
        config.sandbox.volumes.push(VolumeMountConfig {
            label: "Projects".into(),
            host_path: "/Users/dev/projects".into(),
            guest_path: "/workspace".into(),
        });
        config.sandbox.custom_images.push(CustomImageConfig {
            oci_ref: "myregistry/myimage:v1".into(),
            display_name: "My Image".into(),
            tag: "v1".into(),
            default_shell: "/bin/bash".into(),
            default_workdir: "/app".into(),
            last_pulled: "2026-04-12T12:00:00Z".into(),
        });
        let serialized = toml::to_string_pretty(&config).unwrap();
        let deserialized: AppConfig = toml::from_str(&serialized).unwrap();
        assert_eq!(deserialized.sandbox.default_cpus, 4);
        assert_eq!(deserialized.sandbox.default_memory_mib, 2048);
        assert_eq!(deserialized.sandbox.volumes.len(), 1);
        assert_eq!(deserialized.sandbox.volumes[0].label, "Projects");
        assert_eq!(deserialized.sandbox.custom_images.len(), 1);
        assert_eq!(
            deserialized.sandbox.custom_images[0].display_name,
            "My Image"
        );
        assert_eq!(
            deserialized.sandbox.custom_images[0].last_pulled,
            "2026-04-12T12:00:00Z"
        );
    }
}
