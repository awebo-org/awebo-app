//! PTY bridge between microsandbox SDK exec streams and the winit event loop.
//!
//! The microsandbox SDK provides async `exec_stream` for interactive shell
//! sessions inside microVMs. This module bridges those async streams into
//! the synchronous winit event loop via `tokio::spawn` + `proxy.send_event()`.
//!
//! Flow:
//! 1. `SandboxBridge::spawn()` → `tokio::spawn` creates sandbox + exec_stream
//! 2. Stdout/stderr events are forwarded as `TerminalEvent::SandboxOutput`
//! 3. Stdin is written via a `tokio::sync::mpsc` channel from the sync side

use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use alacritty_terminal::Term;
use alacritty_terminal::event::WindowSize;
use alacritty_terminal::sync::FairMutex;
use alacritty_terminal::term::Config;
use alacritty_terminal::vte::ansi;

use crate::terminal::{JsonEventProxy, TerminalEvent};

/// Handle for writing stdin to a running sandbox shell.
/// The bridge reads from the receiver side inside a tokio task.
pub type StdinSender = tokio::sync::mpsc::UnboundedSender<Vec<u8>>;

/// Per-layer pull progress snapshot.
#[derive(Debug, Clone)]
pub struct LayerProgress {
    pub phase: LayerPhase,
    pub downloaded: u64,
    pub total: Option<u64>,
    pub extracted: u64,
    pub extract_total: u64,
}

/// Current phase of a single layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerPhase {
    Waiting,
    Downloading,
    Downloaded,
    Extracting,
    Done,
}

/// Shared pull progress state (behind a Mutex for cheap reads from the render thread).
#[derive(Debug, Clone)]
pub struct PullState {
    pub phase: PullPhase,
    pub layers: Vec<LayerProgress>,
}

/// High-level pull phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PullPhase {
    Resolving,
    Pulling,
    Complete,
}

/// Volume snapshot for display in the side panel.
#[derive(Debug, Clone)]
pub struct VolumeMountInfo {
    /// Guest path inside the sandbox.
    pub guest_path: String,
    /// Host path on the host machine.
    pub host_path: String,
}

/// A running sandbox terminal bridge.
///
/// Owns an alacritty `Term` for rendering (escape sequence processing)
/// and a channel for sending user keyboard input to the sandbox.
pub struct SandboxBridge {
    /// Alacritty term for terminal emulation (escape sequence → grid).
    pub term: Arc<FairMutex<Term<JsonEventProxy>>>,
    /// Channel to send stdin bytes to the sandbox process.
    pub stdin_tx: StdinSender,
    /// The tokio task running the I/O loop.
    task_handle: tokio::task::JoinHandle<()>,
    /// Display name for the tab title.
    pub display_name: String,
    /// Number of virtual CPUs allocated.
    pub cpus: u32,
    /// Memory allocated in MiB.
    pub memory_mib: u32,
    /// Volume mounts active in this sandbox.
    pub volumes: Vec<VolumeMountInfo>,
    /// True while the sandbox is being created / image pulled.
    initializing: Arc<AtomicBool>,
    /// Pull progress state (shared with the async task).
    pull_state: Arc<Mutex<PullState>>,
}

