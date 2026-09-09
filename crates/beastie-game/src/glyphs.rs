//! Shaped outline lettering. Glyphs are triangles in the same scene as the aquarium,
//! never an atlas, bitmap, UI node, or second raster pass.
use std::collections::BTreeMap;

use bevy::{
    asset::RenderAssetUsages, mesh::Indices, prelude::*, render::render_resource::PrimitiveTopology,
};
use lyon_tessellation::{
    BuffersBuilder, FillOptions, FillTessellator, FillVertex, VertexBuffers, math::point,
    path::Path,
};
use rustybuzz::{
    Face, UnicodeBuffer,
    ttf_parser::{GlyphId, OutlineBuilder},
};
use unicode_segmentation::UnicodeSegmentation;

const FONT: &[u8] =
    include_bytes!("../../../assets/generated/ui/atkinson-hyperlegible-next-medium.ttf");

#[derive(Clone, Copy, Debug)]
pub struct Bounds {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Default)]
struct Glyph {
    vertices: Vec<Vec2>,
    indices: Vec<u32>,
}

#[derive(Resource)]
pub struct Lettering {
    face: Face<'static>,
    glyphs: BTreeMap<u16, Glyph>,
}
impl Default for Lettering {
    fn default() -> Self {
        Self {
            face: Face::from_slice(FONT, 0).expect("embedded Atkinson font is valid"),
            glyphs: BTreeMap::new(),
        }
    }
}

struct Outline {
    builder: lyon_tessellation::path::path::Builder,
    open: bool,
    scale: f32,
}
impl OutlineBuilder for Outline {
    fn move_to(&mut self, x: f32, y: f32) {
        if self.open {
            self.builder.end(false);
        }
        self.builder.begin(point(x * self.scale, y * self.scale));
        self.open = true;
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.builder.line_to(point(x * self.scale, y * self.scale));
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.builder.quadratic_bezier_to(
            point(x1 * self.scale, y1 * self.scale),
            point(x * self.scale, y * self.scale),
        );
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.builder.cubic_bezier_to(
            point(x1 * self.scale, y1 * self.scale),
            point(x2 * self.scale, y2 * self.scale),
            point(x * self.scale, y * self.scale),
        );
    }
    fn close(&mut self) {
        self.builder.end(true);
        self.open = false;
    }
}
impl Lettering {
    fn shape(&self, text: &str) -> rustybuzz::GlyphBuffer {
        let mut buffer = UnicodeBuffer::new();
        buffer.push_str(text);
        buffer.guess_segment_properties();
        rustybuzz::shape(&self.face, &[], buffer)
    }

    fn width(&self, text: &str, size: f32) -> f32 {
        self.shape(text)
            .glyph_positions()
            .iter()
            .map(|p| p.x_advance as f32)
            .sum::<f32>()
            * size
            / self.face.units_per_em() as f32
    }

