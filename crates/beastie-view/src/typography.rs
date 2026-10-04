//! Display-free layout using the exact bundled faces rendered by the game shell.
//!
//! Font selection, shaping and wrapping live here once. Only outlines, meshes and
//! clipping belong to the renderer. Cached results affect work, never layout.
use std::{
    collections::VecDeque,
    sync::{Arc, LazyLock, Mutex},
};

use rustybuzz::{Face, UnicodeBuffer};
use unicode_segmentation::UnicodeSegmentation;

const FONT: &[u8] =
    include_bytes!("../../../assets/generated/ui/atkinson-hyperlegible-next-medium.ttf");
const EMOJI_FONT: &[u8] = include_bytes!("../../../assets/generated/ui/noto-emoji-variable.ttf");
const CACHE_ENTRIES: usize = 64;
const CACHE_BYTES: usize = 256 * 1024;

static FONTS: LazyLock<[Face<'static>; 2]> = LazyLock::new(|| {
    [
        Face::from_slice(FONT, 0).expect("embedded Atkinson font is valid"),
        Face::from_slice(EMOJI_FONT, 0).expect("embedded Noto Emoji font is valid"),
    ]
});
static TYPOGRAPHY: LazyLock<Typography> = LazyLock::new(Typography::default);

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum FontId {
    Text,
    Emoji,
}

pub struct ShapedRun {
    pub font: FontId,
    pub glyphs: rustybuzz::GlyphBuffer,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Operation {
    Width { size: u32 },
    Lines { width: u32, size: u32 },
}

#[derive(Clone)]
enum Layout {
    Width(f32),
    Lines(Arc<Vec<String>>),
}

impl Layout {
    fn retained_bytes(&self) -> usize {
        // Include vector storage, string capacity and Arc's two reference counts.
        // Entry storage is reserved and accounted separately, including vacant slots.
        match self {
            Self::Width(_) => 0,
            Self::Lines(lines) => {
                2 * size_of::<usize>()
                    + size_of::<Vec<String>>()
                    + lines.capacity() * size_of::<String>()
                    + lines.iter().map(String::capacity).sum::<usize>()
            }
        }
    }
}

struct CacheEntry {
    text: Box<str>,
    operation: Operation,
    result: Layout,
}

impl CacheEntry {
    fn retained_bytes(&self) -> usize {
        self.text.len() + self.result.retained_bytes()
    }
}

struct LayoutCache {
    entries: VecDeque<CacheEntry>,
    retained_bytes: usize,
}

impl Default for LayoutCache {
    fn default() -> Self {
        let entries = VecDeque::with_capacity(CACHE_ENTRIES);
        let retained_bytes = entries.capacity() * size_of::<CacheEntry>();
        Self {
            entries,
            retained_bytes,
        }
    }
}

impl LayoutCache {
    fn get(&mut self, text: &str, operation: Operation) -> Option<Layout> {
        let index = self
            .entries
            .iter()
            .position(|entry| entry.operation == operation && entry.text.as_ref() == text)?;
        let entry = self.entries.remove(index)?;
        let result = entry.result.clone();
        self.entries.push_back(entry);
        Some(result)
    }

    fn insert(&mut self, text: &str, operation: Operation, result: Layout) {
        let bytes = text.len().saturating_add(result.retained_bytes());
        let storage = self.entries.capacity() * size_of::<CacheEntry>();
        if bytes > CACHE_BYTES.saturating_sub(storage) {
            return;
        }
        // Another caller may have computed the same immutable result concurrently.
        if self.get(text, operation).is_some() {
            return;
        }
        while self.entries.len() >= CACHE_ENTRIES || self.retained_bytes + bytes > CACHE_BYTES {
            let Some(entry) = self.entries.pop_front() else {
                break;
            };
            self.retained_bytes -= entry.retained_bytes();
        }
        self.entries.push_back(CacheEntry {
            text: text.into(),
            operation,
            result,
        });
        self.retained_bytes += bytes;
    }
}

/// The fixed offline font family and its bounded, presentation-only layout cache.
#[derive(Default)]
pub struct Typography {
    cache: Mutex<LayoutCache>,
    #[cfg(test)]
    shape_calls: std::sync::atomic::AtomicUsize,
}

#[must_use]
pub fn typography() -> &'static Typography {
    &TYPOGRAPHY
}

