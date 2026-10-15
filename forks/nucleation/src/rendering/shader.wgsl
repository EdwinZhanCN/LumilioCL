// Nucleation headless render shader
// Supports: texture atlas, vertex colors, directional lighting, HDRI skybox + IBL.

struct Uniforms {
    view_proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    // x: alpha_cutoff, y: hdri_enabled (>0.5 = yes), z: hdri_intensity, w: ambient
    params: vec4<f32>,
    // xyz: world-space direction toward the light, w: directional intensity
    light: vec4<f32>,
    // LumilioCL local change (plan schematic-studio T31, D30): the focus
    // mask's first cell (xyz) and the mode (w: 0 off, 1 fade, 2 hide).
    focus: vec4<f32>,
    // xyz: the mask's size in cells; w: how far an unfocused colour fades.
    focus_size: vec4<f32>,
    // rgb: the colour an unfocused block fades toward (the background).
    focus_tint: vec4<f32>,
    // LumilioCL local change (plan schematic-studio T32): the outlined
    // cell (xyz; w > 0.5 when there is one), its block's box in world space
    // (xyz of the two; the first's w is the band's width in pixels) and the
    // outline's colour.
    mark: vec4<f32>,
    mark_min: vec4<f32>,
    mark_max: vec4<f32>,
    mark_color: vec4<f32>,
    // LumilioCL local change (plan schematic-studio T25): the slice (x: the
    // axis, 0 X, 1 Y, 2 Z; y: the last layer drawn; w > 0.5 when slicing),
    // and the selection's first and last cells (xyz; the first's w > 0.5
    // when there is one) with its colour.
    clip: vec4<f32>,
    selection_min: vec4<f32>,
    selection_max: vec4<f32>,
    selection_color: vec4<f32>,
};

// Per-draw animation state. Written by the animation layer as a `Pose`; an
// un-animated draw supplies the identity value, which is a no-op in the maths
// below so posed and un-posed rendering agree bit for bit.
struct DrawUniforms {
    model: mat4x4<f32>,
    // Inverse-transpose of the model's upper 3x3. Required, or rotated and
    // non-uniformly scaled geometry shades wrong.
    normal_mat: mat3x3<f32>,
    // Multiplied into the base colour. Identity is (1, 1, 1, 1).
    tint: vec4<f32>,
    // Added after lighting. Identity is (0, 0, 0, 0).
    emissive: vec4<f32>,
};

@group(0) @binding(0) var<uniform> uniforms: Uniforms;
// One byte per schematic cell: non-zero where the focused block kind is.
@group(0) @binding(1) var focus_mask: texture_3d<u32>;
@group(1) @binding(0) var atlas_texture: texture_2d<f32>;
@group(1) @binding(1) var atlas_sampler: sampler;
@group(2) @binding(0) var hdri_texture: texture_2d<f32>;
@group(2) @binding(1) var hdri_sampler: sampler;
@group(3) @binding(0) var<uniform> draw: DrawUniforms;

// ─── Mesh rendering ─────────────────────────────────────────────────────────

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_normal: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) world_position: vec3<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let world = draw.model * vec4<f32>(in.position, 1.0);
    out.clip_position = uniforms.view_proj * world;
    out.world_normal = draw.normal_mat * in.normal;
    out.uv = in.uv;
    out.color = in.color;
    out.world_position = world.xyz;
    return out;
}

// Entity instances (LumilioCL local change, plan schematic-studio T8): one
// model drawn many times, each instance with its own model matrix. The matrix
// only turns about Y and moves, so it turns normals as it is.
struct InstanceInput {
    @location(4) m0: vec4<f32>,
    @location(5) m1: vec4<f32>,
    @location(6) m2: vec4<f32>,
    @location(7) m3: vec4<f32>,
};

@vertex
fn vs_instanced(in: VertexInput, instance: InstanceInput) -> VertexOutput {
    var out: VertexOutput;
    let model = mat4x4<f32>(instance.m0, instance.m1, instance.m2, instance.m3);
    let world = model * vec4<f32>(in.position, 1.0);
    out.clip_position = uniforms.view_proj * world;
    out.world_normal = (model * vec4<f32>(in.normal, 0.0)).xyz;
    out.uv = in.uv;
    out.color = in.color;
    out.world_position = world.xyz;
    return out;
}

// Whether the surface at `world` (facing `n`) belongs to a focused cell: the
// point is moved a hair against its normal so a face on a block's boundary
// counts for the block it closes. The mesher puts block `n` at
// `n - 0.5 ..= n + 0.5`, hence the half (plan schematic-studio T31, D30).
fn in_focus(world: vec3<f32>, n: vec3<f32>) -> bool {
    let cell = floor(world + vec3<f32>(0.5) - n * 0.01) - uniforms.focus.xyz;
    if any(cell < vec3<f32>(0.0)) || any(cell >= uniforms.focus_size.xyz) {
        return false;
    }
    return textureLoad(focus_mask, vec3<i32>(cell), 0).r != 0u;
}

