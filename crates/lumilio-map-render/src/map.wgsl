struct Globals {
    origin: vec2<f32>,
    blocks_per_pixel: f32,
    chunks: f32,
    regions: f32,
    padding: vec3<f32>,
};
@group(0) @binding(0) var tile: texture_2d<f32>;
@group(0) @binding(1) var sampling: sampler;
@group(0) @binding(2) var<uniform> globals: Globals;
struct Vertex { @builtin(position) position: vec4<f32>, @location(0) uv: vec2<f32> };
@vertex fn vs(@builtin(vertex_index) index: u32, @location(0) rect: vec4<f32>) -> Vertex {
    let corners = array<vec2<f32>, 6>(vec2(0,0), vec2(1,0), vec2(0,1), vec2(0,1), vec2(1,0), vec2(1,1));
    let uv = corners[index];
    var result: Vertex;
    result.position = vec4(rect.xy + uv * rect.zw, 0, 1);
    result.uv = uv;
    return result;
}
@fragment fn fs(input: Vertex) -> @location(0) vec4<f32> {
    var color = textureSample(tile, sampling, input.uv);
    let world = globals.origin + input.position.xy * globals.blocks_per_pixel;
    let chunk = abs(world - round(world / 16.0) * 16.0);
    let region = abs(world - round(world / 512.0) * 512.0);
    if globals.chunks > 0 && globals.blocks_per_pixel <= 4.0 && min(chunk.x, chunk.y) < globals.blocks_per_pixel * 0.6 {
        color = vec4(color.rgb * 0.65, 1);
    }
    if globals.regions > 0 && min(region.x, region.y) < globals.blocks_per_pixel * 0.9 {
        color = vec4(color.rgb * 0.35, 1);
    }
    return color;
}
