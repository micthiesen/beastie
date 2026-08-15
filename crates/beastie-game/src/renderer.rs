use std::collections::HashMap;
use std::fs;
use std::path::Path;

use beastie_view::{RectCommand, RenderPlan, SpriteCommand, SpriteFlip, TextCommand};
use font8x8::{BASIC_FONTS, UnicodeFonts};
use ggez::graphics::{
    Canvas, Color, DrawMode, DrawParam, FontData, Image, ImageFormat, Mesh, MeshBuilder, Rect,
    Sampler, Text, TextFragment,
};
use ggez::{Context, GameError, GameResult};
use image::{ColorType, ImageFormat as EncodingFormat};

pub const LOGICAL_WIDTH: f32 = 320.0;
pub const LOGICAL_HEIGHT: f32 = 180.0;

const RUNTIME_SPRITE_IDS: &str = include_str!("../../../assets/runtime-sprites.txt");
const BEASTIE_FONT_NAME: &str = "Beastie Tide";

/// Optional runtime art, decoded and uploaded exactly once during game startup.
/// Each semantic id resolves through `assets/final`, then `assets/generated`.
pub struct AssetCatalog {
    images: HashMap<String, Image>,
    custom_font: bool,
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
        let custom_font = load_font(ctx, assets_root);
        Self {
            images,
            custom_font,
        }
    }

    fn image(&self, command: &SpriteCommand) -> Option<&Image> {
        self.images
            .get(&asset_key(&command.id, Some(command.frame)))
            .or_else(|| self.images.get(&asset_key(&command.id, None)))
            .or_else(|| {
                (matches!(
                    command.id.as_str(),
                    "creature-v1/hover"
                        | "creature-v1/swim"
                        | "creature-v1/eat"
                        | "creature-v1/reject-food"
                        | "creature-v1/sleep"
                        | "creature-v1/play"
                ) || command.id.starts_with("creature-v1/mood/")
                    || command.id.starts_with("creature-v1/talk/")
                    || command.id.starts_with("creature-v1/reaction/"))
                .then(|| self.images.get(&asset_key("creature-v1/base", None)))
                .flatten()
            })
    }

    fn has_base(&self, id: &str) -> bool {
        self.images.contains_key(&asset_key(id, None))
            || self.images.contains_key(&asset_key(id, Some(0)))
    }
}

