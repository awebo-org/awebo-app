/// High-level AI controller that owns inference state and provides
/// methods for model loading, querying, and hint generation.
///
/// Groups AI-related state (`AiState` + write cursor) into a single
/// struct extracted from the main `App` god object.
use std::sync::atomic::Ordering;

use crate::ai;
use crate::ai::inference::CancelToken;

pub struct AiController {
    pub state: ai::AiState,
    pub block_written: usize,
    pub cancel_token: Option<CancelToken>,
}

impl AiController {
    pub fn new() -> Self {
        Self {
            state: ai::AiState::new(),
            block_written: 0,
            cancel_token: None,
        }
    }

    /// Create a fresh cancel token and store it. Returns a clone for
    /// the inference thread.
    pub fn arm_cancel(&mut self) -> CancelToken {
        let token = ai::inference::new_cancel_token();
        self.cancel_token = Some(token.clone());
        token
    }

    /// Signal the running inference to stop. Returns true if a token
    /// was present and signalled.
    pub fn cancel_inference(&mut self) -> bool {
        if let Some(token) = self.cancel_token.take() {
            token.store(true, Ordering::Relaxed);
            true
        } else {
            false
        }
    }

    /// Resolve the chat template for the currently loaded model,
    /// falling back to "chatml" if unknown.
    pub fn fallback_template(&self) -> &str {
        self.state
            .loaded_model_name
            .as_deref()
            .and_then(|name| {
                ai::registry::MODELS
                    .iter()
                    .find(|m| m.name == name)
                    .map(|m| m.chat_template)
            })
            .unwrap_or("chatml")
    }
}

impl Default for AiController {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_creates_clean_state() {
        let ctrl = AiController::new();
        assert_eq!(ctrl.block_written, 0);
        assert!(ctrl.state.loaded_model.is_none());
        assert!(ctrl.state.inference_rx.is_none());
        assert!(ctrl.cancel_token.is_none());
    }

    #[test]
    fn default_matches_new() {
        let ctrl = AiController::default();
        assert_eq!(ctrl.block_written, 0);
    }

    #[test]
    fn fallback_template_default_is_chatml() {
        let ctrl = AiController::new();
        assert_eq!(ctrl.fallback_template(), "chatml");
    }

    #[test]
    fn arm_cancel_creates_token() {
        let mut ctrl = AiController::new();
        let token = ctrl.arm_cancel();
        assert!(!token.load(Ordering::Relaxed));
        assert!(ctrl.cancel_token.is_some());
    }

    #[test]
    fn cancel_inference_signals_and_clears() {
        let mut ctrl = AiController::new();
        let token = ctrl.arm_cancel();
        assert!(ctrl.cancel_inference());
        assert!(token.load(Ordering::Relaxed));
        assert!(ctrl.cancel_token.is_none());
    }

    #[test]
    fn cancel_inference_returns_false_when_no_token() {
        let mut ctrl = AiController::new();
        assert!(!ctrl.cancel_inference());
    }
}
