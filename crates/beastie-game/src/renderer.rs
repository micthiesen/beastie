use std::collections::HashMap;
use std::fs;
use std::path::Path;

use beastie_view::{RectCommand, RenderPlan, SpriteCommand, TextCommand};
use ggez::graphics::{
    Canvas, Color, DrawMode, DrawParam, Image, ImageFormat, Mesh, Rect, Sampler, Text,
};
use ggez::{Context, GameError, GameResult};
use image::{ColorType, ImageFormat as EncodingFormat};

pub const LOGICAL_WIDTH: f32 = 320.0;
pub const LOGICAL_HEIGHT: f32 = 180.0;

const RUNTIME_SPRITE_IDS: &str = include_str!("../../../assets/runtime-sprites.txt");

/// Optional runtime art, decoded and uploaded exactly once during game startup.
/// Each semantic id resolves through `assets/final`, then `assets/generated`.
pub struct AssetCatalog {
    images: HashMap<String, Image>,
}

impl AssetCatalog {
    #[must_use]
    pub fn load(ctx: &mut Context, assets_root: &Path) -> Self {
        let mut images = HashMap::new();
        for id in RUNTIME_SPRITE_IDS.lines().filter(|id| !id.is_empty()) {
            load_variant(ctx, assets_root, id, None, &mut images);
            for frame in 0..=3 {
                load_variant(ctx, assets_root, id, Some(frame), &mut images);
            }
        }
        Self { images }
    }

    fn image(&self, command: &SpriteCommand) -> Option<&Image> {
        self.images
            .get(&asset_key(&command.id, Some(command.frame)))
            .or_else(|| self.images.get(&asset_key(&command.id, None)))
    }

    fn has_base(&self, id: &str) -> bool {
        self.images.contains_key(&asset_key(id, None))
            || self.images.contains_key(&asset_key(id, Some(0)))
    }
}

fn load_variant(
    ctx: &mut Context,
    assets_root: &Path,
    id: &str,
    frame: Option<u8>,
    images: &mut HashMap<String, Image>,
) {
    let relative = asset_relative_path(id, frame);
    for source in ["final", "generated"] {
        let path = assets_root.join(source).join(&relative);
        let Ok(encoded) = fs::read(path) else {
            continue;
        };
        let Ok(image) = Image::from_bytes(ctx, &encoded) else {
            continue;
        };
        images.insert(asset_key(id, frame), image);
        return;
    }
}

fn asset_relative_path(id: &str, frame: Option<u8>) -> std::path::PathBuf {
    let suffix = frame.map_or_else(String::new, |frame| format!("-{frame}"));
    std::path::PathBuf::from(format!("{id}{suffix}.png"))
}

fn asset_key(id: &str, frame: Option<u8>) -> String {
    frame.map_or_else(|| id.to_owned(), |frame| format!("{id}#{frame}"))
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Viewport {
    pub x: f32,
    pub y: f32,
    pub scale: f32,
    pub width: f32,
    pub height: f32,
}

impl Viewport {
    #[must_use]
    pub fn for_drawable(width: f32, height: f32) -> Self {
        let scale = ((width / LOGICAL_WIDTH).min(height / LOGICAL_HEIGHT))
            .floor()
            .max(1.0);
        let viewport_width = LOGICAL_WIDTH * scale;
        let viewport_height = LOGICAL_HEIGHT * scale;
        Self {
            x: ((width - viewport_width) / 2.0).floor(),
            y: ((height - viewport_height) / 2.0).floor(),
            scale,
            width: viewport_width,
            height: viewport_height,
        }
    }

    #[must_use]
    pub fn logical_point(self, physical_x: f32, physical_y: f32) -> Option<(f32, f32)> {
        if physical_x < self.x
            || physical_y < self.y
            || physical_x >= self.x + self.width
            || physical_y >= self.y + self.height
        {
            return None;
        }
        Some((
            (physical_x - self.x) / self.scale,
            (physical_y - self.y) / self.scale,
        ))
    }
}

pub fn save_logical_png(ctx: &Context, frame: &Image, path: &Path) -> GameResult {
    let mut pixels = frame.to_pixels(ctx)?;
    match frame.format() {
        ImageFormat::Rgba8Unorm | ImageFormat::Rgba8UnormSrgb => {}
        ImageFormat::Bgra8Unorm | ImageFormat::Bgra8UnormSrgb => {
            for pixel in pixels.chunks_exact_mut(4) {
                pixel.swap(0, 2);
            }
        }
        format => {
            return Err(GameError::RenderError(format!(
                "cannot capture logical framebuffer format {format:?}"
            )));
        }
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| GameError::FilesystemError(error.to_string()))?;
    }
    image::save_buffer_with_format(
        path,
        &pixels,
        frame.width(),
        frame.height(),
        ColorType::Rgba8,
        EncodingFormat::Png,
    )
    .map_err(|error| GameError::ResourceLoadError(error.to_string()))
}

