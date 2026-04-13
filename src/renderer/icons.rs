//! SVG icon rendering with caching.
//!
//! Rasterizes embedded SVG icons at any size via `resvg` and caches the
//! result per `(Icon, size_px)`.  Drawing uses the alpha channel as a
//! mask and applies the caller-supplied color — so every icon is
//! single-color and trivially tintable.

use std::collections::HashMap;

use resvg::tiny_skia::{Pixmap, Transform};
use resvg::usvg;

use crate::renderer::pixel_buffer::{PixelBuffer, Rgb};

/// Available embedded SVG icons (UI chrome — single-color, tinted by caller).
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq)]
pub enum Icon {
    Folder,
    GitBranch,
    Timer,
    Stop,
    Close,
    Plus,
    ChevronDown,
    ChevronRight,
    PanelLeft,
    Trash,
    Play,
    Download,
    Refresh,
    Rows,
    Files,
    CodeSandbox,
    Awebo,
    Diff,
    Sparkle,
    FtRust,
    FtPython,
    FtJavaScript,
    FtTypeScript,
    FtJson,
    FtHtml,
    FtCss,
    FtGo,
    FtJava,
    FtCSharp,
    FtCpp,
    FtC,
    FtRuby,
    FtSwift,
    FtToml,
    FtYaml,
    FtXml,
    FtMarkdown,
    FtShell,
    FtDocker,
    FtSql,
    FtLua,
    FtScala,
    FtKotlin,
    FtPhp,
    FtR,
    FtElixir,
    FtHaskell,
    FtZig,
    FtProto,
    FtCMake,
    FtMakefile,
    FtErlang,
    FtGitignore,
    FtLock,
    FtImage,
    FtConfig,
    FtDefault,
}