/// All line boxes use the same authored leading, with rounding left to the caller.
#[must_use]
pub fn line_height(size: f32) -> f32 {
    size * 1.2
}

impl Typography {
    fn cached(&self, text: &str, operation: Operation, compute: impl FnOnce() -> Layout) -> Layout {
        if let Some(result) = self
            .cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(text, operation)
        {
            return result;
        }
        // Shaping never holds the cache lock; simultaneous misses are harmless.
        let result = compute();
        self.cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(text, operation, result.clone());
        result
    }

    #[must_use]
    pub fn font(&self, id: FontId) -> &'static Face<'static> {
        &FONTS[match id {
            FontId::Text => 0,
            FontId::Emoji => 1,
        }]
    }

    fn shape_face(&self, text: &str, font: FontId) -> rustybuzz::GlyphBuffer {
        #[cfg(test)]
        self.shape_calls
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let mut buffer = UnicodeBuffer::new();
        buffer.push_str(text);
        buffer.guess_segment_properties();
        rustybuzz::shape(self.font(font), &[], buffer)
    }

    fn grapheme_font(&self, grapheme: &str) -> FontId {
        let requests_emoji = grapheme.contains('\u{fe0f}');
        if !requests_emoji
            && grapheme
                .chars()
                .all(|character| self.font(FontId::Text).glyph_index(character).is_some())
        {
            return FontId::Text;
        }
        // Shaping can compose accents or hide default-ignorable controls even
        // when individual code points have no cmap entry. Keep those in the text
        // face unless the grapheme explicitly requests emoji presentation.
        if !requests_emoji
            && self
                .shape_face(grapheme, FontId::Text)
                .glyph_infos()
                .iter()
                .all(|glyph| glyph.glyph_id != 0)
        {
            return FontId::Text;
        }
        let emoji = self.shape_face(grapheme, FontId::Emoji);
        if !emoji.glyph_infos().is_empty()
            && emoji.glyph_infos().iter().all(|glyph| glyph.glyph_id != 0)
        {
            FontId::Emoji
        } else {
            // This is a bounded emoji fallback, not universal script coverage.
            // Preserve the primary font's visible missing-glyph box otherwise.
            FontId::Text
        }
    }

    #[must_use]
    pub fn shape(&self, text: &str) -> Vec<ShapedRun> {
        let mut runs = Vec::new();
        let mut start = 0;
        let mut current = None;
        for (index, grapheme) in text.grapheme_indices(true) {
            let font = self.grapheme_font(grapheme);
            if let Some(previous) = current
                && previous != font
            {
                runs.push(ShapedRun {
                    font: previous,
                    glyphs: self.shape_face(&text[start..index], previous),
                });
                start = index;
            }
            current = Some(font);
        }
        if let Some(font) = current {
            runs.push(ShapedRun {
                font,
                glyphs: self.shape_face(&text[start..], font),
            });
        }
        runs
    }

    #[must_use]
    pub fn run_width(&self, run: &ShapedRun, size: f32) -> f32 {
        run.glyphs
            .glyph_positions()
            .iter()
            .map(|position| position.x_advance as f32)
            .sum::<f32>()
            * size
            / self.font(run.font).units_per_em() as f32
    }

    fn width_uncached(&self, text: &str, size: f32) -> f32 {
        self.shape(text)
            .iter()
            .map(|run| self.run_width(run, size))
            .sum()
    }

    #[must_use]
    pub fn width(&self, text: &str, size: f32) -> f32 {
        let Layout::Width(width) = self.cached(
            text,
            Operation::Width {
                size: size.to_bits(),
            },
            || Layout::Width(self.width_uncached(text, size)),
        ) else {
            unreachable!("width cache key always stores width")
        };
        width
    }

