@group(0) @binding(0) var radiance: texture_2d<f32>;
@fragment
fn fragment(@builtin(position) p: vec4<f32>) -> @location(0) vec4<f32> {
    // Preserve exact glyph coverage; broad bright-pass taps echoed fine UI lettering.
    return textureLoad(radiance, vec2<i32>(p.xy), 0);
}