fn load_font(ctx: &mut Context, assets_root: &Path) -> bool {
    for source in ["final", "generated"] {
        let path = assets_root.join(source).join("ui/beastie-tide.ttf");
        let Ok(bytes) = fs::read(path) else { continue };
        let Ok(font) = FontData::from_vec(bytes) else {
            continue;
        };
        ctx.gfx.add_font(BEASTIE_FONT_NAME, font);
        return true;
    }
    false
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
        if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
            return Self {
                x: 0.0,
                y: 0.0,
                scale: 1.0,
                width: LOGICAL_WIDTH,
                height: LOGICAL_HEIGHT,
            };
        }
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
    pub fn window_dimensions(scale: u8) -> (f32, f32) {
        let scale = f32::from(scale.clamp(1, 6));
        (LOGICAL_WIDTH * scale, LOGICAL_HEIGHT * scale)
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
            draw_text(ctx, canvas, command, assets)?;
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
    let mode = if command.outline {
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
        let integer_scale = f32::from(command.scale.max(1));
        let source = command.source_rect.map(|source| {
            Rect::new(
                source.x as f32 / image.width() as f32,
                source.y as f32 / image.height() as f32,
                source.w as f32 / image.width() as f32,
                source.h as f32 / image.height() as f32,
            )
        });
        let source_width = command
            .source_rect
            .map_or(image.width() as f32, |source| source.w as f32);
        let (destination_x, scale_x) = match command.flip {
            SpriteFlip::None => (x, integer_scale),
            SpriteFlip::Horizontal => (x + source_width * integer_scale, -integer_scale),
        };
        let mut parameters = DrawParam::default()
            .dest([destination_x, y])
            .scale([scale_x, integer_scale]);
        if let Some(source) = source {
            parameters = parameters.src(source);
        }
        canvas.draw(image, parameters);
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
        "aquarium/background" => {
            rectangle(ctx, canvas, x, y, 320.0, 148.0, [20, 63, 83, 255])?;
            rectangle(ctx, canvas, x, y + 82.0, 320.0, 66.0, [27, 82, 91, 255])?;
        }
        "aquarium/cave" => {
            ellipse(
                ctx,
                canvas,
                x + 31.0,
                y + 35.0,
                29.0,
                26.0,
                [34, 40, 52, 255],
            )?;
            ellipse(
                ctx,
                canvas,
                x + 34.0,
                y + 40.0,
                18.0,
                17.0,
                [11, 24, 34, 255],
            )?;
        }
        "aquarium/plants" => {
            for offset in [8.0, 25.0, 44.0, 66.0, 84.0] {
                rectangle(
                    ctx,
                    canvas,
                    x + offset,
                    y + 15.0,
                    3.0,
                    49.0,
                    [48, 118, 85, 255],
                )?;
            }
        }
        "aquarium/toys" => {
            circle(ctx, canvas, x + 16.0, y + 17.0, 10.0, [191, 105, 76, 255])?;
            rectangle(
                ctx,
                canvas,
                x + 46.0,
                y + 8.0,
                20.0,
                18.0,
                [182, 151, 78, 255],
            )?;
        }
        "creature-v1/base"
        | "creature-v1/hover"
        | "creature-v1/swim"
        | "creature-v1/eat"
        | "creature-v1/reject-food"
        | "creature-v1/sleep"
        | "creature-v1/play" => draw_aquatic_creature(ctx, canvas, x, y)?,
        id if id.starts_with("creature-v1/mood/")
            || id.starts_with("creature-v1/talk/")
            || id.starts_with("creature-v1/reaction/") =>
        {
            draw_aquatic_creature(ctx, canvas, x, y)?;
        }
        id if id.starts_with("creature-v1/face/") => {
            draw_expression(ctx, canvas, id, x, y)?;
        }
        id if id.starts_with("creature-v1/gaze/") => draw_gaze(ctx, canvas, id, x, y)?,
        id if id.starts_with("creature-v1/effect/") => {
            circle(ctx, canvas, x + 112.0, y + 35.0, 3.0, [181, 228, 224, 220])?;
            circle(ctx, canvas, x + 122.0, y + 22.0, 2.0, [181, 228, 224, 180])?;
        }
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

fn draw_aquatic_creature(ctx: &mut Context, canvas: &mut Canvas, x: f32, y: f32) -> GameResult {
    ellipse(
        ctx,
        canvas,
        x + 82.0,
        y + 84.0,
        35.0,
        44.0,
        [177, 199, 130, 255],
    )?;
    let tail = [
        [x + 47.0, y + 79.0],
        [x + 20.0, y + 61.0],
        [x + 25.0, y + 98.0],
    ];
    canvas.draw(
        &Mesh::new_polygon(ctx, DrawMode::fill(), &tail, rgba([151, 183, 124, 255]))?,
        DrawParam::default(),
    );
    circle(ctx, canvas, x + 70.0, y + 76.0, 7.0, [230, 224, 180, 255])?;
    circle(ctx, canvas, x + 95.0, y + 76.0, 7.0, [230, 224, 180, 255])?;
    circle(ctx, canvas, x + 70.0, y + 77.0, 3.0, [28, 31, 36, 255])?;
    circle(ctx, canvas, x + 95.0, y + 77.0, 3.0, [28, 31, 36, 255])?;
    Ok(())
}

fn draw_expression(ctx: &mut Context, canvas: &mut Canvas, id: &str, x: f32, y: f32) -> GameResult {
    let color = [28, 31, 36, 255];
    if id.ends_with("sleepy") || id.ends_with("blink") {
        rectangle(ctx, canvas, x + 63.0, y + 76.0, 14.0, 2.0, color)?;
        rectangle(ctx, canvas, x + 88.0, y + 76.0, 14.0, 2.0, color)?;
    }
    if id.ends_with("angry") || id.ends_with("suspicious") {
        rectangle(ctx, canvas, x + 62.0, y + 65.0, 15.0, 2.0, color)?;
        rectangle(ctx, canvas, x + 89.0, y + 65.0, 15.0, 2.0, color)?;
    }
    if id.ends_with("delighted") || id.ends_with("smug") {
        rectangle(ctx, canvas, x + 77.0, y + 94.0, 14.0, 3.0, color)?;
    } else if id.ends_with("lonely") || id.ends_with("wary") {
        rectangle(ctx, canvas, x + 78.0, y + 98.0, 12.0, 2.0, color)?;
    }
    Ok(())
}

fn draw_gaze(ctx: &mut Context, canvas: &mut Canvas, id: &str, x: f32, y: f32) -> GameResult {
    let (dx, dy) = if id.ends_with("left") {
        (-2.0, 0.0)
    } else if id.ends_with("right") || id.ends_with("player") {
        (2.0, 0.0)
    } else if id.ends_with("up") {
        (0.0, -2.0)
    } else if id.ends_with("down") {
        (0.0, 2.0)
    } else {
        (0.0, 0.0)
    };
    circle(
        ctx,
        canvas,
        x + 70.0 + dx,
        y + 77.0 + dy,
        2.0,
        [8, 13, 19, 255],
    )?;
    circle(
        ctx,
        canvas,
        x + 95.0 + dx,
        y + 77.0 + dy,
        2.0,
        [8, 13, 19, 255],
    )?;
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

fn draw_text(
    ctx: &mut Context,
    canvas: &mut Canvas,
    command: &TextCommand,
    assets: &AssetCatalog,
) -> GameResult {
    if assets.custom_font {
        let maximum_width = if command.id == "speech/text" {
            140.0
        } else {
            (315 - command.x) as f32
        };
        let mut rendered = Text::new(
            TextFragment::new(command.text.clone())
                .font(BEASTIE_FONT_NAME)
                .scale(8.0 * f32::from(command.scale.clamp(1, 2)))
                .color(text_color(&command.id)),
        );
        rendered.set_bounds([maximum_width, f32::INFINITY]);
        canvas.draw(
            &rendered,
            DrawParam::default().dest([command.x as f32, command.y as f32]),
        );
        return Ok(());
    }
    let pixel_scale = i32::from(command.scale.clamp(1, 2));
    let glyph_advance = 6 * pixel_scale;
    let line_advance = 8 * pixel_scale;
    let maximum_width = if command.id == "speech/text" {
        140
    } else {
        315 - command.x
    };
    let mut cursor_x = 0;
    let mut cursor_y = 0;
    let mut builder = MeshBuilder::new();
    for character in command.text.chars() {
        if character == '\n' || cursor_x + glyph_advance > maximum_width {
            cursor_x = 0;
            cursor_y += line_advance;
            if character == '\n' {
                continue;
            }
        }
        let glyph = BASIC_FONTS.get(character).unwrap_or_else(|| {
            BASIC_FONTS
                .get('?')
                .expect("the built-in bitmap font contains a fallback glyph")
        });
        for (row, bits) in glyph.into_iter().enumerate() {
            for column in 0_u8..5 {
                if bits & (1 << column) != 0 {
                    builder.rectangle(
                        DrawMode::fill(),
                        Rect::new(
                            (command.x + cursor_x + i32::from(column) * pixel_scale) as f32,
                            (command.y
                                + cursor_y
                                + i32::try_from(row).unwrap_or_default() * pixel_scale)
                                as f32,
                            pixel_scale as f32,
                            pixel_scale as f32,
                        ),
                        Color::from_rgb(238, 224, 194),
                    )?;
                }
            }
        }
        cursor_x += glyph_advance;
    }
    canvas.draw(&Mesh::from_data(ctx, builder.build()), DrawParam::default());
    Ok(())
}

fn text_color(id: &str) -> Color {
    if id.ends_with("/title") || id.contains("warning") {
        Color::from_rgb(239, 201, 123)
    } else if id.contains("status/") || id.contains("summary-behavior") {
        Color::from_rgb(137, 190, 182)
    } else if id.contains("input-text") {
        Color::from_rgb(202, 225, 209)
    } else if id.ends_with("-value") {
        Color::from_rgb(239, 174, 130)
    } else {
        Color::from_rgb(241, 224, 183)
    }
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

    #[test]
    fn supported_window_sizes_are_exact_integer_multiples() {
        for scale in 1..=6 {
            let (width, height) = Viewport::window_dimensions(scale);
            let viewport = Viewport::for_drawable(width, height);
            assert_eq!(viewport.scale, f32::from(scale));
            assert_eq!((viewport.x, viewport.y), (0.0, 0.0));
        }
    }

    #[test]
    fn invalid_transient_drawable_sizes_are_safe() {
        for (width, height) in [(0.0, 0.0), (f32::NAN, 100.0), (100.0, f32::INFINITY)] {
            assert_eq!(Viewport::for_drawable(width, height).scale, 1.0);
        }
    }
}