impl SandboxBridge {
    /// Command: spawn a sandbox shell and return the bridge.
    ///
    /// Creates a microsandbox VM, starts an interactive shell via
    /// `exec_stream_with` (with TTY + stdin pipe), and bridges I/O
    /// to an alacritty `Term` for rendering.
    pub fn spawn(
        config: super::config::SandboxConfig,
        cols: u16,
        lines: u16,
        cell_width: u16,
        cell_height: u16,
        event_proxy: JsonEventProxy,
        manager: &super::manager::SandboxManager,
    ) -> Result<Self, super::manager::SandboxError> {
        if !manager.is_available() {
            return Err(super::manager::SandboxError::NotAvailable(
                "platform requirements not met".into(),
            ));
        }

        let display_name = super::images::image_by_id(&config.image_id)
            .map(|i| i.display_name.to_string())
            .unwrap_or_else(|| config.image_id.clone());

        let term_config = Config::default();
        let window_size = WindowSize {
            num_cols: cols,
            num_lines: lines,
            cell_width,
            cell_height,
        };

        let term_size = crate::terminal::TermSize(window_size);
        let term = Term::new(term_config, &term_size, event_proxy.clone());
        let term = Arc::new(FairMutex::new(term));

        let (stdin_tx, stdin_rx) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();

        let term_clone = term.clone();
        let proxy = event_proxy.proxy.clone();
        let display_for_task = display_name.clone();

        let cpus = config.cpus;
        let memory_mib = config.memory_mib;
        let volumes: Vec<VolumeMountInfo> = config
            .volumes
            .iter()
            .map(|v| VolumeMountInfo {
                guest_path: v.guest_path.clone(),
                host_path: v.host_path.to_string_lossy().into_owned(),
            })
            .collect();

        let initializing = Arc::new(AtomicBool::new(true));
        let init_flag = initializing.clone();

        let pull_state = Arc::new(Mutex::new(PullState {
            phase: PullPhase::Resolving,
            layers: Vec::new(),
        }));
        let pull_state_clone = pull_state.clone();

        let task_handle = tokio::spawn(async move {
            match Self::io_loop(
                config,
                term_clone,
                stdin_rx,
                proxy.clone(),
                init_flag,
                pull_state_clone,
            )
            .await
            {
                Ok(()) => {}
                Err(e) => {
                    let msg = format!("Sandbox '{}' error: {}", display_for_task, e);
                    log::error!("[sandbox] {}", msg);
                    let _ = proxy.send_event(TerminalEvent::SandboxError(msg));
                }
            }
        });

        Ok(Self {
            term,
            stdin_tx,
            task_handle,
            display_name,
            cpus,
            memory_mib,
            volumes,
            initializing,
            pull_state,
        })
    }

    /// Send user input to the sandbox's stdin.
    pub fn input(&self, data: Vec<u8>) {
        let _ = self.stdin_tx.send(data);
    }

    /// Query: whether the sandbox is still pulling the image / booting the VM.
    pub fn is_initializing(&self) -> bool {
        self.initializing.load(Ordering::Relaxed)
    }

    /// Query: snapshot of pull progress for rendering.
    pub fn pull_progress(&self) -> PullState {
        self.pull_state.lock().unwrap().clone()
    }

    /// Query: whether the bridge task is still running.
    pub fn is_alive(&self) -> bool {
        !self.task_handle.is_finished()
    }