// Sample HDRI equirectangular map from a world-space direction.
fn sample_hdri(dir: vec3<f32>) -> vec3<f32> {
    let d = normalize(dir);
    let u = atan2(d.z, d.x) * 0.15915494 + 0.5; // 1/(2*pi)
    let v = acos(clamp(d.y, -1.0, 1.0)) * 0.31830989; // 1/pi
    return textureSampleLevel(hdri_texture, hdri_sampler, vec2<f32>(u, v), 0.0).rgb;
}

// Rough diffuse IBL: sample HDRI in the normal direction (approximation).
fn hdri_diffuse(normal: vec3<f32>) -> vec3<f32> {
    // Sample at a higher mip (or just the base — equirect doesn't have mips,
    // so this is a rough approximation). For proper IBL we'd precompute an
    // irradiance map, but for a PoC this looks decent.
    return sample_hdri(normal);
}

// Translucent layer: alpha-blended.
@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return shade(in);
}

// Opaque and cutout layers: drawn without blending, so a cutout pixel that
// survives the alpha test is written fully opaque.
@fragment
fn fs_solid(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(shade(in).rgb, 1.0);
}

fn shade(in: VertexOutput) -> vec4<f32> {
    // How far one pixel reaches, for the outline; derivatives are taken
    // while control flow is still uniform.
    let pixel = fwidth(in.world_position);
    let lit = shade_lit(in);
    let n = normalize(in.world_normal);
    // A slice draws its layer and those below it only; the cell a surface
    // closes decides, as for the focus.
    let cell = floor(in.world_position + vec3<f32>(0.5) - n * 0.01);
    if uniforms.clip.w > 0.5 {
        var along = cell.x;
        if uniforms.clip.x > 1.5 {
            along = cell.z;
        } else if uniforms.clip.x > 0.5 {
            along = cell.y;
        }
        if along > uniforms.clip.y + 0.5 {
            discard;
        }
    }
    var color = lit;
    let mode = uniforms.focus.w;
    if mode > 0.5 && !in_focus(in.world_position, n) {
        if mode > 1.5 {
            discard;
        }
        // Unfocused: greyed and washed toward the background.
        let grey = vec3<f32>(dot(lit.rgb, vec3<f32>(0.299, 0.587, 0.114)));
        let faded = mix(mix(lit.rgb, grey, 0.8), uniforms.focus_tint.rgb, uniforms.focus_size.w);
        color = vec4<f32>(faded, lit.a);
    }
    if uniforms.selection_min.w > 0.5 {
        color = selected(color, cell, in.world_position, n, pixel);
    }
    if uniforms.mark.w > 0.5 {
        color = outline(color, in.world_position, n, pixel);
    }
    return color;
}

// The selection (LumilioCL local change, plan schematic-studio T25): the
// cells in it are washed a little toward its colour, and a band runs along
// the edges of its box, so it reads apart from the inspected block's.
fn selected(color: vec4<f32>, cell: vec3<f32>, world: vec3<f32>, n: vec3<f32>, pixel: vec3<f32>) -> vec4<f32> {
    let low = uniforms.selection_min.xyz;
    let high = uniforms.selection_max.xyz;
    if any(cell < low) || any(cell > high) {
        return color;
    }
    let edge = min(world - (low - vec3<f32>(0.5)), (high + vec3<f32>(0.5)) - world)
        / max(pixel, vec3<f32>(1e-6));
    let across = select(abs(edge), vec3<f32>(1e6), abs(n) > vec3<f32>(0.5));
    if min(across.x, min(across.y, across.z)) < uniforms.mark_min.w {
        return vec4<f32>(uniforms.selection_color.rgb, 1.0);
    }
    return vec4<f32>(mix(color.rgb, uniforms.selection_color.rgb, 0.18), color.a);
}

// The outlined cell (LumilioCL local change, plan schematic-studio T32): on
// each face its block shows, a band along the edges of the block's box in
// the mark colour, with a rim of the background inside it so it reads on
// any texture. The axis a face looks along does not count, so a slab's top
// face is outlined all round. A surface belongs to the cell as in
// `in_focus`.
fn outline(color: vec4<f32>, world: vec3<f32>, n: vec3<f32>, pixel: vec3<f32>) -> vec4<f32> {
    let inside = world + vec3<f32>(0.5) - n * 0.01 - uniforms.mark.xyz;
    if any(inside < vec3<f32>(0.0)) || any(inside >= vec3<f32>(1.0)) {
        return color;
    }
    let edge = min(world - uniforms.mark_min.xyz, uniforms.mark_max.xyz - world)
        / max(pixel, vec3<f32>(1e-6));
    let across = select(abs(edge), vec3<f32>(1e6), abs(n) > vec3<f32>(0.5));
    let reach = min(across.x, min(across.y, across.z));
    let width = uniforms.mark_min.w;
    if reach < width {
        return vec4<f32>(uniforms.mark_color.rgb, 1.0);
    }
    if reach < width * 1.5 {
        return vec4<f32>(uniforms.focus_tint.rgb, 1.0);
    }
    return color;
}

