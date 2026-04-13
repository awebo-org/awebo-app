//! Per-sandbox configuration and volume definitions.
//!
//! Used by the microsandbox SDK to configure sandboxes.

use std::path::PathBuf;

/// A single volume mount point (bind-mount from host).
#[derive(Debug, Clone)]
pub struct VolumeMount {
    /// Path inside the sandbox where the volume appears.
    pub guest_path: String,
    /// Host path to bind-mount into the guest.
    pub host_path: PathBuf,
}

/// Full configuration for creating a sandbox.
#[derive(Debug, Clone)]
pub struct SandboxConfig {
    /// Which trusted image to use (id from `images::IMAGES`).
    pub image_id: String,
    /// Number of virtual CPUs (default: 1).
    pub cpus: u32,
    /// Memory in MiB (default: 512).
    pub memory_mib: u32,
    /// Volume mounts.
    pub volumes: Vec<VolumeMount>,
    /// Working directory inside the sandbox.
    pub workdir: String,
    /// Extra environment variables.
    pub env: Vec<(String, String)>,
}

impl SandboxConfig {
    /// Create a config with explicit OCI ref, shell, and workdir.
    pub fn new(image_id: &str, _oci_ref: &str, shell: &str, workdir: &str) -> Self {
        Self {
            image_id: image_id.to_string(),
            cpus: 1,
            memory_mib: 512,
            volumes: Vec::new(),
            workdir: workdir.to_string(),
            env: vec![
                ("TERM".to_string(), "xterm-256color".to_string()),
                ("COLORTERM".to_string(), "truecolor".to_string()),
                ("SHELL".to_string(), shell.to_string()),
            ],
        }
    }

    /// Command: create a config with sensible defaults for the given built-in image.
    pub fn for_image(image_id: &str) -> Self {
        let image = super::images::image_by_id(image_id);
        let (workdir, shell) = image
            .map(|i| (i.default_workdir.to_string(), i.default_shell.to_string()))
            .unwrap_or_else(|| ("/root".to_string(), "/bin/sh".to_string()));
        Self::new(image_id, "", &shell, &workdir)
    }

    /// Command: add a bind-mount from host CWD to /workspace.
    pub fn mount_workspace(&mut self, host_path: PathBuf) {
        self.volumes.push(VolumeMount {
            guest_path: "/workspace".to_string(),
            host_path,
        });
        self.workdir = "/workspace".to_string();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_for_alpine() {
        let cfg = SandboxConfig::for_image("alpine");
        assert_eq!(cfg.image_id, "alpine");
        assert_eq!(cfg.cpus, 1);
        assert_eq!(cfg.memory_mib, 512);
        assert_eq!(cfg.workdir, "/root");
        assert!(cfg.volumes.is_empty());
    }

    #[test]
    fn mount_workspace_updates_workdir() {
        let mut cfg = SandboxConfig::for_image("python-dev");
        cfg.mount_workspace(PathBuf::from("/Users/dev/project"));
        assert_eq!(cfg.workdir, "/workspace");
        assert_eq!(cfg.volumes.len(), 1);
        assert_eq!(cfg.volumes[0].guest_path, "/workspace");
    }
}