impl Icon {
    fn svg_source(&self) -> &'static str {
        match self {
            Icon::Folder => include_str!("../../assets/icons/folder.svg"),
            Icon::GitBranch => include_str!("../../assets/icons/git_branch.svg"),
            Icon::Timer => include_str!("../../assets/icons/timer.svg"),
            Icon::Stop => include_str!("../../assets/icons/stop.svg"),
            Icon::Close => include_str!("../../assets/icons/close.svg"),
            Icon::Plus => include_str!("../../assets/icons/plus.svg"),
            Icon::ChevronDown => include_str!("../../assets/icons/chevron_down.svg"),
            Icon::PanelLeft => include_str!("../../assets/icons/panel_left.svg"),
            Icon::Trash => include_str!("../../assets/icons/trash.svg"),
            Icon::Play => include_str!("../../assets/icons/play.svg"),
            Icon::Download => include_str!("../../assets/icons/download.svg"),
            Icon::Refresh => include_str!("../../assets/icons/refresh.svg"),
            Icon::ChevronRight => include_str!("../../assets/icons/chevron_right.svg"),
            Icon::Rows => include_str!("../../assets/icons/rows.svg"),
            Icon::Files => include_str!("../../assets/icons/files.svg"),
            Icon::CodeSandbox => include_str!("../../assets/icons/codesandbox.svg"),
            Icon::Awebo => include_str!("../../assets/icons/awebo.svg"),
            Icon::Diff => include_str!("../../assets/icons/diff.svg"),
            Icon::Sparkle => include_str!("../../assets/icons/sparkle.svg"),
            Icon::FtRust => include_str!("../../assets/icons/filetypes/rust.svg"),
            Icon::FtPython => include_str!("../../assets/icons/filetypes/python.svg"),
            Icon::FtJavaScript => include_str!("../../assets/icons/filetypes/javascript.svg"),
            Icon::FtTypeScript => include_str!("../../assets/icons/filetypes/typescript.svg"),
            Icon::FtJson => include_str!("../../assets/icons/filetypes/json.svg"),
            Icon::FtHtml => include_str!("../../assets/icons/filetypes/html.svg"),
            Icon::FtCss => include_str!("../../assets/icons/filetypes/css.svg"),
            Icon::FtGo => include_str!("../../assets/icons/filetypes/go.svg"),
            Icon::FtJava => include_str!("../../assets/icons/filetypes/java.svg"),
            Icon::FtCSharp => include_str!("../../assets/icons/filetypes/csharp.svg"),
            Icon::FtCpp => include_str!("../../assets/icons/filetypes/cpp.svg"),
            Icon::FtC => include_str!("../../assets/icons/filetypes/c.svg"),
            Icon::FtRuby => include_str!("../../assets/icons/filetypes/ruby.svg"),
            Icon::FtSwift => include_str!("../../assets/icons/filetypes/swift.svg"),
            Icon::FtToml => include_str!("../../assets/icons/filetypes/toml.svg"),
            Icon::FtYaml => include_str!("../../assets/icons/filetypes/yaml.svg"),
            Icon::FtXml => include_str!("../../assets/icons/filetypes/xml.svg"),
            Icon::FtMarkdown => include_str!("../../assets/icons/filetypes/markdown.svg"),
            Icon::FtShell => include_str!("../../assets/icons/filetypes/shell.svg"),
            Icon::FtDocker => include_str!("../../assets/icons/filetypes/docker.svg"),
            Icon::FtSql => include_str!("../../assets/icons/filetypes/sql.svg"),
            Icon::FtLua => include_str!("../../assets/icons/filetypes/lua.svg"),
            Icon::FtScala => include_str!("../../assets/icons/filetypes/scala.svg"),
            Icon::FtKotlin => include_str!("../../assets/icons/filetypes/kotlin.svg"),
            Icon::FtPhp => include_str!("../../assets/icons/filetypes/php.svg"),
            Icon::FtR => include_str!("../../assets/icons/filetypes/r.svg"),
            Icon::FtElixir => include_str!("../../assets/icons/filetypes/elixir.svg"),
            Icon::FtHaskell => include_str!("../../assets/icons/filetypes/haskell.svg"),
            Icon::FtZig => include_str!("../../assets/icons/filetypes/zig.svg"),
            Icon::FtProto => include_str!("../../assets/icons/filetypes/proto.svg"),
            Icon::FtCMake => include_str!("../../assets/icons/filetypes/cmake.svg"),
            Icon::FtMakefile => include_str!("../../assets/icons/filetypes/makefile.svg"),
            Icon::FtErlang => include_str!("../../assets/icons/filetypes/erlang.svg"),
            Icon::FtGitignore => include_str!("../../assets/icons/filetypes/gitignore.svg"),
            Icon::FtLock => include_str!("../../assets/icons/filetypes/lock.svg"),
            Icon::FtImage => include_str!("../../assets/icons/filetypes/image.svg"),
            Icon::FtConfig => include_str!("../../assets/icons/filetypes/config.svg"),
            Icon::FtDefault => include_str!("../../assets/icons/filetypes/default.svg"),
        }
    }
}

