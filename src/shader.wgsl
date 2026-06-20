struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@group(0) @binding(0) var glyph_texture: texture_2d<f32>;
@group(0) @binding(1) var glyph_sampler: sampler;

@vertex
fn vs_main(v: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = vec4<f32>(v.position, 0.0, 1.0);
    out.uv = v.uv;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // uv.x < 0 is the sentinel for the cursor bar: render solid white.
    if in.uv.x < -0.5 {
        return vec4<f32>(1.0, 1.0, 1.0, 1.0);
    }
    // R channel holds glyph alpha coverage (R8Unorm atlas).
    let alpha = textureSample(glyph_texture, glyph_sampler, in.uv).r;
    return vec4<f32>(1.0, 1.0, 1.0, alpha);
}