pub fn execute_plan(
    ctx: &mut Context,
    canvas: &mut Canvas,
    plan: &RenderPlan,
    assets: &AssetCatalog,
) -> GameResult {
    canvas.set_sampler(Sampler::nearest_clamp());
    let min_layer = plan
        .sprites
        .iter()
        .map(|command| command.layer)
        .chain(plan.rects.iter().map(|command| command.layer))
        .chain(plan.text.iter().map(|command| command.layer))
        .min()
        .unwrap_or(0);
    let max_layer = plan
        .sprites
        .iter()
        .map(|command| command.layer)
        .chain(plan.rects.iter().map(|command| command.layer))
        .chain(plan.text.iter().map(|command| command.layer))
        .max()
        .unwrap_or(0);
    for layer in min_layer..=max_layer {
        for command in plan.rects.iter().filter(|command| command.layer == layer) {
            draw_rect(ctx, canvas, command)?;
        }
        for command in plan.sprites.iter().filter(|command| command.layer == layer) {
            draw_sprite(ctx, canvas, command, assets)?;
        }
        for command in plan.text.iter().filter(|command| command.layer == layer) {
            draw_text(canvas, command);
        }
    }
    Ok(())
}

fn draw_rect(ctx: &mut Context, canvas: &mut Canvas, command: &RectCommand) -> GameResult {
    let rect = Rect::new(
        command.rect.x as f32,
        command.rect.y as f32,
        command.rect.w as f32,
        command.rect.h as f32,
    );
    let color = Color::from_rgba(
        command.color[0],
        command.color[1],
        command.color[2],
        command.color[3],
    );
    let mode = if command.id == "ui/focus" {
        DrawMode::stroke(1.0)
    } else {
        DrawMode::fill()
    };
    canvas.draw(
        &Mesh::new_rectangle(ctx, mode, rect, color)?,
        DrawParam::default(),
    );
    Ok(())
}

fn draw_sprite(
    ctx: &mut Context,
    canvas: &mut Canvas,
    command: &SpriteCommand,
    assets: &AssetCatalog,
) -> GameResult {
    let x = command.x as f32;
    let y = command.y as f32;
    if let Some(image) = assets.image(command) {
        canvas.draw(image, DrawParam::default().dest([x, y]));
        return Ok(());
    }
    // The canonical room background already includes these fixtures. Their
    // fallback shapes are only needed when the background itself falls back.
    if assets.has_base("room/background")
        && matches!(command.id.as_str(), "room/window" | "room/bed")
    {
        return Ok(());
    }
    match command.id.as_str() {
        "room/background" => {
            rectangle(ctx, canvas, x, y, 320.0, 180.0, [73, 55, 65, 255])?;
            rectangle(ctx, canvas, 0.0, 105.0, 320.0, 75.0, [104, 73, 61, 255])?;
            rectangle(ctx, canvas, 0.0, 103.0, 320.0, 3.0, [45, 36, 43, 255])?;
        }
        "room/window" => {
            rectangle(ctx, canvas, x, y, 62.0, 55.0, [43, 35, 44, 255])?;
            rectangle(
                ctx,
                canvas,
                x + 5.0,
                y + 5.0,
                52.0,
                43.0,
                [76, 105, 124, 255],
            )?;
            rectangle(ctx, canvas, x + 30.0, y + 5.0, 2.0, 43.0, [43, 35, 44, 255])?;
            rectangle(ctx, canvas, x + 5.0, y + 25.0, 52.0, 2.0, [43, 35, 44, 255])?;
        }
        "room/bed" => {
            rectangle(ctx, canvas, x, y + 17.0, 62.0, 22.0, [67, 45, 47, 255])?;
            rounded_rectangle(
                ctx,
                canvas,
                x + 4.0,
                y + 10.0,
                54.0,
                18.0,
                5.0,
                [145, 91, 79, 255],
            )?;
        }
        "room/bowl" => ellipse(ctx, canvas, x + 15.0, y + 8.0, 17.0, 7.0, [58, 49, 57, 255])?,
        "room/toy" => {
            circle(ctx, canvas, x + 8.0, y + 8.0, 8.0, [191, 105, 76, 255])?;
            rectangle(ctx, canvas, x + 7.0, y, 2.0, 16.0, [236, 177, 93, 255])?;
        }
        "room/clutter" => {
            rectangle(ctx, canvas, x, y + 8.0, 23.0, 12.0, [51, 42, 47, 255])?;
            rectangle(ctx, canvas, x + 14.0, y, 18.0, 17.0, [85, 68, 64, 255])?;
        }
        "room/clutter-tidy" => {
            rectangle(ctx, canvas, x + 7.0, y + 10.0, 24.0, 9.0, [68, 54, 58, 255])?
        }
        id if id.starts_with("food/") => {
            let color = match id {
                "food/berry" => [143, 52, 76, 255],
                "food/mushroom" => [200, 170, 122, 255],
                _ => [174, 137, 72, 255],
            };
            circle(ctx, canvas, x + 4.0, y + 4.0, 4.0, color)?;
        }
        id if id.starts_with("creature/") => draw_creature(ctx, canvas, id, x, y)?,
        _ => {}
    }
    Ok(())
}