/// Resolve the best file-type icon for a file name.
/// Falls back to `Icon::FtDefault` for unknown extensions.
pub fn icon_for_filename(name: &str) -> Icon {
    let lower = name.to_ascii_lowercase();

    match lower.as_str() {
        "dockerfile" | "containerfile" => return Icon::FtDocker,
        "makefile" | "gnumakefile" => return Icon::FtMakefile,
        "cmakelists.txt" => return Icon::FtCMake,
        ".gitignore" | ".gitattributes" | ".gitmodules" => return Icon::FtGitignore,
        "cargo.lock" | "package-lock.json" | "yarn.lock" | "gemfile.lock"
        | "poetry.lock" | "composer.lock" | "pnpm-lock.yaml" => return Icon::FtLock,
        _ => {}
    }

    let ext = match lower.rsplit('.').next() {
        Some(e) if e != lower => e,
        _ => return Icon::FtDefault,
    };

    match ext {
        "rs" => Icon::FtRust,
        "py" | "pyi" | "pyw" | "pyx" => Icon::FtPython,
        "js" | "mjs" | "cjs" | "jsx" => Icon::FtJavaScript,
        "ts" | "mts" | "cts" | "tsx" => Icon::FtTypeScript,
        "json" | "jsonc" | "json5" => Icon::FtJson,
        "html" | "htm" | "xhtml" => Icon::FtHtml,
        "css" | "scss" | "sass" | "less" => Icon::FtCss,
        "go" => Icon::FtGo,
        "java" | "jar" => Icon::FtJava,
        "cs" | "csx" => Icon::FtCSharp,
        "cpp" | "cxx" | "cc" | "hpp" | "hxx" | "hh" => Icon::FtCpp,
        "c" | "h" => Icon::FtC,
        "rb" | "erb" | "gemspec" => Icon::FtRuby,
        "swift" => Icon::FtSwift,
        "toml" => Icon::FtToml,
        "yml" | "yaml" => Icon::FtYaml,
        "xml" | "xsd" | "xsl" | "xslt" | "svg" => Icon::FtXml,
        "md" | "mdx" | "markdown" | "rst" => Icon::FtMarkdown,
        "sh" | "bash" | "zsh" | "fish" | "ksh" | "csh" => Icon::FtShell,
        "sql" | "psql" | "plsql" => Icon::FtSql,
        "lua" => Icon::FtLua,
        "scala" | "sc" => Icon::FtScala,
        "kt" | "kts" => Icon::FtKotlin,
        "php" | "phtml" => Icon::FtPhp,
        "r" | "rmd" => Icon::FtR,
        "ex" | "exs" | "eex" | "heex" => Icon::FtElixir,
        "hs" | "lhs" => Icon::FtHaskell,
        "zig" => Icon::FtZig,
        "proto" => Icon::FtProto,
        "cmake" => Icon::FtCMake,
        "mk" => Icon::FtMakefile,
        "erl" | "hrl" => Icon::FtErlang,
        "png" | "jpg" | "jpeg" | "gif" | "bmp" | "ico" | "webp" | "tiff" | "tif" => Icon::FtImage,
        "cfg" | "conf" | "ini" | "env" | "properties" => Icon::FtConfig,
        _ => Icon::FtDefault,
    }
}

struct CachedRaster {
    width: u32,
    height: u32,
    /// Pre-multiplied RGBA from tiny-skia.
    data: Vec<u8>,
}

/// Rasterizes SVG icons and caches them per `(Icon, size)`.
pub struct IconRenderer {
    svg_options: usvg::Options<'static>,
    trees: HashMap<Icon, usvg::Tree>,
    cache: HashMap<(Icon, u32), CachedRaster>,
}

impl IconRenderer {
    pub fn new() -> Self {
        Self {
            svg_options: usvg::Options::default(),
            trees: HashMap::new(),
            cache: HashMap::new(),
        }
    }

    /// Draw `icon` tinted with `color` at `(x, y)` physical pixels.
    /// The icon is rasterized to a `size × size` square and cached.
    pub fn draw(
        &mut self,
        buf: &mut PixelBuffer,
        icon: Icon,
        x: usize,
        y: usize,
        size: u32,
        color: Rgb,
    ) {
        self.ensure_raster(icon, size);
        let raster = &self.cache[&(icon, size)];
        let (cr, cg, cb) = color;
        let rw = raster.width as usize;
        let rh = raster.height as usize;

        let x_end = (x + rw).min(buf.width);
        let y_end = (y + rh).min(buf.height);
        if x >= buf.width || y >= buf.height {
            return;
        }

        let buf_w = buf.width;
        let is_bgra = buf.is_bgra;
        buf.mark_dirty(y, y_end.saturating_sub(1));

        for py in y..y_end {
            let ry = py - y;
            let row_offset = ry * rw;
            for px in x..x_end {
                let rx = px - x;
                let ridx = (row_offset + rx) * 4;
                let a = raster.data[ridx + 3];
                if a == 0 {
                    continue;
                }
                let alpha = a as f32 * (1.0 / 255.0);
                let inv = 1.0 - alpha;
                let bidx = (py * buf_w + px) * 4;
                if is_bgra {
                    buf.data[bidx]     = (cb as f32 * alpha + buf.data[bidx]     as f32 * inv) as u8;
                    buf.data[bidx + 1] = (cg as f32 * alpha + buf.data[bidx + 1] as f32 * inv) as u8;
                    buf.data[bidx + 2] = (cr as f32 * alpha + buf.data[bidx + 2] as f32 * inv) as u8;
                } else {
                    buf.data[bidx]     = (cr as f32 * alpha + buf.data[bidx]     as f32 * inv) as u8;
                    buf.data[bidx + 1] = (cg as f32 * alpha + buf.data[bidx + 1] as f32 * inv) as u8;
                    buf.data[bidx + 2] = (cb as f32 * alpha + buf.data[bidx + 2] as f32 * inv) as u8;
                }
            }
        }
    }

