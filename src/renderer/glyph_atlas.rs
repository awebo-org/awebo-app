/// CPU-side glyph atlas for monospace terminal rendering.
///
/// Pre-rasterizes glyphs into alpha bitmaps keyed on `(char, font_size)`.
/// Grid rendering blits from the atlas instead of re-shaping text per row,
/// reducing per-frame cost from O(rows × cols × shape) to O(rows × cols × memcpy).
use std::collections::HashMap;

use cosmic_text::{
    Attrs, Buffer, Color as CColor, Family, FontSystem, Metrics, Shaping, Style, SwashCache, Weight, Wrap,
};

/// Rasterized glyph: alpha bitmap + positioning metadata.
#[derive(Clone)]
pub struct RasterizedGlyph {
    pub width: usize,
    pub height: usize,
    pub bearing_x: i32,
    pub bearing_y: i32,
    pub alphas: Vec<u8>,
}

/// Key for atlas lookup — character + font size in centipixels + style flags.
#[derive(Debug, Hash, Eq, PartialEq, Clone, Copy)]
struct GlyphKey {
    ch: char,
    size_cp: u32,
    bold: bool,
    italic: bool,
}

impl GlyphKey {
    fn new(ch: char, font_size: f32, bold: bool, italic: bool) -> Self {
        Self {
            ch,
            size_cp: (font_size * 100.0) as u32,
            bold,
            italic,
        }
    }
}

pub struct GlyphAtlas {
    cache: HashMap<GlyphKey, Option<RasterizedGlyph>>,
    family: Family<'static>,
}

impl GlyphAtlas {
    pub fn new(family: Family<'static>) -> Self {
        Self {
            cache: HashMap::with_capacity(512),
            family,
        }
    }

    /// Get or rasterize a glyph for the given character and font size.
    /// Returns `None` for whitespace or glyphs that produce no pixels.
    pub fn get_or_rasterize(
        &mut self,
        ch: char,
        font_size: f32,
        line_height: f32,
        bold: bool,
        italic: bool,
        font_system: &mut FontSystem,
        swash_cache: &mut SwashCache,
    ) -> Option<&RasterizedGlyph> {
        let key = GlyphKey::new(ch, font_size, bold, italic);

        if !self.cache.contains_key(&key) {
            let glyph = Self::rasterize(
                ch,
                font_size,
                line_height,
                bold,
                italic,
                self.family,
                font_system,
                swash_cache,
            );
            self.cache.insert(key, glyph);
        }

        self.cache.get(&key).and_then(|g| g.as_ref())
    }

    fn rasterize(
        ch: char,
        font_size: f32,
        line_height: f32,
        bold: bool,
        italic: bool,
        family: Family<'static>,
        font_system: &mut FontSystem,
        swash_cache: &mut SwashCache,
    ) -> Option<RasterizedGlyph> {
        if ch.is_whitespace() {
            return None;
        }

        let metrics = Metrics::new(font_size, line_height);
        let cell_w = (font_size * 0.6).ceil() as usize + 4;

        let mut buffer = Buffer::new(font_system, metrics);
        buffer.set_size(font_system, Some(cell_w as f32 * 2.0), Some(line_height));
        buffer.set_wrap(font_system, Wrap::None);

        let s = ch.to_string();
        let weight = if bold { Weight::BOLD } else { Weight::NORMAL };
        let style = if italic { Style::Italic } else { Style::Normal };
        buffer.set_text(
            font_system,
            &s,
            &Attrs::new().family(family).weight(weight).style(style),
            Shaping::Advanced,
            None,
        );
        buffer.shape_until_scroll(font_system, true);

        let mut min_x = i32::MAX;
        let mut min_y = i32::MAX;
        let mut max_x = i32::MIN;
        let mut max_y = i32::MIN;
        let mut pixels: Vec<(i32, i32, u8)> = Vec::new();

        buffer.draw(
            font_system,
            swash_cache,
            CColor::rgba(255, 255, 255, 255),
            |x, y, _gw, _gh, color| {
                let a = color.a();
                if a == 0 {
                    return;
                }
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
                pixels.push((x, y, a));
            },
        );

        if pixels.is_empty() {
            return None;
        }

        let w = (max_x - min_x + 1) as usize;
        let h = (max_y - min_y + 1) as usize;
        let mut alphas = vec![0u8; w * h];

        for (px, py, a) in &pixels {
            let lx = (px - min_x) as usize;
            let ly = (py - min_y) as usize;
            alphas[ly * w + lx] = *a;
        }

        Some(RasterizedGlyph {
            width: w,
            height: h,
            bearing_x: min_x,
            bearing_y: min_y,
            alphas,
        })
    }