    /// The core async I/O loop: creates sandbox, attaches exec_stream,
    /// forwards stdout→Term and stdin_rx→sandbox.
    async fn io_loop(
        config: super::config::SandboxConfig,
        term: Arc<FairMutex<Term<JsonEventProxy>>>,
        mut stdin_rx: tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>,
        proxy: winit::event_loop::EventLoopProxy<TerminalEvent>,
        initializing: Arc<AtomicBool>,
        pull_state: Arc<Mutex<PullState>>,
    ) -> Result<(), super::manager::SandboxError> {
        let image = super::images::image_by_id(&config.image_id);
        let oci_ref = image.map(|i| i.oci_ref).unwrap_or(&config.image_id);

        let sandbox_name = format!("awebo-{}-{}", config.image_id, std::process::id());

        let mut builder = microsandbox::Sandbox::builder(&sandbox_name)
            .image(oci_ref)
            .cpus(config.cpus as u8)
            .memory(config.memory_mib)
            .workdir(&config.workdir)
            .replace();

        for (key, val) in &config.env {
            builder = builder.env(key, val);
        }

        for vol in &config.volumes {
            let host = vol.host_path.clone();
            let guest = vol.guest_path.clone();
            builder = builder.volume(&guest, move |m| m.bind(host));
        }

        let _ = proxy.send_event(TerminalEvent::ToastLevel(
            format!("Pulling image '{}'...", oci_ref),
            crate::ui::components::toast::ToastLevel::Info,
        ));
        let _ = proxy.send_event(TerminalEvent::Wakeup);

        log::info!(
            "[sandbox] Creating VM '{}' with pull progress...",
            sandbox_name
        );

        let (mut progress_handle, create_task) =
            builder.create_with_pull_progress().map_err(|e| {
                initializing.store(false, Ordering::Relaxed);
                super::manager::SandboxError::Runtime(e.to_string())
            })?;

        let ps = pull_state.clone();
        let progress_proxy = proxy.clone();
        let progress_task = tokio::spawn(async move {
            while let Some(evt) = progress_handle.recv().await {
                Self::apply_pull_event(&ps, &evt);
                let _ = progress_proxy.send_event(TerminalEvent::Wakeup);
            }
        });

        let sandbox = create_task
            .await
            .map_err(|e| {
                initializing.store(false, Ordering::Relaxed);
                super::manager::SandboxError::Runtime(format!("task join: {}", e))
            })?
            .map_err(|e| {
                initializing.store(false, Ordering::Relaxed);
                let msg = e.to_string();
                if msg.contains("pull") || msg.contains("image") {
                    super::manager::SandboxError::ImagePullFailed(msg)
                } else if msg.contains("timeout") {
                    super::manager::SandboxError::BootTimeout(msg)
                } else if msg.contains("volume") || msg.contains("mount") {
                    super::manager::SandboxError::VolumeError(msg)
                } else {
                    super::manager::SandboxError::Runtime(msg)
                }
            })?;

        let _ = progress_task.await;

        log::info!("[sandbox] VM '{}' ready, starting shell...", sandbox.name());

        let shell = config
            .env
            .iter()
            .find(|(k, _)| k == "SHELL")
            .map(|(_, v)| v.as_str())
            .unwrap_or("/bin/sh");

        let mut exec_handle = sandbox
            .exec_stream_with(shell, |e| e.stdin_pipe().tty(true).args(["-l"]))
            .await
            .map_err(|e| {
                initializing.store(false, Ordering::Relaxed);
                super::manager::SandboxError::AttachFailed(e.to_string())
            })?;

        let stdin_sink = exec_handle.take_stdin().ok_or_else(|| {
            initializing.store(false, Ordering::Relaxed);
            super::manager::SandboxError::AttachFailed("no stdin sink".into())
        })?;

        initializing.store(false, Ordering::Relaxed);
        log::info!("[sandbox] Shell attached, bridging I/O...");

        let mut parser = ansi::Processor::<ansi::StdSyncHandler>::new();

        let _ = proxy.send_event(TerminalEvent::Toast(format!(
            "Sandbox '{}' ready",
            sandbox_name
        )));
        let _ = proxy.send_event(TerminalEvent::Wakeup);

        loop {
            tokio::select! {
                event = exec_handle.recv() => {
                    match event {
                        Some(microsandbox::sandbox::exec::ExecEvent::Stdout(bytes)) => {
                            {
                                let mut t = term.lock();
                                parser.advance(&mut *t, &bytes);
                            }
                            let _ = proxy.send_event(TerminalEvent::Wakeup);
                        }
                        Some(microsandbox::sandbox::exec::ExecEvent::Stderr(bytes)) => {
                            {
                                let mut t = term.lock();
                                parser.advance(&mut *t, &bytes);
                            }
                            let _ = proxy.send_event(TerminalEvent::Wakeup);
                        }
                        Some(microsandbox::sandbox::exec::ExecEvent::Exited { code }) => {
                            log::info!("[sandbox] Shell exited with code {}", code);
                            let _ = proxy.send_event(TerminalEvent::SandboxExit {
                                name: sandbox_name.clone(),
                                code,
                            });
                            break;
                        }
                        Some(microsandbox::sandbox::exec::ExecEvent::Started { pid }) => {
                            log::info!("[sandbox] Shell started with pid {}", pid);
                        }
                        None => {
                            log::info!("[sandbox] Exec stream ended");
                            let _ = proxy.send_event(TerminalEvent::SandboxExit {
                                name: sandbox_name.clone(),
                                code: 0,
                            });
                            break;
                        }
                    }
                }
                Some(input) = stdin_rx.recv() => {
                    if let Err(e) = stdin_sink.write(&input).await {
                        log::error!("[sandbox] stdin write error: {}", e);
                        break;
                    }
                }
                else => break,
            }
        }

        log::info!("[sandbox] Stopping VM '{}'...", sandbox.name());
        if let Err(e) = sandbox.stop_and_wait().await {
            log::warn!("[sandbox] Stop error (non-fatal): {}", e);
        }

        Ok(())
    }

