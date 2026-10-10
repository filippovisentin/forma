// Forma viewport shader: headlight-shaded surfaces and unlit lines.

struct Uniforms {
    view_proj: mat4x4<f32>,
    // xyz = eye position (perspective) or view direction towards the scene (ortho), w = 1 for ortho
    eye: vec4<f32>,
    // Clipping plane in scene coordinates: xyz = normal, w = offset; everything
    // on the normal's side is cut away. A zero normal disables it.
    clip: vec4<f32>,
};

fn clipped(p: vec3<f32>) -> bool {
    return dot(u.clip.xyz, u.clip.xyz) > 0.0 && dot(u.clip.xyz, p) + u.clip.w > 0.0;
}

@group(0) @binding(0) var<uniform> u: Uniforms;

struct MeshIn {
    @location(0) pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec4<f32>,
};

struct MeshOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec4<f32>,
};

@vertex
fn vs_mesh(v: MeshIn) -> MeshOut {
    var o: MeshOut;
    o.clip = u.view_proj * vec4<f32>(v.pos, 1.0);
    o.world = v.pos;
    o.normal = v.normal;
    o.color = v.color;
    return o;
}

fn shade(i: MeshOut) -> vec4<f32> {
    var n = i.normal;
    if (dot(n, n) < 1e-12) {
        // No normal available: derive a flat one from screen-space derivatives.
        n = cross(dpdx(i.world), dpdy(i.world));
    }
    n = normalize(n);
    var to_eye: vec3<f32>;
    if (u.eye.w > 0.5) {
        to_eye = normalize(-u.eye.xyz);
    } else {
        to_eye = normalize(u.eye.xyz - i.world);
    }
    // Two-sided headlight, like Rhino's default shaded mode.
    let d = abs(dot(n, to_eye));
    let k = 0.30 + 0.70 * d;
    return vec4<f32>(i.color.rgb * k, i.color.a);
}

@fragment
fn fs_mesh(i: MeshOut) -> @location(0) vec4<f32> {
    if (clipped(i.world)) {
        discard;
    }
    return shade(i);
}

struct LineIn {
    @location(0) pos: vec3<f32>,
    @location(1) color: vec4<f32>,
};

struct LineOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) world: vec3<f32>,
};

@vertex
fn vs_line(v: LineIn) -> LineOut {
    var o: LineOut;
    o.clip = u.view_proj * vec4<f32>(v.pos, 1.0);
    o.color = v.color;
    o.world = v.pos;
    return o;
}

@fragment
fn fs_line(i: LineOut) -> @location(0) vec4<f32> {
    if (clipped(i.world)) {
        discard;
    }
    return i.color;
}

// The grid is never clipped.
@fragment
fn fs_grid(i: LineOut) -> @location(0) vec4<f32> {
    return i.color;
}
