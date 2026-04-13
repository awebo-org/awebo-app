/// Command palette entries available via Cmd+P.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PaletteCommand {
    ToggleDebugPanel,
    NewTab,
    CloseTab,
}

impl PaletteCommand {
    pub fn label(&self) -> &'static str {
        match self {
            PaletteCommand::ToggleDebugPanel => "Toggle Debug Panel",
            PaletteCommand::NewTab => "New Tab",
            PaletteCommand::CloseTab => "Close Tab",
        }
    }

    pub fn shortcut(&self) -> &'static str {
        match self {
            PaletteCommand::ToggleDebugPanel => "",
            PaletteCommand::NewTab => "Cmd+T",
            PaletteCommand::CloseTab => "Cmd+W",
        }
    }

    pub fn all() -> &'static [PaletteCommand] {
        &[
            PaletteCommand::ToggleDebugPanel,
            PaletteCommand::NewTab,
            PaletteCommand::CloseTab,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_returns_all_variants() {
        let all = PaletteCommand::all();
        assert_eq!(all.len(), 3);
    }

    #[test]
    fn label_not_empty() {
        for cmd in PaletteCommand::all() {
            assert!(!cmd.label().is_empty());
        }
    }

    #[test]
    fn shortcut_defined_for_tab_commands() {
        assert!(!PaletteCommand::NewTab.shortcut().is_empty());
        assert!(!PaletteCommand::CloseTab.shortcut().is_empty());
    }
}
