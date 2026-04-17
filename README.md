# Awebo

[![CI](https://github.com/awebo-org/awebo-app/actions/workflows/ci.yml/badge.svg)](https://github.com/awebo-org/awebo-app/actions/workflows/ci.yml)
[![Release](https://github.com/awebo-org/awebo-app/actions/workflows/release.yml/badge.svg)](https://github.com/awebo-org/awebo-app/actions/workflows/release.yml)
[![License: BSL 1.1](https://img.shields.io/badge/License-BSL_1.1-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-2024_edition-orange.svg)](https://www.rust-lang.org)
[![Discord](https://img.shields.io/badge/discord-join-5865F2)](https://discord.gg/g5YNDqEK)

**Local AI. Zero cloud.** A private terminal with editor, git, and sandboxed shells.

<p align="center">
  <video src="https://awebo.sh/product.mp4" controls autoplay loop muted playsinline width="720">
    <a href="https://awebo.sh/product.mp4">Watch the product demo</a>
  </video>
</p>

<p align="center">
  <a href="https://github.com/awebo-org/awebo-app/releases"><strong>↓ Download</strong></a> ·
  <a href="https://awebo.sh">awebo.sh</a> ·
  <a href="https://awebo-org.lemonsqueezy.com/checkout/buy/de81be1d-d76a-4d69-a95d-9c1e94fa2c9a?media=0">Buy</a> ·
  <a href="https://discord.gg/g5YNDqEK">Discord</a>
</p>

---

## Why Awebo

- **Runs on your machine.** Local LLM inference via llama.cpp — no API keys, no accounts, no prompts leaving your box. Optional Ollama backend if you already run one.
- **Zero telemetry by default.** Nothing phones home.
- **Works fully offline.** Every feature, including AI.
- **Batteries included.** Editor, git panel, sandboxed shells, tabs, split views — no multiplexer required.
- **GPU-accelerated.** wgpu backend with instanced glyph rendering; automatic CPU (softbuffer) fallback.
- **Source-available.** Read it, audit it, build it, hack on it. See [License](#license).

## Features

- **Built-in AI assistant** — local GGUF models via llama.cpp; optional Ollama; built-in web search; agent mode with tool use
- **Sandboxed environments** — isolated terminal sessions powered by microsandbox with OCI image support
- **Built-in code editor** — tree-sitter syntax highlighting, multi-file tabs, find/replace, undo/redo, git diff view
- **Git panel** — status, diff, staging, commit (with AI-generated messages), branch switching via libgit2
- **Tabs & split views** — multi-tab interface with file tree, search panel, sandbox panel, session manager
- **Smart terminal** — alacritty_terminal core with block-based output, command blocks, AI hints
- **Customizable** — TOML configuration for appearance, AI, shell, and sandbox defaults
- **Native macOS integration** — system menus, clipboard, HiDPI-aware rendering

## How Awebo compares

See the [feature comparison on awebo.sh](https://awebo.sh/#compare).

## Install

### macOS

Universal build (Apple Silicon + Intel):

**[↓ Download for Mac](https://github.com/awebo-org/awebo-app/releases)**

### Linux

`.deb`, `.rpm`, and AppImage on the way. Build from source in the meantime.

### Windows

`.exe` for x64 and ARM64 on the way.

## Build from source

```bash
git clone https://github.com/awebo-org/awebo-app.git
cd awebo-app
cargo build --release
```

The binary will be at `target/release/awebo`.

### Requirements

- **macOS** (primary platform; Linux/Windows planned)
- Rust 2024 edition (1.85+)
- For AI features: sufficient RAM to load your chosen GGUF model

## Configuration

Awebo stores its configuration in `~/.config/awebo/config.toml`:

```toml
[appearance]
font_family = "monospace"
font_size = 14.0
line_height = 1.2

[ai]
web_search = true
auto_load = false
# Optional: route inference through a local Ollama instance
# ollama_enabled = true
# ollama_host = "http://localhost:11434"
# ollama_model = "llama3.1"

[general]
default_shell = "/bin/zsh"
```

## Community

Join the [Awebo Discord](https://discord.gg/g5YNDqEK) to swap workflows, hack on features, test nightly builds, and vote on what ships next. This is where the product gets shaped — by the people who actually live in a terminal.

## Releasing

Releases are automated via GitHub Actions. Pushing a version tag triggers the full pipeline: tests → build (aarch64 + x86_64) → DMG packaging → GitHub Release.

```bash
# Stable
git tag v1.0.0
git push origin v1.0.0

# Pre-release (alpha → beta → rc → stable)
git tag v1.0.0-alpha.1    # early testing, expect breaking changes
git tag v1.0.0-beta.1     # feature-complete, bug fixes only
git tag v1.0.0-rc.1       # final validation before stable
```

Tags containing `-alpha`, `-beta`, or `-rc` are automatically marked as **prerelease** on GitHub. Follows [Semantic Versioning](https://semver.org): `vMAJOR.MINOR.PATCH[-prerelease.N]`.

## License

Awebo is **source-available** under the [Business Source License 1.1](LICENSE). The complete source is in this repository — you can read it, audit it, build it, modify it, and use it freely for personal, non-commercial purposes.

| Parameter | Value |
|---|---|
| **Licensor** | Apptivity Patryk Pasek (VAT EU: PL6941695701) |
| **Additional Use Grant** | Personal, non-commercial use |
| **Change Date** | 4 years from each version's release date (rolling) |
| **Change License** | Apache License 2.0 |

### What this means in practice

- **Personal use**: Free. Build it, use it, hack on it.
- **Commercial / production use**: Requires a [commercial license](https://awebo.sh/pricing). This supports continued development.
- **After 4 years**: Each version automatically converts to Apache 2.0 — a permissive open-source license with no commercial restrictions.

This model (used by Sentry, CockroachDB, HashiCorp, and others) balances open development with sustainable funding.

For enterprise licensing, volume discounts, or custom arrangements, contact **hello@awebo.sh**.

### Third-party licenses

Awebo depends on open-source libraries under their own licenses. Key dependencies:

| Crate | License |
|---|---|
| alacritty_terminal | Apache 2.0 |
| wgpu | Apache 2.0 / MIT |
| winit | Apache 2.0 / MIT |
| cosmic-text | Apache 2.0 / MIT |
| tree-sitter | MIT |
| llama-cpp-2 | MIT |
| tokio | MIT |
| git2 | Apache 2.0 / MIT |

Run `cargo license` for the complete list.

## Contributing

Contributions are welcome. By submitting a pull request, you agree that your contributions will be licensed under the same BSL 1.1 terms as the rest of the project (and will transition to Apache 2.0 on the same Change Date schedule).

See [AGENTS.md](AGENTS.md) for architecture notes and contribution guidelines.

## Contact

- Website: [awebo.sh](https://awebo.sh)
- Email: [hello@awebo.sh](mailto:hello@awebo.sh)
- Discord: [discord.gg/g5YNDqEK](https://discord.gg/g5YNDqEK)

---

© 2026 Apptivity Patryk Pasek · VAT EU: PL6941695701