    /// Preserve explicit lines and internal spaces, wrap at word boundaries, and split
    /// unusually long words only between graphemes. Shaping is repeated after each wrap.
    fn lines(&self, text: &str, width: f32, size: f32) -> Vec<String> {
        let mut lines = Vec::new();
        for paragraph in text.split('\n') {
            let mut line = String::new();
            for word in paragraph.split_inclusive(char::is_whitespace) {
                let candidate = format!("{line}{word}");
                if self.width(candidate.trim_end(), size) <= width {
                    line = candidate;
                    continue;
                }
                if !line.trim().is_empty() {
                    lines.push(line.trim_end().to_owned());
                    line.clear();
                }
                for grapheme in word.graphemes(true) {
                    let candidate = format!("{line}{grapheme}");
                    if !line.is_empty() && self.width(candidate.trim_end(), size) > width {
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

    fn glyph(&mut self, id: u16) -> &Glyph {
        self.glyphs.entry(id).or_insert_with(|| {
            let mut outline = Outline {
                builder: Path::builder(),
                open: false,
                scale: 1.0 / self.face.units_per_em() as f32,
            };
            if self.face.outline_glyph(GlyphId(id), &mut outline).is_none() {
                return Glyph::default();
            }
            if outline.open {
                outline.builder.end(true);
            }
            let path = outline.builder.build();
            let mut output: VertexBuffers<Vec2, u32> = VertexBuffers::new();
            FillTessellator::new()
                .tessellate_path(
                    &path,
                    &FillOptions::default().with_tolerance(0.001),
                    &mut BuffersBuilder::new(&mut output, |v: FillVertex<'_>| {
                        Vec2::new(v.position().x, v.position().y)
                    }),
                )
                .expect("embedded font outlines tessellate");
            Glyph {
                vertices: output.vertices,
                indices: output.indices,
            }
        })
    }

    pub fn append(&mut self, mesh: &mut LetterMesh, label: Label<'_>) {
        let Label {
            text,
            bounds,
            clips,
            size,
            centered,
            vertical_centered,
            color,
            z,
        } = label;
        if clips.is_empty() {
            return;
        }
        let rgba = Color::srgb_u8(color[0], color[1], color[2])
            .to_linear()
            .to_f32_array();
        let mut size = size;
        // Use measured advances, including kerning and ligatures, for the final fit.
        // This also protects unusually wide glyph strings from the legacy mean-width estimate.
        let mut lines = self.lines(text, bounds.w, size);
        for _ in 0..20 {
            if lines.len() as f32 * size * 1.2 <= bounds.h || size <= 1.0 {
                break;
            }
            size = (size * 0.94).max(1.0);
            lines = self.lines(text, bounds.w, size);
        }
        let em = self.face.units_per_em() as f32;
        let ascent = self.face.ascender() as f32 / em;
        let descent = self.face.descender() as f32 / em;
        let line_height = size * 1.2;
        let block_height = lines.len() as f32 * line_height;
        let top = bounds.y
            + if centered || vertical_centered {
                ((bounds.h - block_height) * 0.5).max(0.0)
            } else {
                0.0
            };
        // Center the font's metric box within each line box, without clipping ascenders.
        let baseline_offset = (line_height - (ascent - descent) * size) * 0.5 + ascent * size;
        for (row, line) in lines.iter().enumerate() {
            let mut pen = bounds.x
                + if centered {
                    (bounds.w - self.width(line, size)) * 0.5
                } else {
                    0.0
                };
            let baseline = top + row as f32 * line_height + baseline_offset;
            let shaped = self.shape(line);
            for (info, position) in shaped.glyph_infos().iter().zip(shaped.glyph_positions()) {
                let origin = Vec2::new(
                    pen + position.x_offset as f32 * size / em,
                    baseline - position.y_offset as f32 * size / em,
                );
                let glyph = self.glyph(info.glyph_id as u16);
                for triangle in glyph.indices.chunks_exact(3) {
                    let vertices = [triangle[0], triangle[1], triangle[2]].map(|index| {
                        let v = glyph.vertices[index as usize];
                        origin + Vec2::new(v.x, -v.y) * size
                    });
                    for clip in clips {
                        mesh.clipped_triangle(vertices, *clip, rgba, z);
                    }
                }
                pen += position.x_advance as f32 * size / em;
            }
        }
    }
}

pub struct Label<'a> {
    pub text: &'a str,
    pub bounds: Bounds,
    pub clips: &'a [Bounds],
    pub size: f32,
    pub centered: bool,
    /// Center within the line block while retaining left alignment for input and row labels.
    pub vertical_centered: bool,
    pub color: [u8; 3],
    pub z: f32,
}

#[derive(Default)]
pub struct LetterMesh {
    positions: Vec<[f32; 3]>,
    colors: Vec<[f32; 4]>,
}
impl LetterMesh {
    fn clipped_triangle(&mut self, triangle: [Vec2; 3], clip: Bounds, rgba: [f32; 4], z: f32) {
        let lower = Vec2::new(clip.x, clip.y);
        let upper = Vec2::new(clip.x + clip.w, clip.y + clip.h);
        let minimum = triangle[0].min(triangle[1]).min(triangle[2]);
        let maximum = triangle[0].max(triangle[1]).max(triangle[2]);
        if maximum.x < lower.x || minimum.x > upper.x || maximum.y < lower.y || minimum.y > upper.y
        {
            return;
        }
        if minimum.cmpge(lower).all() && maximum.cmple(upper).all() {
            self.append_polygon(&triangle, rgba, z);
            return;
        }

        // A convex triangle gains at most one vertex per rectangle half-plane,
        // giving 3 -> 4 -> 5 -> 6 -> 7 vertices. Keep a conservative 48-slot
        // bound so even duplicate boundary points or floating-point degeneracy
        // exactly retain the old algorithm: each pass emits at most twice its
        // input length, giving the unconditional bound 3 * 2^4 = 48.
        let mut first = [Vec2::ZERO; 48];
        let mut second = [Vec2::ZERO; 48];
        first[..3].copy_from_slice(&triangle);
        let (mut polygon, mut output) = (&mut first, &mut second);
        let mut length = 3;
        for (axis, boundary, sign) in [
            (0, lower.x, 1.0),
            (0, upper.x, -1.0),
            (1, lower.y, 1.0),
            (1, upper.y, -1.0),
        ] {
            if length == 0 {
                return;
            }
            let mut output_length = 0;
            let mut previous = polygon[length - 1];
            let mut previous_distance = (previous[axis] - boundary) * sign;
            for &current in &polygon[..length] {
                let distance = (current[axis] - boundary) * sign;
                if (distance >= 0.0) != (previous_distance >= 0.0) {
                    let t = previous_distance / (previous_distance - distance);
                    output[output_length] = previous.lerp(current, t);
                    output_length += 1;
                }
                if distance >= 0.0 {
                    output[output_length] = current;
                    output_length += 1;
                }
                previous = current;
                previous_distance = distance;
            }
            std::mem::swap(&mut polygon, &mut output);
            length = output_length;
        }
        self.append_polygon(&polygon[..length], rgba, z);
    }

    fn append_polygon(&mut self, polygon: &[Vec2], rgba: [f32; 4], z: f32) {
        for index in 1..polygon.len().saturating_sub(1) {
            let mut points = [polygon[0], polygon[index], polygon[index + 1]]
                .map(|p| Vec3::new((p.x - 160.0) / 20.0, (90.0 - p.y) / 20.0, z));
            let area = (points[1] - points[0]).cross(points[2] - points[0]).z;
            if area.abs() < 1e-12 {
                continue;
            }
            if area < 0.0 {
                points.swap(1, 2);
            }
            self.positions.extend(points.map(|p| p.to_array()));
            self.colors.extend([rgba; 3]);
        }
    }
    pub fn mesh(self) -> Mesh {
        let count = self.positions.len();
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::MAIN_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 0.0, 1.0]; count])
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, self.colors)
        .with_inserted_indices(Indices::U32((0..count as u32).collect()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Preserve the old allocating Sutherland-Hodgman implementation as an
    // independent oracle, including output ordering, winding and tiny-area cutoff.
    fn reference_clip(triangle: [Vec2; 3], clip: Bounds, color: [u8; 3]) -> LetterMesh {
        let mut polygon = triangle.to_vec();
        for (axis, boundary, sign) in [
            (0, clip.x, 1.0),
            (0, clip.x + clip.w, -1.0),
            (1, clip.y, 1.0),
            (1, clip.y + clip.h, -1.0),
        ] {
            if polygon.is_empty() {
                break;
            }
            let mut output = Vec::new();
            let mut previous = polygon[polygon.len() - 1];
            let mut previous_distance = (previous[axis] - boundary) * sign;
            for &current in &polygon {
                let distance = (current[axis] - boundary) * sign;
                if (distance >= 0.0) != (previous_distance >= 0.0) {
                    output.push(
                        previous.lerp(current, previous_distance / (previous_distance - distance)),
                    );
                }
                if distance >= 0.0 {
                    output.push(current);
                }
                previous = current;
                previous_distance = distance;
            }
            polygon = output;
        }
        let rgba = Color::srgb_u8(color[0], color[1], color[2])
            .to_linear()
            .to_f32_array();
        let mut mesh = LetterMesh::default();
        for index in 1..polygon.len().saturating_sub(1) {
            let mut points = [polygon[0], polygon[index], polygon[index + 1]]
                .map(|p| Vec3::new((p.x - 160.0) / 20.0, (90.0 - p.y) / 20.0, 8.1));
            let area = (points[1] - points[0]).cross(points[2] - points[0]).z;
            if area.abs() < 1e-12 {
                continue;
            }
            if area < 0.0 {
                points.swap(1, 2);
            }
            mesh.positions.extend(points.map(|p| p.to_array()));
            mesh.colors.extend([rgba; 3]);
        }
        mesh
    }

    #[test]
    fn stack_clipping_matches_original_vertices_colors_and_winding() {
        let color = [17, 139, 221];
        let rgba = Color::srgb_u8(color[0], color[1], color[2])
            .to_linear()
            .to_f32_array();
        let mut compared = 0;
        let mut state = 17_u32;
        let mut random = || {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (state >> 8) as f32 / 16_777_216.0
        };
        for scale in [0.001, 0.1, 1.0, 20.0, 200.0] {
            let origin = Vec2::new(13.0, -11.0);
            let clips = [
                Bounds {
                    x: origin.x,
                    y: origin.y,
                    w: scale,
                    h: scale,
                },
                Bounds {
                    x: origin.x,
                    y: origin.y,
                    w: scale * 0.07,
                    h: scale * 1.9,
                },
                Bounds {
                    x: origin.x,
                    y: origin.y,
                    w: 0.0,
                    h: scale,
                },
            ];
            let mut triangles = vec![
                [Vec2::ZERO, Vec2::X, Vec2::Y],
                [Vec2::ZERO, Vec2::X, Vec2::ONE],
                [Vec2::ZERO, Vec2::ZERO, Vec2::ONE],
                [Vec2::splat(0.2), Vec2::new(0.8, 0.2), Vec2::new(0.2, 0.8)],
                [
                    Vec2::new(-1.0, -1.0),
                    Vec2::new(-0.1, 2.0),
                    Vec2::new(-2.0, 1.0),
                ],
                [
                    Vec2::new(-2.0, 0.5),
                    Vec2::new(0.5, 2.0),
                    Vec2::new(2.0, -2.0),
                ],
            ];
            for _ in 0..1000 {
                triangles.push(std::array::from_fn(|_| {
                    Vec2::new(random() * 4.0 - 1.5, random() * 4.0 - 1.5)
                }));
            }
            for triangle in triangles {
                for clip in clips {
                    let triangle = triangle.map(|p| origin + p * scale);
                    let mut actual = LetterMesh::default();
                    actual.clipped_triangle(triangle, clip, rgba, 8.1);
                    let expected = reference_clip(triangle, clip, color);
                    assert_eq!(
                        actual.positions, expected.positions,
                        "triangle {triangle:?}, clip {clip:?}"
                    );
                    assert_eq!(actual.colors, expected.colors);
                    compared += 1;
                }
            }
        }
        assert!(compared > 15_000);
    }

    #[test]
    fn fully_occluded_labels_skip_shaping_and_tessellation() {
        let mut lettering = Lettering::default();
        let mut mesh = LetterMesh::default();
        lettering.append(
            &mut mesh,
            Label {
                text: "Hidden settings label é漢",
                bounds: Bounds {
                    x: 0.0,
                    y: 0.0,
                    w: 100.0,
                    h: 20.0,
                },
                clips: &[],
                size: 6.0,
                centered: false,
                vertical_centered: false,
                color: [200, 180, 160],
                z: 8.1,
            },
        );
        assert!(lettering.glyphs.is_empty());
        assert!(mesh.positions.is_empty());
    }

    #[test]
    fn input_text_centers_vertically_without_changing_left_padding_or_glyphs() {
        let bounds = Bounds {
            x: 166.0,
            y: 157.0,
            w: 98.0,
            h: 16.0,
        };
        for size in [4.8, 6.0, 7.8] {
            let mut lettering = Lettering::default();
            let render = |lettering: &mut Lettering, vertical_centered| {
                let mut mesh = LetterMesh::default();
                lettering.append(
                    &mut mesh,
                    Label {
                        text: "Talk to Mop...",
                        bounds,
                        clips: &[bounds],
                        size,
                        centered: false,
                        vertical_centered,
                        color: [255; 3],
                        z: 8.1,
                    },
                );
                mesh.positions
            };
            let top = render(&mut lettering, false);
            let centered = render(&mut lettering, true);
            assert_eq!(top.len(), centered.len());
            let shift = (bounds.h - size * 1.2) * 0.5 / 20.0;
            for (before, after) in top.iter().zip(&centered) {
                assert!((before[0] - after[0]).abs() < 1e-5);
                assert!((before[1] - after[1] - shift).abs() < 1e-5);
            }
            let ys: Vec<_> = centered.iter().map(|p| 90.0 - p[1] * 20.0).collect();
            let min = ys.iter().copied().reduce(f32::min).unwrap();
            let max = ys.iter().copied().reduce(f32::max).unwrap();
            assert!(((min + max) * 0.5 - (bounds.y + bounds.h * 0.5)).abs() < 0.5);
        }
    }

    #[test]
    fn outline_counters_remain_open_and_accents_are_real_geometry() {
        let mut lettering = Lettering::default();
        for character in ['O', 'B', 'é', '…', '\u{fffd}'] {
            let id = lettering.face.glyph_index(character).unwrap_or(GlyphId(0));
            let glyph = lettering.glyph(id.0);
            assert!(!glyph.indices.is_empty(), "{character}");
            assert!(glyph.vertices.iter().all(|p| p.is_finite()));
        }
        let id = lettering.face.glyph_index('O').unwrap();
        let glyph = lettering.glyph(id.0);
        let min = glyph.vertices.iter().copied().reduce(Vec2::min).unwrap();
        let max = glyph.vertices.iter().copied().reduce(Vec2::max).unwrap();
        let center = (min + max) * 0.5;
        for t in glyph.indices.chunks_exact(3) {
            let [a, b, c] = [t[0], t[1], t[2]].map(|i| glyph.vertices[i as usize]);
            let crosses = [
                (b - a).perp_dot(center - a),
                (c - b).perp_dot(center - b),
                (a - c).perp_dot(center - c),
            ];
            assert!(
                !crosses.iter().all(|v| *v >= 0.0) && !crosses.iter().all(|v| *v <= 0.0),
                "O counter was filled"
            );
        }
    }

    #[test]
    fn unsupported_emoji_and_cjk_render_visible_missing_glyphs() {
        let mut lettering = Lettering::default();
        let bounds = Bounds {
            x: 10.0,
            y: 10.0,
            w: 40.0,
            h: 20.0,
        };
        for character in ['🦊', '漢'] {
            assert!(lettering.face.glyph_index(character).is_none());
            let text = character.to_string();
            let shaped = lettering.shape(&text);
            assert_eq!(shaped.glyph_infos().len(), 1);
            assert_eq!(shaped.glyph_infos()[0].glyph_id, 0);
            assert!(shaped.glyph_positions()[0].x_advance > 0);
            assert!(!lettering.glyph(0).indices.is_empty());
            let mut mesh = LetterMesh::default();
            lettering.append(
                &mut mesh,
                Label {
                    text: &text,
                    bounds,
                    clips: &[bounds],
                    size: 6.0,
                    centered: false,
                    vertical_centered: false,
                    color: [255; 3],
                    z: 8.1,
                },
            );
            assert!(
                !mesh.positions.is_empty(),
                "unsupported {character} disappeared"
            );
            assert!(
                mesh.positions
                    .iter()
                    .flatten()
                    .all(|value| value.is_finite())
            );
        }
    }

    #[test]
    fn wrapping_preserves_graphemes_and_explicit_lines() {
        let lettering = Lettering::default();
        let lines = lettering.lines("Café\nwater water", 20.0, 5.0);
        assert_eq!(lines[0], "Café");
        assert_eq!(lines[1..], ["water", "water"]);
        let combined = "a\u{301}a\u{301}a\u{301}";
        let lines = lettering.lines(combined, 4.0, 5.0);
        assert_eq!(lines.concat(), combined);
        assert!(lines.iter().all(|s| !s.starts_with('\u{301}')));
    }

    #[test]
    fn clipping_stays_inside_panel_and_retains_front_winding() {
        let mut mesh = LetterMesh::default();
        let clip = Bounds {
            x: 10.0,
            y: 10.0,
            w: 5.0,
            h: 5.0,
        };
        mesh.clipped_triangle(
            [
                Vec2::new(0.0, 0.0),
                Vec2::new(30.0, 0.0),
                Vec2::new(15.0, 30.0),
            ],
            clip,
            [1.0; 4],
            8.1,
        );
        assert!(!mesh.positions.is_empty());
        for p in &mesh.positions {
            assert!((10.0 - 1e-4..=15.0 + 1e-4).contains(&(p[0] * 20.0 + 160.0)));
            assert!((10.0 - 1e-4..=15.0 + 1e-4).contains(&(90.0 - p[1] * 20.0)));
        }
        for t in mesh.positions.chunks_exact(3) {
            let [a, b, c] = [t[0], t[1], t[2]].map(Vec3::from_array);
            assert!((b - a).cross(c - a).z > 0.0);
        }
    }
}