fn draw_creature(ctx: &mut Context, canvas: &mut Canvas, pose: &str, x: f32, y: f32) -> GameResult {
    let body = if pose == "creature/annoyed" {
        [176, 154, 100, 255]
    } else {
        [177, 199, 130, 255]
    };
    let squash = if pose == "creature/sleep" { 10.0 } else { 15.0 };
    ellipse(ctx, canvas, x + 16.0, y + 19.0, 15.0, squash, body)?;
    let ear = [[x + 5.0, y + 9.0], [x + 9.0, y], [x + 13.0, y + 10.0]];
    let other_ear = [[x + 19.0, y + 10.0], [x + 24.0, y], [x + 28.0, y + 11.0]];
    canvas.draw(
        &Mesh::new_polygon(ctx, DrawMode::fill(), &ear, rgba(body))?,
        DrawParam::default(),
    );
    canvas.draw(
        &Mesh::new_polygon(ctx, DrawMode::fill(), &other_ear, rgba(body))?,
        DrawParam::default(),
    );
    if pose == "creature/sleep" {
        rectangle(ctx, canvas, x + 7.0, y + 16.0, 7.0, 1.0, [32, 31, 35, 255])?;
        rectangle(ctx, canvas, x + 20.0, y + 16.0, 7.0, 1.0, [32, 31, 35, 255])?;
    } else {
        circle(ctx, canvas, x + 11.0, y + 15.0, 2.0, [30, 29, 33, 255])?;
        circle(ctx, canvas, x + 23.0, y + 15.0, 2.0, [30, 29, 33, 255])?;
        if pose == "creature/annoyed" {
            rectangle(ctx, canvas, x + 7.0, y + 10.0, 8.0, 1.0, [30, 29, 33, 255])?;
            rectangle(ctx, canvas, x + 20.0, y + 10.0, 8.0, 1.0, [30, 29, 33, 255])?;
        }
    }
    Ok(())
}

fn draw_text(canvas: &mut Canvas, command: &TextCommand) {
    let mut text = Text::new(command.text.as_str());
    text.set_scale(if command.id == "speech/text" {
        9.0
    } else {
        8.0
    });
    if command.id == "speech/text" {
        text.set_bounds([150.0, 36.0]);
    }
    canvas.draw(
        &text,
        DrawParam::default()
            .dest([command.x as f32, command.y as f32])
            .color(Color::from_rgb(238, 224, 194)),
    );
}

fn rectangle(
    ctx: &mut Context,
    canvas: &mut Canvas,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    color: [u8; 4],
) -> GameResult {
    canvas.draw(
        &Mesh::new_rectangle(ctx, DrawMode::fill(), Rect::new(x, y, w, h), rgba(color))?,
        DrawParam::default(),
    );
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn rounded_rectangle(
    ctx: &mut Context,
    canvas: &mut Canvas,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    radius: f32,
    color: [u8; 4],
) -> GameResult {
    canvas.draw(
        &Mesh::new_rounded_rectangle(
            ctx,
            DrawMode::fill(),
            Rect::new(x, y, w, h),
            radius,
            rgba(color),
        )?,
        DrawParam::default(),
    );
    Ok(())
}

fn circle(
    ctx: &mut Context,
    canvas: &mut Canvas,
    x: f32,
    y: f32,
    radius: f32,
    color: [u8; 4],
) -> GameResult {
    canvas.draw(
        &Mesh::new_circle(ctx, DrawMode::fill(), [x, y], radius, 0.5, rgba(color))?,
        DrawParam::default(),
    );
    Ok(())
}

fn ellipse(
    ctx: &mut Context,
    canvas: &mut Canvas,
    x: f32,
    y: f32,
    radius_x: f32,
    radius_y: f32,
    color: [u8; 4],
) -> GameResult {
    canvas.draw(
        &Mesh::new_ellipse(
            ctx,
            DrawMode::fill(),
            [x, y],
            radius_x,
            radius_y,
            0.5,
            rgba(color),
        )?,
        DrawParam::default(),
    );
    Ok(())
}

fn rgba(color: [u8; 4]) -> Color {
    Color::from_rgba(color[0], color[1], color[2], color[3])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letterboxes_at_integer_scale_and_maps_inside_points() {
        let viewport = Viewport::for_drawable(1_000.0, 700.0);
        assert_eq!(viewport.scale, 3.0);
        assert_eq!(viewport.width, 960.0);
        assert_eq!(viewport.height, 540.0);
        assert_eq!(viewport.logical_point(20.0, 80.0), Some((0.0, 0.0)));
        assert_eq!(
            viewport.logical_point(979.0, 619.0),
            Some((959.0 / 3.0, 539.0 / 3.0))
        );
        assert_eq!(viewport.logical_point(19.0, 80.0), None);
        assert_eq!(viewport.logical_point(20.0, 620.0), None);
    }

    #[test]
    fn asset_paths_preserve_semantic_ids_and_frame_suffixes() {
        assert_eq!(
            asset_relative_path("creature/idle", None),
            std::path::PathBuf::from("creature/idle.png")
        );
        assert_eq!(
            asset_relative_path("creature/walk", Some(1)),
            std::path::PathBuf::from("creature/walk-1.png")
        );
    }
}