    /// Draw an icon using its **native SVG colors** (no tinting).
    /// Used for multi-color file type icons.
    pub fn draw_colored(
        &mut self,
        buf: &mut PixelBuffer,
        icon: Icon,
        x: usize,
        y: usize,
        size: u32,
    ) {
        self.ensure_raster(icon, size);
        let raster = &self.cache[&(icon, size)];
        let rw = raster.width as usize;
        let rh = raster.height as usize;

        let x_end = (x + rw).min(buf.width);
        let y_end = (y + rh).min(buf.height);
        if x >= buf.width || y >= buf.height {
            return;
        }

        let buf_w = buf.width;
        let is_bgra = buf.is_bgra;
        buf.mark_dirty(y, y_end.saturating_sub(1));

        for py in y..y_end {
            let ry = py - y;
            let row_offset = ry * rw;
            for px in x..x_end {
                let rx = px - x;
                let ridx = (row_offset + rx) * 4;
                let a = raster.data[ridx + 3];
                if a == 0 {
                    continue;
                }
                let alpha = a as f32 * (1.0 / 255.0);
                let inv = 1.0 - alpha;
                let inv_a = if a > 0 { 255.0 / a as f32 } else { 0.0 };
                let sr = (raster.data[ridx] as f32 * inv_a).min(255.0);
                let sg = (raster.data[ridx + 1] as f32 * inv_a).min(255.0);
                let sb = (raster.data[ridx + 2] as f32 * inv_a).min(255.0);

                let bidx = (py * buf_w + px) * 4;
                if is_bgra {
                    buf.data[bidx]     = (sb * alpha + buf.data[bidx]     as f32 * inv) as u8;
                    buf.data[bidx + 1] = (sg * alpha + buf.data[bidx + 1] as f32 * inv) as u8;
                    buf.data[bidx + 2] = (sr * alpha + buf.data[bidx + 2] as f32 * inv) as u8;
                } else {
                    buf.data[bidx]     = (sr * alpha + buf.data[bidx]     as f32 * inv) as u8;
                    buf.data[bidx + 1] = (sg * alpha + buf.data[bidx + 1] as f32 * inv) as u8;
                    buf.data[bidx + 2] = (sb * alpha + buf.data[bidx + 2] as f32 * inv) as u8;
                }
            }
        }
    }

    fn ensure_tree(&mut self, icon: Icon) {
        if self.trees.contains_key(&icon) {
            return;
        }
        let tree = usvg::Tree::from_str(icon.svg_source(), &self.svg_options)
            .expect("embedded SVG icon failed to parse");
        self.trees.insert(icon, tree);
    }

    fn ensure_raster(&mut self, icon: Icon, size: u32) {
        let key = (icon, size);
        if self.cache.contains_key(&key) {
            return;
        }

        self.ensure_tree(icon);
        let tree = &self.trees[&icon];

        let svg_size = tree.size();
        let sx = size as f32 / svg_size.width();
        let sy = size as f32 / svg_size.height();

        let mut pixmap = Pixmap::new(size, size)
            .expect("failed to allocate icon pixmap");
        resvg::render(tree, Transform::from_scale(sx, sy), &mut pixmap.as_mut());

        self.cache.insert(key, CachedRaster {
            width: size,
            height: size,
            data: pixmap.data().to_vec(),
        });
    }
}

