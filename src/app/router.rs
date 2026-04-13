//! Internal screen router.
//!
//! Manages which "screen" is currently active. The terminal grid is the
//! default; overlays like Settings are full-screen routes that replace it.
//!
//! Designed for easy extension — add a variant to [`Route`] and handle it
//! in the render / input dispatch.

/// A screen the app can navigate to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    /// Normal terminal view (grid / block view + prompt bar).
    Terminal,
    /// Full-screen settings panel.
    Settings,
    /// Full-screen model repository browser.
    Models,
    /// In-app file editor (text / image / hex).
    Editor,
}

/// Minimal stack-based router.
///
/// Keeps a history stack so `back()` can return to the previous screen.
/// The bottom of the stack is always `Route::Terminal`.
pub struct Router {
    stack: Vec<Route>,
}

impl Router {
    pub fn new() -> Self {
        Self {
            stack: vec![Route::Terminal],
        }
    }

    /// The currently active route.
    pub fn current(&self) -> Route {
        *self.stack.last().unwrap_or(&Route::Terminal)
    }

    /// Replace the current route (no stack growth).
    pub fn replace(&mut self, route: Route) {
        if let Some(top) = self.stack.last_mut() {
            *top = route;
        }
    }
}

impl Default for Router {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_route_is_terminal() {
        let r = Router::new();
        assert_eq!(r.current(), Route::Terminal);
    }

    #[test]
    fn replace_swaps_top() {
        let mut r = Router::new();
        r.replace(Route::Settings);
        assert_eq!(r.current(), Route::Settings);
        r.replace(Route::Terminal);
        assert_eq!(r.current(), Route::Terminal);
    }
}
