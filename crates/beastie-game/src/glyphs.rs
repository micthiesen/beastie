//! Shaped outline lettering. Glyphs are triangles in the same scene as the aquarium,
//! never an atlas, bitmap, UI node, or second raster pass.
use std::collections::BTreeMap;

use beastie_view::{
    TextInputState,
    typography::{FontId, Typography, line_height, typography},
};
use bevy::{
    asset::RenderAssetUsages, mesh::Indices, prelude::*, render::render_resource::PrimitiveTopology,
};
use lyon_tessellation::{
    BuffersBuilder, FillOptions, FillTessellator, FillVertex, VertexBuffers, math::point,
    path::Path,
};
use rustybuzz::ttf_parser::{GlyphId, OutlineBuilder};
#[cfg(test)]
use unicode_segmentation::UnicodeSegmentation;

const CARET_WIDTH: f32 = 0.35;
const CARET_GAP: f32 = 0.65;
pub(crate) const CARET_SPACE: f32 = CARET_WIDTH + CARET_GAP;

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
    typography: &'static Typography,
    // Both faces use independent glyph IDs. Cached outlines are normalized to ems
    // from their own face, so they can be reused at every authored text size.
    glyphs: BTreeMap<(FontId, u16), Glyph>,
}
impl Default for Lettering {
    fn default() -> Self {
        Self {
            typography: typography(),
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
    fn width(&self, text: &str, size: f32) -> f32 {
        self.typography.width(text, size)
    }

    pub(crate) fn layout_lines(&self, text: &str, bounds: Bounds, size: f32) -> Vec<String> {
        self.typography.layout_lines(text, bounds.w, bounds.h, size)
    }

    pub(crate) fn tail_line(&self, text: &str, bounds: Bounds, size: f32) -> String {
        self.typography.tail_line(text, bounds.w, bounds.h, size)
    }

    fn input_decoration_bounds(&self, label: Label<'_>, state: TextInputState) -> Option<Bounds> {
        let bounds = label.bounds;
        if !label.size.is_finite()
            || label.size <= 0.0
            || !bounds.w.is_finite()
            || !bounds.h.is_finite()
            || bounds.w <= 0.0
            || bounds.h <= 0.0
        {
            return None;
        }
        let advance = self.width(label.text, label.size).min(bounds.w);
        let left = bounds.x
            + if label.centered {
                (bounds.w - advance) * 0.5
            } else {
                0.0
            };
        let (x, w, height) = match state {
            TextInputState::Selected if advance <= 0.0 => return None,
            TextInputState::Selected => (left, advance, line_height(label.size)),
            TextInputState::Caret => {
                let width = CARET_WIDTH.min(bounds.w);
                let gap = if label.text.is_empty() {
                    0.0
                } else {
                    CARET_GAP
                };
                (
                    (left + advance + gap).min(bounds.x + bounds.w - width),
                    width,
                    label.size * 1.05,
                )
            }
        };
        let height = height.min(bounds.h);
        let top = bounds.y
            + if label.centered || label.vertical_centered {
                (bounds.h - height) * 0.5
            } else {
                ((line_height(label.size) - height) * 0.5).min(bounds.h - height)
            };
        Some(Bounds {
            x,
            y: top,
            w,
            h: height,
        })
    }

    /// Decorations share the retained label mesh and the label's actual occlusion
    /// clips. A steady caret introduces no timer or per-frame geometry updates.
    pub(crate) fn append_input_decoration(
        &self,
        mesh: &mut LetterMesh,
        label: Label<'_>,
        state: TextInputState,
    ) {
        if label.clips.is_empty() {
            return;
        }
        let Some(area) = self.input_decoration_bounds(label, state) else {
            return;
        };
        let rgba = Color::srgb_u8(label.color[0], label.color[1], label.color[2])
            .to_linear()
            .to_f32_array();
        let a = Vec2::new(area.x, area.y);
        let b = Vec2::new(area.x + area.w, area.y);
        let c = Vec2::new(area.x + area.w, area.y + area.h);
        let d = Vec2::new(area.x, area.y + area.h);
        for clip in label.clips {
            mesh.clipped_triangle([a, b, c], *clip, rgba, label.z);
            mesh.clipped_triangle([a, c, d], *clip, rgba, label.z);
        }
    }

    fn glyph(&mut self, font: FontId, id: u16) -> &Glyph {
        let face = self.typography.font(font);
        self.glyphs.entry((font, id)).or_insert_with(|| {
            let mut outline = Outline {
                builder: Path::builder(),
                open: false,
                scale: 1.0 / face.units_per_em() as f32,
            };
            if face.outline_glyph(GlyphId(id), &mut outline).is_none() {
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
        let lines = self.layout_lines(text, bounds, size);
        let line_height = line_height(size);
        let block_height = lines.len() as f32 * line_height;
        let top = bounds.y
            + if centered || vertical_centered {
                ((bounds.h - block_height) * 0.5).max(0.0)
            } else {
                0.0
            };
        for (row, line) in lines.iter().enumerate() {
            let shaped = self.typography.shape(line);
            let mut pen = bounds.x
                + if centered {
                    (bounds.w
                        - shaped
                            .iter()
                            .map(|run| self.typography.run_width(run, size))
                            .sum::<f32>())
                        * 0.5
                } else {
                    0.0
                };
            for run in shaped {
                let face = self.typography.font(run.font);
                let em = face.units_per_em() as f32;
                let ascent = face.ascender() as f32 / em;
                let descent = face.descender() as f32 / em;
                // Center each face's metric box in the same authored line box.
                let baseline_offset =
                    (line_height - (ascent - descent) * size) * 0.5 + ascent * size;
                let baseline = top + row as f32 * line_height + baseline_offset;
                for (info, position) in run
                    .glyphs
                    .glyph_infos()
                    .iter()
                    .zip(run.glyphs.glyph_positions())
                {
                    let origin = Vec2::new(
                        pen + position.x_offset as f32 * size / em,
                        baseline - position.y_offset as f32 * size / em,
                    );
                    let glyph = self.glyph(run.font, info.glyph_id as u16);
                    for triangle in glyph.indices.as_chunks::<3>().0.iter() {
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
}

#[derive(Clone, Copy)]
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
            let id = lettering
                .typography
                .font(FontId::Text)
                .glyph_index(character)
                .unwrap_or(GlyphId(0));
            let glyph = lettering.glyph(FontId::Text, id.0);
            assert!(!glyph.indices.is_empty(), "{character}");
            assert!(glyph.vertices.iter().all(|p| p.is_finite()));
        }
        let id = lettering
            .typography
            .font(FontId::Text)
            .glyph_index('O')
            .unwrap();
        let glyph = lettering.glyph(FontId::Text, id.0);
        let min = glyph.vertices.iter().copied().reduce(Vec2::min).unwrap();
        let max = glyph.vertices.iter().copied().reduce(Vec2::max).unwrap();
        let center = (min + max) * 0.5;
        for t in glyph.indices.as_chunks::<3>().0.iter() {
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
    fn unsupported_scripts_retain_visible_missing_glyphs() {
        let mut lettering = Lettering::default();
        let bounds = Bounds {
            x: 10.0,
            y: 10.0,
            w: 40.0,
            h: 20.0,
        };
        for character in ['漢', '한'] {
            assert!(
                lettering
                    .typography
                    .font(FontId::Text)
                    .glyph_index(character)
                    .is_none()
            );
            let text = character.to_string();
            let shaped = lettering.typography.shape(&text);
            assert_eq!(shaped.len(), 1);
            assert_eq!(shaped[0].font, FontId::Text);
            assert_eq!(shaped[0].glyphs.glyph_infos().len(), 1);
            assert_eq!(shaped[0].glyphs.glyph_infos()[0].glyph_id, 0);
            assert!(shaped[0].glyphs.glyph_positions()[0].x_advance > 0);
            assert!(!lettering.glyph(FontId::Text, 0).indices.is_empty());
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
    fn emoji_fallback_preserves_primary_runs_and_uses_each_faces_units() {
        let mut lettering = Lettering::default();
        // Latin kerning, ligatures, and composed/decomposed accents still reach
        // Rustybuzz together, exactly as they did before fallback was available.
        for text in ["AV office Wi café", "cafe\u{301}", "123 # *"] {
            let runs = lettering.typography.shape(text);
            assert_eq!(runs.len(), 1);
            assert_eq!(runs[0].font, FontId::Text);
            let mut buffer = rustybuzz::UnicodeBuffer::new();
            buffer.push_str(text);
            buffer.guess_segment_properties();
            let original = rustybuzz::shape(lettering.typography.font(FontId::Text), &[], buffer);
            let actual_ids: Vec<_> = runs[0]
                .glyphs
                .glyph_infos()
                .iter()
                .map(|info| (info.glyph_id, info.cluster))
                .collect();
            let expected_ids: Vec<_> = original
                .glyph_infos()
                .iter()
                .map(|info| (info.glyph_id, info.cluster))
                .collect();
            assert_eq!(actual_ids, expected_ids);
            let positions = |buffer: &rustybuzz::GlyphBuffer| {
                buffer
                    .glyph_positions()
                    .iter()
                    .map(|position| {
                        (
                            position.x_advance,
                            position.y_advance,
                            position.x_offset,
                            position.y_offset,
                        )
                    })
                    .collect::<Vec<_>>()
            };
            assert_eq!(positions(&runs[0].glyphs), positions(&original));
        }
        let runs = lettering.typography.shape("AV café 🌿 Wi");
        assert_eq!(
            runs.iter().map(|run| run.font).collect::<Vec<_>>(),
            [FontId::Text, FontId::Emoji, FontId::Text]
        );
        assert!(runs.iter().all(|run| {
            run.glyphs
                .glyph_infos()
                .iter()
                .all(|glyph| glyph.glyph_id != 0)
        }));
        let leaf = &runs[1];
        assert_eq!(lettering.typography.font(FontId::Text).units_per_em(), 1000);
        assert_eq!(
            lettering.typography.font(FontId::Emoji).units_per_em(),
            2048
        );
        assert_eq!(leaf.glyphs.glyph_positions()[0].x_advance, 2600);
        for size in [6.0, 7.8] {
            let expected = lettering.width("AV café ", size)
                + 2600.0 / 2048.0 * size
                + lettering.width(" Wi", size);
            assert!((lettering.width("AV café 🌿 Wi", size) - expected).abs() < 0.0001);
        }
        let id = leaf.glyphs.glyph_infos()[0].glyph_id as u16;
        assert!(!lettering.glyph(FontId::Emoji, id).indices.is_empty());
        lettering.glyph(FontId::Text, id);
        assert!(lettering.glyphs.contains_key(&(FontId::Emoji, id)));
        assert!(lettering.glyphs.contains_key(&(FontId::Text, id)));
        assert_ne!(
            lettering.glyphs[&(FontId::Emoji, id)].vertices,
            lettering.glyphs[&(FontId::Text, id)].vertices
        );
    }

    #[test]
    fn joined_modified_flag_and_keycap_emoji_shape_as_whole_clusters() {
        let mut lettering = Lettering::default();
        for text in ["👩‍🔬", "👨‍👩‍👧‍👦", "👍🏽", "🇨🇦", "1️⃣", "❤️"]
        {
            assert_eq!(text.graphemes(true).count(), 1);
            let runs = lettering.typography.shape(text);
            assert_eq!(runs.len(), 1, "{text}");
            let run = &runs[0];
            assert_eq!(run.font, FontId::Emoji, "{text}");
            assert!(
                run.glyphs
                    .glyph_infos()
                    .iter()
                    .all(|info| info.glyph_id != 0)
            );
            assert_eq!(
                run.glyphs
                    .glyph_positions()
                    .iter()
                    .filter(|position| position.x_advance > 0)
                    .count(),
                1,
                "{text} must use its joined glyph, not separate advancing components"
            );
            let mut visible = 0;
            for info in run.glyphs.glyph_infos() {
                let glyph = lettering.glyph(run.font, info.glyph_id as u16);
                visible += usize::from(!glyph.indices.is_empty());
                assert!(glyph.vertices.iter().all(|vertex| vertex.is_finite()));
            }
            assert_eq!(visible, 1, "{text}");
        }
    }

    #[test]
    fn mixed_emoji_tails_selection_caret_and_geometry_share_measured_runs() {
        let mut lettering = Lettering::default();
        let draft = format!("{}AV 🌿 👩‍🔬Z", "W".repeat(100));
        let expected = "…AV 🌿 👩‍🔬Z";
        for size in [6.0, 7.8] {
            let bounds = Bounds {
                x: 12.0,
                y: 20.0,
                w: lettering.width(expected, size) + CARET_SPACE + 0.001,
                h: 18.0,
            };
            let tail = lettering.tail_line(
                &draft,
                Bounds {
                    w: bounds.w - CARET_SPACE,
                    ..bounds
                },
                size,
            );
            assert_eq!(tail, expected);
            let label = Label {
                text: &tail,
                bounds,
                clips: &[bounds],
                size,
                centered: false,
                vertical_centered: true,
                color: [255; 3],
                z: 8.1,
            };
            let selection = lettering
                .input_decoration_bounds(label, TextInputState::Selected)
                .unwrap();
            let caret = lettering
                .input_decoration_bounds(label, TextInputState::Caret)
                .unwrap();
            assert_eq!(selection.w, lettering.width(expected, size));
            assert!((caret.x - (bounds.x + selection.w + CARET_GAP)).abs() < 0.0001);
            let mut mesh = LetterMesh::default();
            lettering.append(&mut mesh, label);
            assert!(!mesh.positions.is_empty());
            // Complex emoji still use the same bounded triangle clipping path.
            assert!(mesh.positions.len() < 30_000);
            for position in mesh.positions {
                assert!(position.iter().all(|value| value.is_finite()));
                let x = position[0] * 20.0 + 160.0;
                let y = 90.0 - position[1] * 20.0;
                assert!((bounds.x - 0.0001..=bounds.x + bounds.w + 0.0001).contains(&x));
                assert!((bounds.y - 0.0001..=bounds.y + bounds.h + 0.0001).contains(&y));
            }
        }
    }

    #[test]
    fn wrapping_preserves_graphemes_and_explicit_lines() {
        let lettering = Lettering::default();
        let lines = lettering.typography.lines("Café\nwater water", 20.0, 5.0);
        assert_eq!(lines[0], "Café");
        assert_eq!(lines[1..], ["water", "water"]);
        let combined = "a\u{301}a\u{301}a\u{301}";
        let lines = lettering.typography.lines(combined, 4.0, 5.0);
        assert_eq!(lines.concat(), combined);
        assert!(lines.iter().all(|s| !s.starts_with('\u{301}')));
    }

    #[test]
    fn overflow_preserves_requested_size_and_marks_only_the_last_complete_line() {
        let mut lettering = Lettering::default();
        let text = "MMMMM MMMMM MMMMM MMMMM MMMMM MMMMM MMMMM MMMMM";
        for size in [6.0, 7.8] {
            let bounds = Bounds {
                x: 10.0,
                y: 10.0,
                w: 35.0,
                h: size * 2.4 + 0.01,
            };
            let lines = lettering.layout_lines(text, bounds, size);
            assert_eq!(lines.len(), 2);
            assert!(!lines[0].ends_with('…'));
            assert!(lines[1].ends_with('…'));
            assert!(
                lines
                    .iter()
                    .all(|line| lettering.width(line, size) <= bounds.w)
            );
            let render = |lettering: &mut Lettering, text: &str| {
                let mut mesh = LetterMesh::default();
                lettering.append(
                    &mut mesh,
                    Label {
                        text,
                        bounds,
                        clips: &[bounds],
                        size,
                        centered: false,
                        vertical_centered: false,
                        color: [255; 3],
                        z: 8.1,
                    },
                );
                mesh.positions
            };
            // Rendering the overflowing original must have exactly the same glyph
            // size as explicitly authoring its bounded lines at the requested size.
            let actual = render(&mut lettering, text);
            let expected = render(&mut lettering, &lines.join("\n"));
            assert!(!actual.is_empty());
            assert_eq!(actual, expected);
        }
    }

    #[test]
    fn ellipsis_never_splits_combining_clusters_or_emits_a_partial_line() {
        let lettering = Lettering::default();
        let text = "a\u{301}a\u{301}a\u{301}a\u{301}a\u{301}a\u{301}";
        let bounds = Bounds {
            x: 0.0,
            y: 0.0,
            w: 14.0,
            h: 7.2,
        };
        let lines = lettering.layout_lines(text, bounds, 6.0);
        assert_eq!(lines.len(), 1);
        let visible = lines[0].strip_suffix('…').expect("overflow is visible");
        assert!(
            visible
                .graphemes(true)
                .all(|grapheme| grapheme == "a\u{301}")
        );
        assert!(lettering.width(&lines[0], 6.0) <= bounds.w);
        assert!(
            lettering
                .layout_lines("Tall", Bounds { h: 7.1, ..bounds }, 6.0)
                .is_empty()
        );
        assert_eq!(
            lettering.layout_lines("\nA\n\nB", Bounds { h: 30.0, ..bounds }, 6.0),
            ["", "A", "", "B"]
        );
        assert_eq!(lettering.typography.layout_lines(text, 0.1, 7.2, 6.0), [""]);
    }

    #[test]
    fn editable_tail_uses_measured_width_and_retains_the_newest_glyph_at_both_sizes() {
        let lettering = Lettering::default();
        let draft = format!("{}Z", "WMWiii ".repeat(70));
        for size in [6.0, 7.8] {
            for width in [87.0, 118.0, 286.0] {
                let bounds = Bounds {
                    x: 0.0,
                    y: 0.0,
                    w: width,
                    h: 18.0,
                };
                let tail = lettering.tail_line(&draft, bounds, size);
                assert!(tail.starts_with('…') && tail.ends_with('Z'), "{tail:?}");
                assert!(lettering.width(&tail, size) <= width);
                assert!(draft.ends_with(tail.strip_prefix('…').unwrap()));
                assert_eq!(lettering.layout_lines(&tail, bounds, size), [tail]);
            }
        }
    }

    #[test]
    fn editable_tail_keeps_combining_and_emoji_clusters_whole() {
        let lettering = Lettering::default();
        let draft = format!("{}👩‍🔬a\u{301}Z", "W".repeat(100));
        for size in [6.0, 7.8] {
            for expected in ["…👩‍🔬a\u{301}Z", "…a\u{301}Z", "…Z"] {
                let bounds = Bounds {
                    x: 0.0,
                    y: 0.0,
                    w: lettering.width(expected, size) + 0.001,
                    h: 18.0,
                };
                assert_eq!(lettering.tail_line(&draft, bounds, size), expected);
            }
        }
    }

    #[test]
    fn editable_tail_handles_empty_tiny_and_single_line_bounds() {
        let lettering = Lettering::default();
        let bounds = Bounds {
            x: 0.0,
            y: 0.0,
            w: 200.0,
            h: 18.0,
        };
        assert_eq!(lettering.tail_line("", bounds, 6.0), "");
        assert_eq!(lettering.tail_line("CaféZ", bounds, 6.0), "CaféZ");
        assert_eq!(lettering.tail_line("one\ntwoZ", bounds, 6.0), "one twoZ");
        for invalid in [
            Bounds { w: 0.0, ..bounds },
            Bounds { w: 0.01, ..bounds },
            Bounds { h: 7.1, ..bounds },
        ] {
            assert_eq!(lettering.tail_line("W👩‍🔬Z", invalid, 6.0), "");
        }
        let ellipsis_only = Bounds {
            w: lettering.width("…", 6.0) + 0.001,
            ..bounds
        };
        assert_eq!(lettering.tail_line("👩‍🔬👩‍🔬", ellipsis_only, 6.0), "…");
    }

    #[test]
    fn selection_follows_measured_text_and_uses_the_same_occlusion_clip() {
        let lettering = Lettering::default();
        let bounds = Bounds {
            x: 12.0,
            y: 20.0,
            w: 100.0,
            h: 18.0,
        };
        for size in [6.0, 7.8] {
            let label = Label {
                text: "Wi café",
                bounds,
                clips: &[bounds],
                size,
                centered: false,
                vertical_centered: true,
                color: [25, 103, 101],
                z: 8.018,
            };
            let selected = lettering
                .input_decoration_bounds(label, TextInputState::Selected)
                .unwrap();
            assert_eq!(selected.x, bounds.x);
            assert_eq!(selected.w, lettering.width(label.text, size));
            assert!(
                selected.w < bounds.w * 0.5,
                "selection must not fill the field"
            );
            assert!((selected.h - size * 1.2).abs() < 0.0001);
            assert!((selected.y - (bounds.y + (bounds.h - selected.h) * 0.5)).abs() < 0.0001);
            let clipped = Bounds {
                w: selected.w * 0.5,
                ..bounds
            };
            let mut mesh = LetterMesh::default();
            lettering.append_input_decoration(
                &mut mesh,
                Label {
                    clips: &[clipped],
                    ..label
                },
                TextInputState::Selected,
            );
            assert!(!mesh.positions.is_empty());
            for position in mesh.positions {
                let x = position[0] * 20.0 + 160.0;
                let y = 90.0 - position[1] * 20.0;
                assert!((clipped.x - 0.0001..=clipped.x + clipped.w + 0.0001).contains(&x));
                assert!((selected.y - 0.0001..=selected.y + selected.h + 0.0001).contains(&y));
                assert_eq!(position[2], label.z);
            }
            assert!(
                lettering
                    .input_decoration_bounds(Label { text: "", ..label }, TextInputState::Selected)
                    .is_none()
            );
        }
    }

    #[test]
    fn steady_end_caret_fits_empty_short_and_long_values_at_authored_size() {
        let lettering = Lettering::default();
        let bounds = Bounds {
            x: 12.0,
            y: 20.0,
            w: 87.0,
            h: 18.0,
        };
        for size in [6.0, 7.8] {
            for draft in [
                String::new(),
                "Wi café".into(),
                format!("{}a\u{301}👩‍🔬Z", "Wi ".repeat(100)),
            ] {
                let text = lettering.tail_line(
                    &draft,
                    Bounds {
                        w: bounds.w - CARET_SPACE,
                        ..bounds
                    },
                    size,
                );
                let label = Label {
                    text: &text,
                    bounds,
                    clips: &[bounds],
                    size,
                    centered: false,
                    vertical_centered: true,
                    color: [177, 212, 199],
                    z: 8.02,
                };
                let caret = lettering
                    .input_decoration_bounds(label, TextInputState::Caret)
                    .unwrap();
                assert!((caret.h - size * 1.05).abs() < 0.0001);
                let expected = bounds.x
                    + lettering.width(&text, size)
                    + if text.is_empty() { 0.0 } else { CARET_GAP };
                assert!((caret.x - expected).abs() < 0.0001);
                let mut mesh = LetterMesh::default();
                lettering.append_input_decoration(&mut mesh, label, TextInputState::Caret);
                assert!(!mesh.positions.is_empty());
                for position in mesh.positions {
                    let x = position[0] * 20.0 + 160.0;
                    let y = 90.0 - position[1] * 20.0;
                    assert!((bounds.x - 0.0001..=bounds.x + bounds.w + 0.0001).contains(&x));
                    assert!((bounds.y - 0.0001..=bounds.y + bounds.h + 0.0001).contains(&y));
                }
                let mut hidden = LetterMesh::default();
                lettering.append_input_decoration(
                    &mut hidden,
                    Label {
                        clips: &[],
                        ..label
                    },
                    TextInputState::Caret,
                );
                assert!(hidden.positions.is_empty());
                let tiny = Bounds {
                    w: 0.1,
                    h: 0.1,
                    ..bounds
                };
                let tiny_caret = lettering
                    .input_decoration_bounds(
                        Label {
                            bounds: tiny,
                            ..label
                        },
                        TextInputState::Caret,
                    )
                    .unwrap();
                assert!(tiny_caret.x >= tiny.x && tiny_caret.x + tiny_caret.w <= tiny.x + tiny.w);
                assert!(tiny_caret.y >= tiny.y && tiny_caret.y + tiny_caret.h <= tiny.y + tiny.h);
                assert!(
                    lettering
                        .input_decoration_bounds(
                            Label {
                                bounds: Bounds { w: 0.0, ..bounds },
                                ..label
                            },
                            TextInputState::Caret
                        )
                        .is_none()
                );
            }
        }
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
        for t in mesh.positions.as_chunks::<3>().0.iter() {
            let [a, b, c] = [t[0], t[1], t[2]].map(Vec3::from_array);
            assert!((b - a).cross(c - a).z > 0.0);
        }
    }
}