const AVATAR_PNG: &[u8] = include_bytes!("../../assets/awebo.png");

/// Renders the embedded avatar PNG at any size with rounded (circular) corners.
/// Caches the scaled RGBA data per requested size.
pub struct AvatarRenderer {
    decoded: Option<image::DynamicImage>,
    cache: HashMap<u32, CachedRaster>,
}

impl AvatarRenderer {
    pub fn new() -> Self {
        let decoded = image::load_from_memory(AVATAR_PNG).ok();
        Self { decoded, cache: HashMap::new() }
    }

    /// Draw the avatar at `(x, y)` physical pixels, scaled to `size × size`,
    /// with fully rounded (circular) corners.
    pub fn draw(&mut self, buf: &mut PixelBuffer, x: usize, y: usize, size: u32) {
        self.ensure_raster(size);
        let raster = &self.cache[&size];
        let rw = raster.width as usize;
        let rh = raster.height as usize;

        let x_end = (x + rw).min(buf.width);
        let y_end = (y + rh).min(buf.height);
        if x >= buf.width || y >= buf.height {
            return;
        }

        let buf_w = buf.width;
        let is_bgra = buf.is_bgra;
        buf.mark_dirty(y, y_end.saturating_sub(1));

        let radius = size as f32 / 2.0;
        let cx = radius;
        let cy = radius;

        for py in y..y_end {
            let ry = py - y;
            let row_offset = ry * rw;
            for px in x..x_end {
                let rx = px - x;

                let dx = rx as f32 + 0.5 - cx;
                let dy = ry as f32 + 0.5 - cy;
                let dist = (dx * dx + dy * dy).sqrt();
                if dist > radius {
                    continue;
                }
                let circle_alpha = (radius - dist).min(1.0).max(0.0);

                let ridx = (row_offset + rx) * 4;
                let sr = raster.data[ridx];
                let sg = raster.data[ridx + 1];
                let sb = raster.data[ridx + 2];
                let sa = raster.data[ridx + 3];

                let alpha = (sa as f32 / 255.0) * circle_alpha;
                let inv = 1.0 - alpha;

                let bidx = (py * buf_w + px) * 4;
                if is_bgra {
                    buf.data[bidx]     = (sb as f32 * alpha + buf.data[bidx]     as f32 * inv) as u8;
                    buf.data[bidx + 1] = (sg as f32 * alpha + buf.data[bidx + 1] as f32 * inv) as u8;
                    buf.data[bidx + 2] = (sr as f32 * alpha + buf.data[bidx + 2] as f32 * inv) as u8;
                } else {
                    buf.data[bidx]     = (sr as f32 * alpha + buf.data[bidx]     as f32 * inv) as u8;
                    buf.data[bidx + 1] = (sg as f32 * alpha + buf.data[bidx + 1] as f32 * inv) as u8;
                    buf.data[bidx + 2] = (sb as f32 * alpha + buf.data[bidx + 2] as f32 * inv) as u8;
                }
            }
        }
    }

