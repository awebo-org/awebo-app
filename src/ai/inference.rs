use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;

use llama_cpp_2::model::AddBos;
use llama_cpp_2::sampling::LlamaSampler;
use winit::event_loop::EventLoopProxy;

use crate::ai::model_manager::LoadedModelHandle;
use crate::terminal::TerminalEvent;

pub struct InferenceResult {
    pub handle: LoadedModelHandle,
    pub prompt_tokens: usize,
    pub generated_tokens: usize,
}

/// Shared cancellation flag — set to `true` to abort a running inference.
pub type CancelToken = Arc<AtomicBool>;

pub fn new_cancel_token() -> CancelToken {
    Arc::new(AtomicBool::new(false))
}

pub fn run_inference(
    handle: LoadedModelHandle,
    prompt: String,
    token_tx: mpsc::Sender<String>,
    proxy: EventLoopProxy<TerminalEvent>,
    cancel: CancelToken,
) -> tokio::task::JoinHandle<Option<InferenceResult>> {
    tokio::task::spawn_blocking(move || do_inference(handle, prompt, token_tx, proxy, cancel))
}

/// Public wrapper for `do_inference` — used when the caller manages its own thread.
pub fn do_inference_pub(
    handle: LoadedModelHandle,
    prompt: String,
    token_tx: mpsc::Sender<String>,
    proxy: EventLoopProxy<TerminalEvent>,
    cancel: CancelToken,
) -> Option<InferenceResult> {
    do_inference(handle, prompt, token_tx, proxy, cancel)
}