    pub fn clear(&mut self) {
        self.cache.clear();
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.cache.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glyph_key_equality() {
        let a = GlyphKey::new('A', 16.0, false, false);
        let b = GlyphKey::new('A', 16.0, false, false);
        let c = GlyphKey::new('B', 16.0, false, false);
        let d = GlyphKey::new('A', 14.0, false, false);
        let e = GlyphKey::new('A', 16.0, true, false);
        let f = GlyphKey::new('A', 16.0, false, true);
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_ne!(a, d);
        assert_ne!(a, e);
        assert_ne!(a, f);
    }

    #[test]
    fn atlas_new_is_empty() {
        let atlas = GlyphAtlas::new(Family::Monospace);
        assert_eq!(atlas.len(), 0);
    }

    #[test]
    fn atlas_clear_empties() {
        let mut atlas = GlyphAtlas::new(Family::Monospace);
        let mut fs = FontSystem::new();
        let mut sc = SwashCache::new();
        atlas.get_or_rasterize('A', 16.0, 22.0, false, false, &mut fs, &mut sc);
        assert!(atlas.len() > 0);
        atlas.clear();
        assert_eq!(atlas.len(), 0);
    }

    #[test]
    fn atlas_caches_glyph() {
        let mut atlas = GlyphAtlas::new(Family::Monospace);
        let mut fs = FontSystem::new();
        let mut sc = SwashCache::new();

        let g1 = atlas.get_or_rasterize('A', 16.0, 22.0, false, false, &mut fs, &mut sc);
        assert!(g1.is_some());
        let count = atlas.len();

        let g2 = atlas.get_or_rasterize('A', 16.0, 22.0, false, false, &mut fs, &mut sc);
        assert!(g2.is_some());
        assert_eq!(atlas.len(), count);
    }

    #[test]
    fn atlas_whitespace_returns_none() {
        let mut atlas = GlyphAtlas::new(Family::Monospace);
        let mut fs = FontSystem::new();
        let mut sc = SwashCache::new();
        assert!(atlas
            .get_or_rasterize(' ', 16.0, 22.0, false, false, &mut fs, &mut sc)
            .is_none());
    }

    #[test]
    fn rasterized_glyph_has_pixels() {
        let mut atlas = GlyphAtlas::new(Family::Monospace);
        let mut fs = FontSystem::new();
        let mut sc = SwashCache::new();
        let g = atlas
            .get_or_rasterize('W', 32.0, 44.0, false, false, &mut fs, &mut sc)
            .unwrap();
        assert!(g.width > 0);
        assert!(g.height > 0);
        assert!(!g.alphas.is_empty());
        assert!(g.alphas.iter().any(|&a| a > 0));
    }

    #[test]
    fn different_sizes_cached_separately() {
        let mut atlas = GlyphAtlas::new(Family::Monospace);
        let mut fs = FontSystem::new();
        let mut sc = SwashCache::new();
        atlas.get_or_rasterize('X', 16.0, 22.0, false, false, &mut fs, &mut sc);
        atlas.get_or_rasterize('X', 32.0, 44.0, false, false, &mut fs, &mut sc);
        assert_eq!(atlas.len(), 2);
    }
}