    fn ensure_raster(&mut self, size: u32) {
        if self.cache.contains_key(&size) {
            return;
        }
        let Some(img) = &self.decoded else { return };
        let resized = img.resize_exact(size, size, image::imageops::FilterType::Triangle);
        let rgba = resized.to_rgba8();

        self.cache.insert(size, CachedRaster {
            width: size,
            height: size,
            data: rgba.into_raw(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_icons_parse() {
        let mut r = IconRenderer::new();
        let ui_icons = [
            Icon::Folder, Icon::GitBranch, Icon::Timer, Icon::Stop,
            Icon::Close, Icon::Plus, Icon::ChevronDown, Icon::ChevronRight,
            Icon::PanelLeft,
            Icon::Trash, Icon::Play, Icon::Download, Icon::Refresh,
            Icon::Rows, Icon::Files,
            Icon::CodeSandbox, Icon::Awebo, Icon::Diff, Icon::Sparkle,
        ];
        let ft_icons = [
            Icon::FtRust, Icon::FtPython, Icon::FtJavaScript, Icon::FtTypeScript,
            Icon::FtJson, Icon::FtHtml, Icon::FtCss, Icon::FtGo,
            Icon::FtJava, Icon::FtCSharp, Icon::FtCpp, Icon::FtC,
            Icon::FtRuby, Icon::FtSwift, Icon::FtToml, Icon::FtYaml,
            Icon::FtXml, Icon::FtMarkdown, Icon::FtShell, Icon::FtDocker,
            Icon::FtSql, Icon::FtLua, Icon::FtScala, Icon::FtKotlin,
            Icon::FtPhp, Icon::FtR, Icon::FtElixir, Icon::FtHaskell,
            Icon::FtZig, Icon::FtProto, Icon::FtCMake, Icon::FtMakefile,
            Icon::FtErlang, Icon::FtGitignore, Icon::FtLock, Icon::FtImage,
            Icon::FtConfig, Icon::FtDefault,
        ];
        for icon in ui_icons.iter().chain(ft_icons.iter()) {
            r.ensure_tree(*icon);
        }
        assert_eq!(r.trees.len(), ui_icons.len() + ft_icons.len());
    }

    #[test]
    fn rasterize_produces_correct_dimensions() {
        let mut r = IconRenderer::new();
        r.ensure_raster(Icon::Folder, 32);
        let raster = &r.cache[&(Icon::Folder, 32)];
        assert_eq!(raster.width, 32);
        assert_eq!(raster.height, 32);
        assert_eq!(raster.data.len(), 32 * 32 * 4);
    }

    #[test]
    fn rasterize_has_nonzero_alpha() {
        let mut r = IconRenderer::new();
        r.ensure_raster(Icon::Folder, 24);
        let raster = &r.cache[&(Icon::Folder, 24)];
        let any_visible = raster.data.chunks(4).any(|px| px[3] > 0);
        assert!(any_visible, "icon should have visible pixels");
    }

    #[test]
    fn draw_does_not_panic() {
        let mut r = IconRenderer::new();
        let mut buf = PixelBuffer::new(100, 100, false, (0, 0, 0));
        r.draw(&mut buf, Icon::Folder, 10, 10, 16, (255, 255, 255));
    }

    #[test]
    fn different_sizes_cached_separately() {
        let mut r = IconRenderer::new();
        r.ensure_raster(Icon::GitBranch, 16);
        r.ensure_raster(Icon::GitBranch, 32);
        assert_eq!(r.cache.len(), 2);
    }

    #[test]
    fn draw_colored_does_not_panic() {
        let mut r = IconRenderer::new();
        let mut buf = PixelBuffer::new(100, 100, false, (0, 0, 0));
        r.draw_colored(&mut buf, Icon::FtRust, 10, 10, 16);
    }

    #[test]
    fn icon_for_filename_known_extensions() {
        assert_eq!(icon_for_filename("main.rs"), Icon::FtRust);
        assert_eq!(icon_for_filename("app.py"), Icon::FtPython);
        assert_eq!(icon_for_filename("index.tsx"), Icon::FtTypeScript);
        assert_eq!(icon_for_filename("styles.css"), Icon::FtCss);
        assert_eq!(icon_for_filename("data.json"), Icon::FtJson);
        assert_eq!(icon_for_filename("README.md"), Icon::FtMarkdown);
        assert_eq!(icon_for_filename("build.zig"), Icon::FtZig);
    }

    #[test]
    fn icon_for_filename_special_names() {
        assert_eq!(icon_for_filename("Dockerfile"), Icon::FtDocker);
        assert_eq!(icon_for_filename("Makefile"), Icon::FtMakefile);
        assert_eq!(icon_for_filename(".gitignore"), Icon::FtGitignore);
        assert_eq!(icon_for_filename("Cargo.lock"), Icon::FtLock);
    }

    #[test]
    fn icon_for_filename_unknown() {
        assert_eq!(icon_for_filename("something.xyz"), Icon::FtDefault);
        assert_eq!(icon_for_filename("noext"), Icon::FtDefault);
    }
}