fn do_inference(
    mut handle: LoadedModelHandle,
    prompt: String,
    token_tx: mpsc::Sender<String>,
    proxy: EventLoopProxy<TerminalEvent>,
    cancel: CancelToken,
) -> Option<InferenceResult> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);

    let tokens = match handle.model.str_to_token(&prompt, AddBos::Never) {
        Ok(t) => t,
        Err(e) => {
            log::error!("Tokenization failed: {e}");
            let _ = proxy.send_event(TerminalEvent::AiError(format!("Tokenization failed: {e}")));
            let _ = token_tx.send(String::new());
            return Some(InferenceResult {
                handle,
                prompt_tokens: 0,
                generated_tokens: 0,
            });
        }
    };

    let max_gen = 2048usize;
    let n_ctx = handle.n_ctx as usize;
    let max_prompt = n_ctx.saturating_sub(max_gen).max(64);

    let tokens = if tokens.len() > max_prompt {
        log::warn!(
            "Prompt truncated from {} to {} tokens (n_ctx={})",
            tokens.len(),
            max_prompt,
            n_ctx
        );
        tokens[tokens.len() - max_prompt..].to_vec()
    } else {
        tokens
    };

    let n_tokens = tokens.len();
    if n_tokens == 0 {
        let _ = token_tx.send(String::new());
        return Some(InferenceResult {
            handle,
            prompt_tokens: 0,
            generated_tokens: 0,
        });
    }

    handle.context.clear_kv_cache();

    let mut batch = llama_cpp_2::llama_batch::LlamaBatch::new(512, 1);

    let chunk_size = 512;
    for chunk_start in (0..n_tokens).step_by(chunk_size) {
        if cancel.load(Ordering::Relaxed) {
            log::info!("Inference cancelled during prompt ingestion");
            let _ = token_tx.send(String::new());
            let _ = proxy.send_event(TerminalEvent::Wakeup);
            return Some(InferenceResult {
                handle,
                prompt_tokens: n_tokens,
                generated_tokens: 0,
            });
        }
        batch.clear();
        let chunk_end = (chunk_start + chunk_size).min(n_tokens);
        for i in chunk_start..chunk_end {
            let is_last = i == n_tokens - 1;
            if batch.add(tokens[i], i as i32, &[0], is_last).is_err() {
                log::error!("Failed to add token to batch");
                let _ = token_tx.send(String::new());
                return Some(InferenceResult {
                    handle,
                    prompt_tokens: n_tokens,
                    generated_tokens: 0,
                });
            }
        }
        if handle.context.decode(&mut batch).is_err() {
            log::error!("Decode failed during prompt ingestion");
            let _ = token_tx.send(String::new());
            return Some(InferenceResult {
                handle,
                prompt_tokens: n_tokens,
                generated_tokens: 0,
            });
        }
    }

    let mut sampler = LlamaSampler::chain_simple([
        LlamaSampler::temp(0.6),
        LlamaSampler::top_p(0.95, 1),
        LlamaSampler::dist(42),
    ]);

    let max_tokens = max_gen.min(n_ctx.saturating_sub(n_tokens));
    let mut n_decoded = 0usize;
    let mut pos = n_tokens;

    let eos = handle.model.token_eos();
    let mut decoder = encoding_rs::UTF_8.new_decoder();

    let mut buf = String::new();
    let mut in_think = false;

    loop {
        if n_decoded >= max_tokens {
            break;
        }

        if cancel.load(Ordering::Relaxed) {
            log::info!("Inference cancelled after {n_decoded} tokens");
            break;
        }

        if std::time::Instant::now() >= deadline {
            log::warn!("Inference timed out after 120s ({n_decoded} tokens generated)");
            let _ = proxy.send_event(TerminalEvent::AiError(
                "Inference timed out (120s limit)".into(),
            ));
            break;
        }

        let token = sampler.sample(&handle.context, batch.n_tokens() - 1);
        sampler.accept(token);

        if token == eos || handle.model.is_eog_token(token) {
            break;
        }

        let piece = match handle
            .model
            .token_to_piece(token, &mut decoder, false, None)
        {
            Ok(p) => p,
            Err(_) => String::new(),
        };

        if !piece.is_empty() {
            buf.push_str(&piece);

            loop {
                if in_think {
                    let end_think = buf.find("</think>");
                    let end_channel = buf.find("<channel|>");
                    let (end_pos, tag_len) = match (end_think, end_channel) {
                        (Some(a), Some(b)) if a <= b => (Some(a), 8),
                        (Some(_), Some(b)) => (Some(b), 10),
                        (Some(a), None) => (Some(a), 8),
                        (None, Some(b)) => (Some(b), 10),
                        (None, None) => (None, 0),
                    };
                    if let Some(end) = end_pos {
                        buf = buf[end + tag_len..].to_string();
                        in_think = false;
                        continue;
                    }
                    buf.clear();
                    break;
                }

                let start_think = buf.find("<think>");
                let start_channel = buf.find("<|channel>");
                let (start_pos, tag_len) = match (start_think, start_channel) {
                    (Some(a), Some(b)) if a <= b => (Some(a), 7),
                    (Some(_), Some(b)) => (Some(b), 10),
                    (Some(a), None) => (Some(a), 7),
                    (None, Some(b)) => (Some(b), 10),
                    (None, None) => (None, 0),
                };

                if let Some(start) = start_pos {
                    let before = &buf[..start];
                    if !before.is_empty() {
                        if token_tx.send(before.to_string()).is_err() {
                            let _ = token_tx.send(String::new());
                            let _ = proxy.send_event(TerminalEvent::Wakeup);
                            return Some(InferenceResult {
                                handle,
                                prompt_tokens: n_tokens,
                                generated_tokens: n_decoded,
                            });
                        }
                        let _ = proxy.send_event(TerminalEvent::Wakeup);
                    }
                    buf = buf[start + tag_len..].to_string();
                    in_think = true;
                    continue;
                }

                if buf.len() > 10 {
                    let target = buf.len() - 10;
                    let safe = buf.floor_char_boundary(target);
                    if safe > 0 {
                        let to_send = buf[..safe].to_string();
                        buf = buf[safe..].to_string();
                        if token_tx.send(to_send).is_err() {
                            let _ = token_tx.send(String::new());
                            let _ = proxy.send_event(TerminalEvent::Wakeup);
                            return Some(InferenceResult {
                                handle,
                                prompt_tokens: n_tokens,
                                generated_tokens: n_decoded,
                            });
                        }
                        let _ = proxy.send_event(TerminalEvent::Wakeup);
                    }
                }
                break;
            }
        }

        batch.clear();
        if batch.add(token, pos as i32, &[0], true).is_err() {
            break;
        }
        pos += 1;

        if handle.context.decode(&mut batch).is_err() {
            log::error!("Decode failed at token {n_decoded}");
            break;
        }

        n_decoded += 1;
    }

    if !in_think && !buf.is_empty() {
        let _ = token_tx.send(buf);
    }

    let _ = token_tx.send(String::new());
    let _ = proxy.send_event(TerminalEvent::Wakeup);
    Some(InferenceResult {
        handle,
        prompt_tokens: n_tokens,
        generated_tokens: n_decoded,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancel_token_starts_false() {
        let token = new_cancel_token();
        assert!(!token.load(Ordering::Relaxed));
    }

    #[test]
    fn cancel_token_shared_signal() {
        let token = new_cancel_token();
        let clone = token.clone();
        token.store(true, Ordering::Relaxed);
        assert!(clone.load(Ordering::Relaxed));
    }
}
