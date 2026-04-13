# Awebo

A GPU-accelerated terminal emulator with built-in AI, sandboxed environments, and git integration. Built with Rust.

## Features

- **GPU-accelerated rendering** — wgpu backend with instanced glyph rendering; automatic CPU (softbuffer) fallback
- **Built-in AI assistant** — local LLM inference via llama.cpp with GGUF model management and web search
- **Sandboxed environments** — isolated terminal sessions powered by microsandbox with OCI image support
- **Git integration** — built-in git panel with libgit2
- **Syntax highlighting** — tree-sitter based highlighting in the editor and terminal views
- **Tabs & split views** — multi-tab interface with side panels, file tree, and editor
- **Customizable** — TOML configuration for appearance, AI, shell, and sandbox defaults
- **Native feel** — system menus, clipboard, and macOS-native window integration

## Requirements

- **macOS** (primary platform; Linux/Windows support planned)
- Rust 2024 edition (1.85+)
- For AI features: sufficient RAM for GGUF model loading

## Building from source

```bash
git clone https://github.com/awebo-org/awebo-app.git
cd awebo
cargo build --release
```

The binary will be at `target/release/awebo`.

## Releasing

Releases are automated via GitHub Actions. Pushing a version tag triggers the full pipeline: tests → build (aarch64 + x86_64) → DMG packaging → GitHub Release.

### Stable release

```bash
git tag v1.0.0
git push origin v1.0.0
```

### Pre-release (alpha → beta → rc → stable)

```bash
# Alpha — early testing, expect breaking changes
git tag v1.0.0-alpha.1
git push origin v1.0.0-alpha.1

# Beta — feature-complete, bug fixes only
git tag v1.0.0-beta.1
git push origin v1.0.0-beta.1

# Release candidate — final validation before stable
git tag v1.0.0-rc.1
git push origin v1.0.0-rc.1

# Stable
git tag v1.0.0
git push origin v1.0.0
```

Tags containing `-alpha`, `-beta`, or `-rc` are automatically marked as **prerelease** on GitHub.

### Version format

Follows [Semantic Versioning](https://semver.org): `vMAJOR.MINOR.PATCH[-prerelease.N]`

| Bump | When |
|---|---|
| `MAJOR` | Breaking changes |
| `MINOR` | New features, backwards-compatible |
| `PATCH` | Bug fixes |

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

[general]
default_shell = "/bin/zsh"
```

## Code Availability

Awebo is **source-available** software. The complete source code is published in this repository so you can:

- **Read** and audit the code
- **Build** the application from source
- **Modify** and create derivative works
- **Contribute** improvements back to the project
- **Use it freely** for personal, non-commercial purposes

### License

Awebo is licensed under the [Business Source License 1.1](LICENSE) (BSL 1.1).

| Parameter | Value |
|---|---|
| **Licensor** | Apptivity Patryk Pasek (VAT EU: PL6941695701) |
| **Additional Use Grant** | Personal, non-commercial use |
| **Change Date** | 4 years from each version's release date (rolling) |
| **Change License** | Apache License 2.0 |

#### What this means in practice

- **Personal use**: Free. Build it, use it, hack on it.
- **Commercial / production use**: Requires a [commercial license](https://awebo.sh). This supports continued development.
- **After 4 years**: Each version automatically converts to Apache 2.0 — a permissive open-source license with no commercial restrictions.

This model (used by Sentry, CockroachDB, HashiCorp, and others) balances open development with sustainable funding.

### Commercial Licensing

For commercial or production use, purchase a license at **[awebo.sh](https://awebo.sh)**.

For enterprise licensing, volume discounts, or custom arrangements, contact **hello@awebo.sh**.

### Third-Party Licenses

Awebo depends on open-source libraries, each under their own licenses. Key dependencies include:

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

Run `cargo license` for a complete list.

## Contributing

Contributions are welcome! By submitting a pull request, you agree that your contributions will be licensed under the same BSL 1.1 terms as the rest of the project (and will transition to Apache 2.0 on the same Change Date schedule).

## Contact

- Website: [awebo.sh](https://awebo.sh)
- Email: hello@awebo.sh
