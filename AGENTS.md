# Agents Guide

Instructions for AI coding agents working on the Awebo codebase.

## Project Overview

Awebo is a GPU-accelerated terminal emulator with built-in AI, sandboxed environments, and git integration. Single Rust binary, ~43k lines across 102 source files. Targets macOS primarily (Linux planned).

## Build & Test

```bash
cargo build              # dev build
cargo build --release    # optimized build
cargo test               # run all 501+ unit tests
cargo fmt -- --check     # formatting (must pass before commit)
cargo clippy -- -D warnings  # lints (must pass before commit)
```

All four commands must pass cleanly before any commit.

The build script (`build.rs`) compiles tree-sitter grammars from `vendor/grammars/`. Adding a language requires only a new directory there with `grammar.toml`, `parser.c`, and optional `scanner.c` - no Rust changes needed.

## Architecture

```
src/
  main.rs              # Entry point, event loop setup
  app/                 # Application state, tab management, input routing
    mod.rs             # App struct (winit ApplicationHandler)
    tabs.rs            # TabManager, Tab, TabKind
    actions.rs         # Action dispatch (keyboard shortcuts, menu commands)
    router.rs          # View routing
    views/             # Per-tab view logic (smart terminal, sandbox, agent, etc.)
  renderer/            # Pixel-based rendering pipeline
    mod.rs             # Renderer: orchestrates all drawing, manages pixel buffer
    backend.rs         # GPU (wgpu) and CPU (softbuffer) backends
    pixel_buffer.rs    # PixelBuffer: fill_rect, blit, alpha blending
    glyph_atlas.rs     # Glyph caching and rasterization (cosmic-text + swash)
    gpu_grid.rs        # Instanced GPU grid rendering for terminal cells
    theme.rs           # Color constants (all colors defined here)
    text.rs            # Text measurement and rendering helpers
    icons.rs           # SVG icon loading and rendering (resvg)
  terminal/            # PTY wrapper around alacritty_terminal
  ui/                  # UI components (stateless drawing functions)
    components/        # Tab bar, grid, git panel, prompt bar, overlays, toasts
    widgets/           # Reusable widgets (dropdown, tooltip, text input, search)
    layout.rs          # Layout calculations
    editor.rs          # Text editor component
    markdown.rs        # Markdown rendering
    syntax/            # Tree-sitter syntax highlighting
  ai/                  # Local LLM inference (llama.cpp), model management, web search
  agent/               # AI agent orchestrator, tool use, session management
  git/                 # libgit2 wrapper (status, diff, branches)
  sandbox/             # microsandbox integration (OCI images, isolated sessions)
  config.rs            # TOML configuration (AppConfig)
  session.rs           # Session persistence (save/restore tabs)
  license.rs           # License encryption (chacha20poly1305)
  blocks.rs            # Block/chunk data model for AI responses
  commands.rs          # Command definitions
  prompt.rs            # AI prompt construction
  usage.rs             # Usage tracking
  system_info.rs       # System information collection
```

## Design Principles

**CQS (Command-Query Separation)**: Methods either change state (commands, return nothing) or return data (queries, no side effects). Never both.

**SOLID**: Single responsibility per struct/function. Prefer composition. Depend on abstractions where practical.

**No code comments inside function bodies**: The code should be self-explanatory. Doc comments (`///`) on public items are acceptable.

**Rendering model**: Awebo uses a custom pixel-based renderer, not a retained-mode GUI framework. Every frame, the `Renderer` composites UI elements into a `PixelBuffer` which is presented via wgpu (GPU) or softbuffer (CPU fallback). All drawing functions are stateless - they take a buffer reference and draw into it.

**Terminal**: PTY management wraps `alacritty_terminal`. The `Terminal` struct owns the PTY and provides query methods for grid state, cursor, colors, etc.

## Code Conventions

- Rust 2024 edition
- `rustfmt` default formatting (no custom config)
- All warnings are errors in CI (`-D warnings`)
- Visibility: prefer `pub(crate)` over `pub` for internal APIs
- Error handling: propagate with `?` where possible; `log::error!` + graceful fallback for non-fatal errors
- No `unwrap()` in production code paths; `unwrap()` is acceptable only in tests
- Colors are `(u8, u8, u8)` tuples; all theme colors live in `renderer/theme.rs`
- Coordinates use `usize` for pixel positions; `f32` for logical/scaled values
- Scale factor (`sf: f64`) is threaded through rendering functions for HiDPI support

## Testing

Tests are co-located with source files using `#[cfg(test)]` modules. Nearly every module has tests. Run the full suite with `cargo test`.

When adding new functionality:
- Add unit tests in the same file under `#[cfg(test)]`
- Test edge cases (empty inputs, boundary values, zero-size areas)
- Rendering functions are tested by verifying pixel buffer contents after drawing

## Key Patterns

**Tab system**: `TabManager` holds a `Vec<Tab>`. Each `Tab` contains a `TabKind` (SmartTerminal, Sandbox, Agent, etc.) and owns its `Terminal`. New tabs inherit the current tab's working directory.

**App-controlled mode**: When a TUI application (vim, claude, etc.) takes over the terminal, the renderer switches from block-based rendering to raw grid rendering with black background fill.

**Git panel**: `GitPanelState` polls the repo via `GitRepo` on refresh. Diff stats (additions/deletions) are computed via libgit2's `diff_tree_to_workdir_with_index`.

**Configuration**: `AppConfig` is deserialized from `~/.config/awebo/config.toml` via serde. Config file gets 0600 permissions on save (unix).

**License**: Encrypted with chacha20poly1305 using a random per-file salt. License file format is versioned.

## CI Pipeline

GitHub Actions (`.github/workflows/ci.yml`):
1. **check**: `cargo fmt -- --check` + `cargo clippy -- -D warnings`
2. **test**: `cargo test`
3. **build**: `cargo build --release` (runs after check + test pass)

All jobs run on `macos-latest`. Permissions are locked down (top-level `permissions: {}`, per-job `contents: read`).

## Security

- Never hard-code cryptographic keys or salts
- Sanitize sensitive data (credentials, tokens) before logging
- OCI references are sanitized to strip embedded credentials before display
- Config files use restrictive permissions (0600)
- Workflow permissions follow least-privilege (explicit per-job)
- See `SECURITY.md` for the vulnerability reporting policy

## Common Tasks

**Adding a new UI component**: Create a file in `src/ui/components/`, export a stateless `draw_*` function that takes `&mut PixelBuffer` and returns layout info. Wire it into the renderer's `render()` method.

**Adding a new overlay/modal**: Add to `src/ui/components/overlay/`, register state in `OverlayState` (`src/app/state.rs`), handle input in `actions.rs`.

**Adding a tree-sitter grammar**: Drop a directory into `vendor/grammars/` with `grammar.toml`, `parser.c`, `scanner.c` (optional), and `.scm` query files. The build script handles the rest.

**Modifying the terminal**: The PTY layer is in `src/terminal/mod.rs` wrapping `alacritty_terminal`. Grid state queries go through `Terminal` methods.
