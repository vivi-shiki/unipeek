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
    // emission rgb, normal scale (0 without a normal map)
    emission: vec4<f32>,
    // MatCap tint rgb and weight
    matcap: vec4<f32>,
    // MatCap blend: 0 none, 1 mix, 2 add, 3 multiply
    extra: vec4<f32>,
};
@group(0) @binding(0) var<uniform> g: Globals;
@group(1) @binding(0) var tex: texture_2d<f32>;
@group(1) @binding(1) var samp: sampler;
@group(1) @binding(2) var<uniform> m: Material;
@group(1) @binding(3) var normal_tex: texture_2d<f32>;
@group(1) @binding(4) var emission_tex: texture_2d<f32>;
@group(1) @binding(5) var matcap_tex: texture_2d<f32>;

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) tangent: vec4<f32>,
};

@vertex
fn vs(@location(0) pos: vec3<f32>, @location(1) normal: vec3<f32>, @location(2) uv: vec2<f32>, @location(3) tangent: vec4<f32>) -> VsOut {
    var o: VsOut;
    o.clip = g.view_proj * vec4<f32>(pos, 1.0);
    o.world = pos;
    o.normal = normal;
    o.uv = uv;
    o.tangent = tangent;
    return o;
}

@fragment
fn fs(i: VsOut, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
    let base = textureSample(tex, samp, i.uv) * m.color;
    let mode = m.params.y;
    if (mode > 0.5 && mode < 1.5 && base.a < m.params.x) { discard; }
    if (mode > 1.5 && base.a < 0.002) { discard; }
    var n = normalize(i.normal);
    if (m.emission.w != 0.0) {
        // Tangent-space normal map in MikkTSpace (computed with the FBX's own, upward v).
        let t = normalize(i.tangent.xyz - n * dot(n, i.tangent.xyz));
        let b = cross(n, t) * i.tangent.w;
        var tn = textureSample(normal_tex, samp, i.uv).xyz * 2.0 - 1.0;
        tn = vec3<f32>(tn.xy * m.emission.w, tn.z);
        n = normalize(t * tn.x + b * tn.y + n * max(tn.z, 0.001));
    }
    if (!front) { n = -n; }
    let l = normalize(g.light.xyz);
    let v = normalize(g.eye.xyz - i.world);
    // toon-ish: half-lambert through a soft step, tinted by the material's shadow color
    let hl = dot(n, l) * 0.5 + 0.5;
    let lit = smoothstep(0.45, 0.55, hl);
    let ambient = mix(vec3<f32>(0.92, 0.92, 0.96), vec3<f32>(1.0), n.y * 0.5 + 0.5);
    let rim = pow(1.0 - max(dot(n, v), 0.0), 4.0) * 0.12;
    var color = base.rgb * mix(m.shadow.rgb, vec3<f32>(1.0), lit) * ambient + rim;
    let blend = m.extra.x;
    if (blend > 0.5) {
        // MatCap: the view-space normal picks a point on the sphere image.
        let right = normalize(cross(vec3<f32>(0.0, 1.0, 0.0), v));
        let up = cross(v, right);
        let muv = vec2<f32>(dot(n, right), -dot(n, up)) * 0.49 + 0.5;
        let mc = textureSample(matcap_tex, samp, muv).rgb * m.matcap.rgb;
        let w = m.matcap.a;
        if (blend < 1.5) {
            color = mix(color, mc, w);
        } else if (blend < 2.5) {
            color = color + mc * w;
        } else {
            color = color * mix(vec3<f32>(1.0), mc, w);
        }
    }
    color = color + textureSample(emission_tex, samp, i.uv).rgb * m.emission.rgb;
    var alpha = base.a;
    if (mode < 1.5) { alpha = 1.0; }
    return vec4<f32>(color, alpha);
}

@vertex
fn vs_outline(@location(0) pos: vec3<f32>, @location(1) normal: vec3<f32>, @location(2) uv: vec2<f32>, @location(3) tangent: vec4<f32>) -> VsOut {
    var o: VsOut;
    let world = pos + normalize(normal) * m.params.z;
    o.clip = g.view_proj * vec4<f32>(world, 1.0);
    o.world = world;
    o.normal = normal;
    o.uv = uv;
    o.tangent = tangent;
    return o;
}

@fragment
fn fs_outline(i: VsOut) -> @location(0) vec4<f32> {
    let base = textureSample(tex, samp, i.uv) * m.color;
    let mode = m.params.y;
    if (mode > 0.5 && mode < 1.5 && base.a < m.params.x) { discard; }
    return vec4<f32>(m.outline.rgb, 1.0);
}