    /// Preserve explicit lines and internal spaces, wrap at word boundaries, and
    /// split unusually long words only between complete graphemes.
    #[must_use]
    pub fn lines(&self, text: &str, width: f32, size: f32) -> Arc<Vec<String>> {
        let Layout::Lines(lines) = self.cached(
            text,
            Operation::Lines {
                width: width.to_bits(),
                size: size.to_bits(),
            },
            || Layout::Lines(Arc::new(self.lines_uncached(text, width, size))),
        ) else {
            unreachable!("line cache key always stores lines")
        };
        lines
    }

    fn lines_uncached(&self, text: &str, width: f32, size: f32) -> Vec<String> {
        if !width.is_finite() || !size.is_finite() || width <= 0.0 || size <= 0.0 {
            return Vec::new();
        }
        let mut lines = Vec::new();
        for paragraph in text.split('\n') {
            let mut line = String::new();
            for word in paragraph.split_inclusive(char::is_whitespace) {
                let candidate = format!("{line}{word}");
                if self.width_uncached(candidate.trim_end(), size) <= width {
                    line = candidate;
                    continue;
                }
                if !line.trim().is_empty() {
                    lines.push(line.trim_end().to_owned());
                    line.clear();
                }
                for grapheme in word.graphemes(true) {
                    let candidate = format!("{line}{grapheme}");
                    if !line.is_empty() && self.width_uncached(candidate.trim_end(), size) > width {
                        lines.push(line.trim_end().to_owned());
                        line.clear();
                    }
                    line.push_str(grapheme);
                }
            }
            lines.push(line.trim_end().to_owned());
        }
        lines
    }

    #[must_use]
    pub fn height(&self, text: &str, width: f32, size: f32) -> f32 {
        self.lines(text, width, size).len() as f32 * line_height(size)
    }

    /// Retain complete line boxes at the authored size and mark actual overflow.
    #[must_use]
    pub fn layout_lines(&self, text: &str, width: f32, height: f32, size: f32) -> Vec<String> {
        if !size.is_finite()
            || !width.is_finite()
            || !height.is_finite()
            || size <= 0.0
            || width <= 0.0
            || height + 0.0001 < line_height(size)
        {
            return Vec::new();
        }
        let capacity = ((height + 0.0001) / line_height(size)).floor() as usize;
        let mut lines = self.lines(text, width, size).as_ref().clone();
        let overflow = lines.len() > capacity;
        lines.truncate(capacity);
        let last = lines.len().saturating_sub(1);
        for (index, line) in lines.iter_mut().enumerate() {
            if (overflow && index == last) || self.width_uncached(line, size) > width {
                *line = self.ellipsized(line, width, size);
            }
        }
        lines
    }

    fn ellipsized(&self, text: &str, width: f32, size: f32) -> String {
        if self.width_uncached("…", size) > width {
            return String::new();
        }
        let mut end = text.trim_end().len();
        loop {
            let candidate = format!("{}…", text[..end].trim_end());
            if self.width_uncached(&candidate, size) <= width {
                return candidate;
            }
            end = text[..end]
                .grapheme_indices(true)
                .next_back()
                .map_or(0, |(index, _)| index);
        }
    }

