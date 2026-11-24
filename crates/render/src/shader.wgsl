struct Globals {
    view_proj: mat4x4<f32>,
    eye: vec4<f32>,
    light: vec4<f32>,
};
struct Material {
    color: vec4<f32>,
    shadow: vec4<f32>,
    // cutoff, mode (0 opaque, 1 cutout, 2 transparent), outline width (m), unused
    params: vec4<f32>,
    outline: vec4<f32>,
};
@group(0) @binding(0) var<uniform> g: Globals;
@group(1) @binding(0) var tex: texture_2d<f32>;
@group(1) @binding(1) var samp: sampler;
@group(1) @binding(2) var<uniform> m: Material;

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
};

@vertex
fn vs(@location(0) pos: vec3<f32>, @location(1) normal: vec3<f32>, @location(2) uv: vec2<f32>) -> VsOut {
    var o: VsOut;
    o.clip = g.view_proj * vec4<f32>(pos, 1.0);
    o.world = pos;
    o.normal = normal;
    o.uv = uv;
    return o;
}

@fragment
fn fs(i: VsOut, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
    let base = textureSample(tex, samp, i.uv) * m.color;
    let mode = m.params.y;
    if (mode > 0.5 && mode < 1.5 && base.a < m.params.x) { discard; }
    if (mode > 1.5 && base.a < 0.002) { discard; }
    var n = normalize(i.normal);
    if (!front) { n = -n; }
    let l = normalize(g.light.xyz);
    let v = normalize(g.eye.xyz - i.world);
    // toon-ish: half-lambert through a soft step, tinted by the material's shadow color
    let hl = dot(n, l) * 0.5 + 0.5;
    let lit = smoothstep(0.45, 0.55, hl);
    let ambient = mix(vec3<f32>(0.92, 0.92, 0.96), vec3<f32>(1.0), n.y * 0.5 + 0.5);
    let rim = pow(1.0 - max(dot(n, v), 0.0), 4.0) * 0.12;
    let color = base.rgb * mix(m.shadow.rgb, vec3<f32>(1.0), lit) * ambient + rim;
    var alpha = base.a;
    if (mode < 1.5) { alpha = 1.0; }
    return vec4<f32>(color, alpha);
}

@vertex
fn vs_outline(@location(0) pos: vec3<f32>, @location(1) normal: vec3<f32>, @location(2) uv: vec2<f32>) -> VsOut {
    var o: VsOut;
    let world = pos + normalize(normal) * m.params.z;
    o.clip = g.view_proj * vec4<f32>(world, 1.0);
    o.world = world;
    o.normal = normal;
    o.uv = uv;
    return o;
}

@fragment
fn fs_outline(i: VsOut) -> @location(0) vec4<f32> {
    let base = textureSample(tex, samp, i.uv) * m.color;
    let mode = m.params.y;
    if (mode > 0.5 && mode < 1.5 && base.a < m.params.x) { discard; }
    return vec4<f32>(m.outline.rgb, 1.0);
}
