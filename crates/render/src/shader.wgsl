struct Globals {
    view_proj: mat4x4<f32>,
    eye: vec4<f32>,
    light: vec4<f32>,
};
@group(0) @binding(0) var<uniform> g: Globals;
@group(1) @binding(0) var tex: texture_2d<f32>;
@group(1) @binding(1) var samp: sampler;

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
    let base = textureSample(tex, samp, i.uv);
    if (base.a < 0.02) { discard; }
    var n = normalize(i.normal);
    if (!front) { n = -n; }
    let l = normalize(g.light.xyz);
    let v = normalize(g.eye.xyz - i.world);
    // toon-ish: half-lambert through a soft step, tinted shadow (lilToon-like defaults)
    let hl = dot(n, l) * 0.5 + 0.5;
    let lit = smoothstep(0.45, 0.55, hl);
    let shadow_tint = vec3<f32>(0.82, 0.74, 0.80);
    let ambient = mix(vec3<f32>(0.92, 0.92, 0.96), vec3<f32>(1.0), n.y * 0.5 + 0.5);
    let rim = pow(1.0 - max(dot(n, v), 0.0), 4.0) * 0.12;
    let color = base.rgb * mix(shadow_tint, vec3<f32>(1.0), lit) * ambient + rim;
    return vec4<f32>(color, base.a);
}