    /// Fit a single-line input without changing size or splitting its newest graphemes.
    #[must_use]
    pub fn tail_line(&self, text: &str, width: f32, height: f32, size: f32) -> String {
        if !size.is_finite()
            || !width.is_finite()
            || !height.is_finite()
            || size <= 0.0
            || width <= 0.0
            || height + 0.0001 < line_height(size)
        {
            return String::new();
        }
        let line = text.replace(['\r', '\n'], " ");
        if self.width_uncached(&line, size) <= width {
            return line;
        }
        if self.width_uncached("…", size) > width {
            return String::new();
        }
        let mut start = line.len();
        for (index, _) in line.grapheme_indices(true).rev() {
            if self.width_uncached(&format!("…{}", &line[index..]), size) > width {
                break;
            }
            start = index;
        }
        format!("…{}", &line[start..])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::Ordering;

    #[test]
    fn exact_wrapping_retains_primary_shaping_and_complete_fallback_clusters() {
        let typography = Typography::default();
        for size in [6.0, 7.8] {
            for text in [
                "AV café e\u{301}  water".to_owned(),
                "W".repeat(24),
                "🌿👩‍🔬👍🏽🇨🇦 ".repeat(10),
                "First line\n\nLast line".to_owned(),
            ] {
                let lines = typography.lines(&text, 122.0, size);
                let height = typography.height(&text, 122.0, size);
                assert_eq!(height, lines.len() as f32 * line_height(size));
                assert!(
                    lines
                        .iter()
                        .all(|line| typography.width(line, size) <= 122.0)
                );
                assert_eq!(
                    typography.layout_lines(&text, 122.0, height.ceil(), size),
                    *lines
                );
                let visible = |value: &str| {
                    value
                        .chars()
                        .filter(|c| !c.is_whitespace())
                        .collect::<String>()
                };
                assert_eq!(visible(&lines.concat()), visible(&text));
                for line in lines.iter() {
                    assert!(!line.starts_with('\u{301}') && !line.starts_with('\u{200d}'));
                }
            }
            // Equal character counts can require different actual line counts.
            let narrow = typography.lines(&"i".repeat(48), 122.0, size);
            let wide = typography.lines(&"W".repeat(48), 122.0, size);
            assert_eq!(narrow.len(), 1);
            assert!(wide.len() > narrow.len());
        }
        let joined = "👩‍🔬";
        let runs = typography.shape(joined);
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].font, FontId::Emoji);
        assert_eq!(typography.run_width(&runs[0], 1.0), 2600.0 / 2048.0);
        assert_eq!(
            typography.width(joined, 1.0),
            typography.run_width(&runs[0], 1.0)
        );
    }

    #[test]
    fn repeated_measurement_reuses_results_without_shaping() {
        let typography = Typography::default();
        let page = "A quiet thought beside the plant 🌿 and its scientist 👩‍🔬.";
        let lines = typography.lines(page, 122.0, 7.8);
        let height = typography.height(page, 122.0, 7.8);
        let width = typography.width("mushroom", 6.24);
        let calls = typography.shape_calls.load(Ordering::Relaxed);
        for _ in 0..100 {
            assert!(Arc::ptr_eq(&lines, &typography.lines(page, 122.0, 7.8)));
            assert_eq!(typography.height(page, 122.0, 7.8), height);
            assert_eq!(typography.width("mushroom", 6.24), width);
        }
        assert_eq!(typography.shape_calls.load(Ordering::Relaxed), calls);
    }

    #[test]
    fn layout_cache_bounds_retained_memory_and_eviction_preserves_results() {
        let typography = Typography::default();
        let expected = typography.lines("AV café 🌿", 24.0, 7.8);
        let operation = Operation::Width {
            size: 6.0f32.to_bits(),
        };
        {
            let mut cache = typography.cache.lock().unwrap();
            for index in 0..200 {
                cache.insert(
                    &format!("{index} {}", "W".repeat(8_000)),
                    operation,
                    Layout::Width(index as f32),
                );
                assert!(cache.entries.len() <= CACHE_ENTRIES);
                assert!(cache.retained_bytes <= CACHE_BYTES);
                assert_eq!(
                    cache.retained_bytes,
                    cache.entries.capacity() * size_of::<CacheEntry>()
                        + cache
                            .entries
                            .iter()
                            .map(CacheEntry::retained_bytes)
                            .sum::<usize>()
                );
            }
            let before = cache.retained_bytes;
            let oversized = "W".repeat(CACHE_BYTES);
            cache.insert(&oversized, operation, Layout::Width(1.0));
            assert!(cache.get(&oversized, operation).is_none());
            assert_eq!(cache.retained_bytes, before);
            for index in 0..200 {
                cache.insert(&index.to_string(), operation, Layout::Width(index as f32));
            }
            assert_eq!(cache.entries.len(), CACHE_ENTRIES);
            assert!(cache.retained_bytes <= CACHE_BYTES);
        }
        let actual = typography.lines("AV café 🌿", 24.0, 7.8);
        assert_eq!(*actual, *expected);
        assert!(!Arc::ptr_eq(&actual, &expected));
    }
}
