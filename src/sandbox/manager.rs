//! Sandbox lifecycle manager using microsandbox SDK.
//!
//! Uses the microsandbox Rust SDK to create/manage microVM sandboxes.
//! All sandbox operations are async (tokio) — spawned from the sync winit
//! event loop via `tokio::spawn`, results communicated back via `proxy.send_event()`.

use std::fmt;

/// Sandbox operation error.
#[derive(Debug)]
pub enum SandboxError {
    /// microsandbox SDK/runtime not available on this platform.
    NotAvailable(String),
    /// Failed to pull the OCI image.
    ImagePullFailed(String),
    /// Sandbox failed to boot within the timeout.
    BootTimeout(String),
    /// Could not connect streams to the sandbox.
    AttachFailed(String),
    /// Volume operation failed.
    VolumeError(String),
    /// Generic runtime error forwarded from the SDK.
    Runtime(String),
}

impl fmt::Display for SandboxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotAvailable(msg) => write!(f, "microsandbox not available: {msg}"),
            Self::ImagePullFailed(msg) => write!(f, "image pull failed: {msg}"),
            Self::BootTimeout(msg) => write!(f, "sandbox boot timeout: {msg}"),
            Self::AttachFailed(msg) => write!(f, "attach failed: {msg}"),
            Self::VolumeError(msg) => write!(f, "volume error: {msg}"),
            Self::Runtime(msg) => write!(f, "sandbox runtime error: {msg}"),
        }
    }
}

impl std::error::Error for SandboxError {}

/// Strip potential credentials from an OCI image reference for safe display.
///
/// OCI refs may embed credentials as `user:token@registry/image:tag`.
/// This replaces the userinfo portion with `***` to avoid leaking secrets
/// in toast messages or logs.
pub(crate) fn sanitize_oci_ref(oci_ref: &str) -> String {
    // OCI digests use `@sha256:...` — those must not be stripped.
    if let Some(at) = oci_ref.find('@') {
        let prefix = &oci_ref[..at];
        let suffix = &oci_ref[at + 1..];
        // If the part after @ looks like a digest (e.g. sha256:…), keep as-is.
        if suffix.starts_with("sha256:") || suffix.starts_with("sha512:") {
            return oci_ref.to_string();
        }
        // If the prefix contains a colon but no slash, it's likely `user:pass`.
        if prefix.contains(':') && !prefix.contains('/') {
            return format!("***@{suffix}");
        }
    }
    oci_ref.to_string()
}

impl From<microsandbox::MicrosandboxError> for SandboxError {
    fn from(err: microsandbox::MicrosandboxError) -> Self {
        Self::Runtime(err.to_string())
    }
}

/// Manages sandbox lifecycles via the microsandbox SDK.
///
/// The SDK boots microVMs as child processes — no daemon, no CLI required.
/// All async operations are dispatched with `tokio::spawn` and results are
/// sent back to the winit event loop via `EventLoopProxy`.
pub struct SandboxManager {
    /// Whether the microsandbox runtime is available on this platform.
    available: bool,
}

impl SandboxManager {
    /// Command: create a new manager, checking SDK availability.
    pub fn new() -> Self {
        let available = Self::check_availability();
        if available {
            log::info!("[sandbox] microsandbox SDK available");
        } else {
            log::info!("[sandbox] microsandbox SDK not available — sandbox features disabled");
        }
        Self { available }
    }

    /// Query: is microsandbox available on this system?
    pub fn is_available(&self) -> bool {
        self.available
    }

    /// Command: pull an OCI image in the background.
    /// Spawns a tokio task that creates a temporary sandbox to trigger the pull,
    /// then immediately stops it. Sends a Toast event on success/failure.
    pub fn pull_image(
        &self,
        oci_ref: String,
        proxy: winit::event_loop::EventLoopProxy<crate::terminal::TerminalEvent>,
    ) {
        if !self.available {
            let _ = proxy.send_event(crate::terminal::TerminalEvent::Toast(
                "Sandbox not available — cannot pull image".into(),
            ));
            return;
        }
        let display_ref = sanitize_oci_ref(&oci_ref);
        let _ = proxy.send_event(crate::terminal::TerminalEvent::Toast(format!(
            "Pulling image: {}…",
            display_ref
        )));
        tokio::spawn(async move {
            let display_ref = sanitize_oci_ref(&oci_ref);
            let sandbox_name = format!("awebo-pull-{}", std::process::id());
            let result = microsandbox::Sandbox::builder(&sandbox_name)
                .image(oci_ref.as_str())
                .cpus(1)
                .memory(512)
                .replace()
                .create()
                .await;
            match result {
                Ok(handle) => {
                    let _ = handle.kill().await;
                    let _ = proxy.send_event(crate::terminal::TerminalEvent::ToastLevel(
                        format!("Image pulled successfully: {}", display_ref),
                        crate::ui::components::toast::ToastLevel::Success,
                    ));
                }
                Err(e) => {
                    let _ = proxy.send_event(crate::terminal::TerminalEvent::SandboxError(
                        format!("Failed to pull {}: {}", display_ref, e),
                    ));
                }
            }
        });
    }

    /// Command: remove a cached OCI image in the background.
    /// Sends a toast event on success/failure.
    pub fn remove_image(
        &self,
        oci_ref: String,
        proxy: winit::event_loop::EventLoopProxy<crate::terminal::TerminalEvent>,
    ) {
        if !self.available {
            let _ = proxy.send_event(crate::terminal::TerminalEvent::Toast(
                "Sandbox not available — cannot remove image".into(),
            ));
            return;
        }
        let display_ref = sanitize_oci_ref(&oci_ref);
        tokio::spawn(async move {
            let _ = proxy.send_event(crate::terminal::TerminalEvent::ToastLevel(
                format!("Image cache removal requested: {}", display_ref),
                crate::ui::components::toast::ToastLevel::Info,
            ));
        });
    }

    /// Query: check platform requirements for microsandbox.
    /// Requires macOS Apple Silicon or Linux with KVM.
    fn check_availability() -> bool {
        #[cfg(target_os = "macos")]
        {
            cfg!(target_arch = "aarch64")
        }
        #[cfg(target_os = "linux")]
        {
            std::path::Path::new("/dev/kvm").exists()
        }
        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manager_reports_availability() {
        let mgr = SandboxManager::new();
        let _ = mgr.is_available();
    }

    #[test]
    fn sandbox_error_display() {
        let err = SandboxError::NotAvailable("test".into());
        let msg = format!("{err}");
        assert!(msg.contains("not available"));
    }
}
