use std::borrow::Cow;

use winit::event_loop::ActiveEventLoop;

use crate::app::actions::AppAction;
use crate::commands::PaletteCommand;

impl super::super::App {
    pub(crate) fn filtered_commands(&self) -> Vec<PaletteCommand> {
        let q = self.overlay.palette_query.to_lowercase();
        PaletteCommand::all()
            .iter()
            .filter(|cmd| {
                if q.is_empty() {
                    return true;
                }
                cmd.label().to_lowercase().contains(&q)
            })
            .copied()
            .collect()
    }

    pub(crate) fn execute_command(&mut self, cmd: PaletteCommand, event_loop: &ActiveEventLoop) {
        self.dispatch(AppAction::CloseAllOverlays, event_loop);

        let action = match cmd {
            PaletteCommand::ToggleDebugPanel => AppAction::ToggleDebugPanel,
            PaletteCommand::NewTab => AppAction::CreateTab { shell_path: None },
            PaletteCommand::CloseTab => AppAction::CloseTab {
                index: self.tab_mgr.active_index(),
            },
        };
        self.dispatch(action, event_loop);
    }

    /// Execute a slash command selected from the popup menu.
    pub(crate) fn execute_slash_command(&mut self, name: &str, event_loop: &ActiveEventLoop) {
        self.smart_input.slash_menu_open = false;
        self.smart_input.slash_selected = 0;

        match name {
            "/agent" => {
                self.dispatch(AppAction::EnterAgentMode, event_loop);
                self.smart_input.text.clear();
                self.smart_input.cursor = 0;
            }
            "/close" => {
                self.dispatch(AppAction::ExitAgentMode, event_loop);
                self.smart_input.text.clear();
                self.smart_input.cursor = 0;
            }
            "/clear" => {
                if let Some(tab) = self.tab_mgr.get_mut(self.tab_mgr.active_index())
                    && let super::super::TabKind::Terminal {
                        terminal,
                        block_list,
                        ..
                    } = &mut tab.kind
                {
                    terminal.input(Cow::Borrowed(b"clear\n"));
                    block_list.blocks.clear();
                    block_list.sync_checkpoint(terminal);
                }
                self.smart_input.text.clear();
                self.smart_input.cursor = 0;
            }
            "/help" => {
                self.show_help_block();
                self.smart_input.text.clear();
                self.smart_input.cursor = 0;
            }
            "/models" => {
                self.dispatch(AppAction::OpenModels, event_loop);
                self.smart_input.text.clear();
                self.smart_input.cursor = 0;
            }
            "/ask" => {
                self.smart_input.text = "/ask ".to_string();
                self.smart_input.cursor = self.smart_input.text.len();
            }
            "/summarize" => {
                self.smart_input.text = "/summarize ".to_string();
                self.smart_input.cursor = self.smart_input.text.len();
            }
            _ => {
                self.smart_input.text = format!("{} ", name);
                self.smart_input.cursor = self.smart_input.text.len();
            }
        }
    }

    /// Create a help block showing available slash commands.
    pub(crate) fn show_help_block(&mut self) {
        if let Some(tab) = self.tab_mgr.get_mut(self.tab_mgr.active_index())
            && let super::super::TabKind::Terminal {
                terminal,
                block_list,
                ..
            } = &mut tab.kind
        {
            let prompt_info = terminal.prompt_info();
            block_list.push_command(prompt_info, "/help".to_string());
            block_list.append_output_text(
                    "Available commands:\n  /agent <task>    \u{2014} Enter agent mode + start task\n  /close           \u{2014} Exit agent mode\n  /ask <query>     \u{2014} Ask AI a question\n  /summarize      \u{2014} Summarize recent terminal output\n  /clear           \u{2014} Clear terminal and blocks\n  /models          \u{2014} Open model repository\n  /help            \u{2014} Show this help",
                );
            block_list.finish_last();
            self.record_last_block();
        }
    }
}
