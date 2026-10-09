struct Params { opacity: vec4<f32> };
@group(0) @binding(0) var image: texture_2d<f32>;
@group(0) @binding(1) var sampling: sampler;
@group(0) @binding(2) var<uniform> params: Params;
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
    let color = textureSample(image, sampling, input.uv);
    return vec4(color.rgb, color.a * params.opacity.x);
}
