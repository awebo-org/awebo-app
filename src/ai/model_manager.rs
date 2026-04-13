use std::path::PathBuf;
use std::sync::mpsc;

/// Return the directory where downloaded GGUF model files are stored.
pub fn models_dir() -> PathBuf {
    let base = dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("."));
    base.join("awebo").join("models")
}

pub struct LoadedModelHandle {
    pub _backend: llama_cpp_2::llama_backend::LlamaBackend,
    pub model: Box<llama_cpp_2::model::LlamaModel>,
    pub context: llama_cpp_2::context::LlamaContext<'static>,
    pub n_ctx: u32,
}

/// SAFETY: llama.cpp context is not thread-bound. The crate omits Send because of
/// NonNull<llama_context>, but the underlying C API is safe to use from any thread
/// as long as access is single-threaded (which it is — we move the handle, not share it).
unsafe impl Send for LoadedModelHandle {}

/// Progress update sent from the download thread to the UI.
#[derive(Debug, Clone)]
pub struct DownloadProgress {
    pub bytes_downloaded: u64,
    pub bytes_total: u64,
    pub finished: bool,
    pub error: Option<String>,
}

impl DownloadProgress {
    /// Returns download progress as a fraction 0.0–1.0.
    pub fn fraction(&self) -> f64 {
        if self.bytes_total == 0 {
            0.0
        } else {
            self.bytes_downloaded as f64 / self.bytes_total as f64
        }
    }

    /// Returns download progress as a percentage 0–100.
    pub fn percent(&self) -> u8 {
        (self.fraction() * 100.0).min(100.0) as u8
    }
}

/// Download a GGUF model file from HuggingFace to `dest_dir`.
///
/// Sends `DownloadProgress` updates through `progress_tx`.
/// Also sends a `TerminalEvent::Wakeup` to trigger UI redraws.
pub fn download_model(
    hf_repo: &str,
    hf_filename: &str,
    model_name: &str,
    dest_dir: &std::path::Path,
    progress_tx: mpsc::Sender<DownloadProgress>,
    proxy: winit::event_loop::EventLoopProxy<crate::terminal::TerminalEvent>,
) {
    let url = format!(
        "https://huggingface.co/{}/resolve/main/{}",
        hf_repo, hf_filename
    );
    let dest = dest_dir.join(hf_filename);
    let model_name = model_name.to_string();

    tokio::task::spawn_blocking(move || {
        let send_progress = |dl: u64, total: u64, finished: bool, error: Option<String>| {
            let _ = progress_tx.send(DownloadProgress {
                bytes_downloaded: dl,
                bytes_total: total,
                finished,
                error,
            });
            let _ = proxy.send_event(crate::terminal::TerminalEvent::Wakeup);
        };

        log::info!("Downloading {} from {}", model_name, url);
        send_progress(0, 0, false, None);

        if let Some(parent) = dest.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        let response = match ureq::get(&url).call() {
            Ok(r) => r,
            Err(e) => {
                log::error!("Download request failed: {e}");
                send_progress(0, 0, true, Some(format!("Request failed: {e}")));
                return;
            }
        };

        let content_length: u64 = response
            .headers()
            .get("content-length")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        let mut reader = response.into_body().into_reader();
        let tmp_dest = dest.with_extension("gguf.part");
        let mut file = match std::fs::File::create(&tmp_dest) {
            Ok(f) => f,
            Err(e) => {
                log::error!("Failed to create file: {e}");
                send_progress(
                    0,
                    content_length,
                    true,
                    Some(format!("File create error: {e}")),
                );
                return;
            }
        };

        let mut downloaded: u64 = 0;
        let mut buf = vec![0u8; 256 * 1024];
        let mut last_report = std::time::Instant::now();

        loop {
            let n = match std::io::Read::read(&mut reader, &mut buf) {
                Ok(0) => break,
                Ok(n) => n,
                Err(e) => {
                    log::error!("Download read error: {e}");
                    send_progress(
                        downloaded,
                        content_length,
                        true,
                        Some(format!("Read error: {e}")),
                    );
                    let _ = std::fs::remove_file(&tmp_dest);
                    return;
                }
            };

            if std::io::Write::write_all(&mut file, &buf[..n]).is_err() {
                send_progress(downloaded, content_length, true, Some("Write error".into()));
                let _ = std::fs::remove_file(&tmp_dest);
                return;
            }

            downloaded += n as u64;

            if last_report.elapsed().as_millis() >= 100 {
                send_progress(downloaded, content_length, false, None);
                last_report = std::time::Instant::now();
            }
        }

        if let Err(e) = std::fs::rename(&tmp_dest, &dest) {
            log::error!("Failed to rename downloaded file: {e}");
            send_progress(
                downloaded,
                content_length,
                true,
                Some(format!("Rename error: {e}")),
            );
            return;
        }

        log::info!("Download complete: {} ({} bytes)", model_name, downloaded);
        send_progress(downloaded, content_length, true, None);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn models_dir_is_absolute() {
        let dir = models_dir();
        assert!(dir.is_absolute());
    }

    #[test]
    fn download_progress_fraction_zero_total() {
        let p = DownloadProgress {
            bytes_downloaded: 0,
            bytes_total: 0,
            finished: false,
            error: None,
        };
        assert_eq!(p.fraction(), 0.0);
        assert_eq!(p.percent(), 0);
    }

    #[test]
    fn download_progress_fraction_half() {
        let p = DownloadProgress {
            bytes_downloaded: 500,
            bytes_total: 1000,
            finished: false,
            error: None,
        };
        assert!((p.fraction() - 0.5).abs() < f64::EPSILON);
        assert_eq!(p.percent(), 50);
    }

    #[test]
    fn download_progress_fraction_complete() {
        let p = DownloadProgress {
            bytes_downloaded: 1000,
            bytes_total: 1000,
            finished: true,
            error: None,
        };
        assert!((p.fraction() - 1.0).abs() < f64::EPSILON);
        assert_eq!(p.percent(), 100);
    }
}