    /// Apply a single pull progress event to the shared state.
    fn apply_pull_event(state: &Arc<Mutex<PullState>>, evt: &microsandbox::sandbox::PullProgress) {
        use microsandbox::sandbox::PullProgress;
        let mut s = state.lock().unwrap();
        match evt {
            PullProgress::Resolving { .. } => {
                s.phase = PullPhase::Resolving;
            }
            PullProgress::Resolved { layer_count, .. } => {
                s.phase = PullPhase::Pulling;
                s.layers.resize(
                    *layer_count,
                    LayerProgress {
                        phase: LayerPhase::Waiting,
                        downloaded: 0,
                        total: None,
                        extracted: 0,
                        extract_total: 0,
                    },
                );
            }
            PullProgress::LayerDownloadProgress {
                layer_index,
                downloaded_bytes,
                total_bytes,
                ..
            } => {
                if let Some(layer) = s.layers.get_mut(*layer_index) {
                    layer.phase = LayerPhase::Downloading;
                    layer.downloaded = *downloaded_bytes;
                    layer.total = *total_bytes;
                }
            }
            PullProgress::LayerDownloadComplete {
                layer_index,
                downloaded_bytes,
                ..
            } => {
                if let Some(layer) = s.layers.get_mut(*layer_index) {
                    layer.phase = LayerPhase::Downloaded;
                    layer.downloaded = *downloaded_bytes;
                }
            }
            PullProgress::LayerExtractStarted { layer_index, .. } => {
                if let Some(layer) = s.layers.get_mut(*layer_index) {
                    layer.phase = LayerPhase::Extracting;
                }
            }
            PullProgress::LayerExtractProgress {
                layer_index,
                bytes_read,
                total_bytes,
            } => {
                if let Some(layer) = s.layers.get_mut(*layer_index) {
                    layer.phase = LayerPhase::Extracting;
                    layer.extracted = *bytes_read;
                    layer.extract_total = *total_bytes;
                }
            }
            PullProgress::LayerExtractComplete { layer_index, .. }
            | PullProgress::LayerIndexComplete { layer_index } => {
                if let Some(layer) = s.layers.get_mut(*layer_index) {
                    layer.phase = LayerPhase::Done;
                }
            }
            PullProgress::Complete { .. } => {
                s.phase = PullPhase::Complete;
                for layer in &mut s.layers {
                    layer.phase = LayerPhase::Done;
                }
            }
            _ => {}
        }
    }
}

impl Drop for SandboxBridge {
    fn drop(&mut self) {
        self.task_handle.abort();
    }
}

#[cfg(test)]
mod tests {
    use super::super::manager::SandboxError;

    #[test]
    fn sandbox_error_display_not_available() {
        let err = SandboxError::NotAvailable("test reason".into());
        assert!(format!("{err}").contains("not available"));
    }

    #[test]
    fn sandbox_error_lifecycle_variants() {
        let cases: Vec<(SandboxError, &str)> = vec![
            (SandboxError::ImagePullFailed("timeout".into()), "pull"),
            (SandboxError::BootTimeout("slow".into()), "timeout"),
            (SandboxError::AttachFailed("io".into()), "attach"),
            (SandboxError::VolumeError("perm".into()), "volume"),
            (SandboxError::Runtime("crash".into()), "runtime"),
        ];
        for (err, substr) in cases {
            assert!(
                format!("{err}").to_lowercase().contains(substr),
                "Expected '{substr}' in '{err}'"
            );
        }
    }
}