fn shade_lit(in: VertexOutput) -> vec4<f32> {
    let tex_color = textureSample(atlas_texture, atlas_sampler, in.uv);
    let material_alpha = tex_color.a * in.color.a;
    let base_color = tex_color * in.color * draw.tint;
    if base_color.a <= 0.0001 {
        discard;
    }

    // Cutout classification depends on the material, not animation opacity.
    let alpha_cutoff = uniforms.params.x;
    if alpha_cutoff > 0.0 && material_alpha < alpha_cutoff {
        discard;
    }

    let n = normalize(in.world_normal);
    let hdri_enabled = uniforms.params.y > 0.5;
    let hdri_intensity = uniforms.params.z;
    let light_dir = normalize(uniforms.light.xyz);
    let directional_intensity = uniforms.light.w;

    var lighting: f32;
    var ambient_color: vec3<f32>;

    if hdri_enabled {
        // Image-based lighting from HDRI
        let ibl = hdri_diffuse(n) * hdri_intensity;
        // Key light for definition
        let n_dot_l = max(dot(n, light_dir), 0.0);
        // Minimum ambient floor to prevent total darkness
        let min_ambient = vec3<f32>(0.15);
        ambient_color = base_color.rgb * max(ibl + 0.35 * directional_intensity * n_dot_l, min_ambient);
        // Tonemap mesh colors too (matches skybox)
        let mapped = ambient_color / (ambient_color + vec3<f32>(1.0));
        return vec4<f32>(mapped + draw.emissive.rgb, base_color.a);
    } else {
        // Fallback: simple directional lighting
        let n_dot_l = max(dot(n, light_dir), 0.0);
        let ambient = uniforms.params.w;
        lighting = ambient + (1.0 - ambient) * directional_intensity * n_dot_l;
        return vec4<f32>(base_color.rgb * lighting + draw.emissive.rgb, base_color.a);
    }
}

// ─── Grid / axis lines ──────────────────────────────────────────────────────
// A world-space reference grid, drawn with the scene's view-projection so it
// sits under the build and shares its perspective. No lighting or texturing —
// each vertex just carries its own colour.

struct LineVertex {
    @location(0) position: vec3<f32>,
    @location(1) color: vec4<f32>,
};

struct LineOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_line(in: LineVertex) -> LineOutput {
    var out: LineOutput;
    out.clip_position = uniforms.view_proj * vec4<f32>(in.position, 1.0);
    out.color = in.color;
    return out;
}

@fragment
fn fs_line(in: LineOutput) -> @location(0) vec4<f32> {
    return in.color;
}

// ─── Skybox rendering ───────────────────────────────────────────────────────

struct SkyVertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) clip_pos: vec2<f32>,
};

// Fullscreen triangle (3 vertices, no vertex buffer needed)
@vertex
fn vs_sky(@builtin(vertex_index) vertex_index: u32) -> SkyVertexOutput {
    var out: SkyVertexOutput;
    // Generate fullscreen triangle: covers (-1,-1) to (1,1)
    let x = f32(i32(vertex_index & 1u)) * 4.0 - 1.0;
    let y = f32(i32(vertex_index >> 1u)) * 4.0 - 1.0;
    out.clip_position = vec4<f32>(x, y, 0.0, 1.0); // farthest depth (reversed depth)
    out.clip_pos = vec2<f32>(x, y);
    return out;
}

@fragment
fn fs_sky(in: SkyVertexOutput) -> @location(0) vec4<f32> {
    // Reconstruct the world-space view ray from two depths on this pixel
    // (reversed depth: 1 is the near plane, 0.5 lies beyond it).
    let near_h = uniforms.inv_view_proj * vec4<f32>(in.clip_pos.x, in.clip_pos.y, 1.0, 1.0);
    let far_h = uniforms.inv_view_proj * vec4<f32>(in.clip_pos.x, in.clip_pos.y, 0.5, 1.0);
    let dir = normalize(far_h.xyz / far_h.w - near_h.xyz / near_h.w);

    let color = sample_hdri(dir);

    // Tonemap (Reinhard) for HDR → LDR
    let mapped = color / (color + vec3<f32>(1.0));

    return vec4<f32>(mapped, 1.0);
}
