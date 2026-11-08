struct Globals {
    size: vec2<f32>,   // target size, physical px
    srgb: f32,         // 1.0 if the target format is *Srgb (needs linear output)
    _pad: f32,
};
@group(0) @binding(0) var<uniform> g: Globals;

fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    let lo = c / 12.92;
    let hi = pow((c + vec3<f32>(0.055)) / 1.055, vec3<f32>(2.4));
    return select(hi, lo, c <= vec3<f32>(0.04045));
}

// Input colors are sRGB-encoded, straight alpha. Output: premultiplied, in the
// target's encoding.
fn prep(c: vec4<f32>) -> vec4<f32> {
    var rgb = c.rgb;
    if (g.srgb > 0.5) {
        rgb = srgb_to_linear(rgb);
    }
    return vec4<f32>(rgb * c.a, c.a);
}

fn to_ndc(p: vec2<f32>) -> vec4<f32> {
    return vec4<f32>(p.x / g.size.x * 2.0 - 1.0, 1.0 - p.y / g.size.y * 2.0, 0.0, 1.0);
}

// ---- rounded rectangles (instanced) ----

struct RectIn {
    @location(0) rect: vec4<f32>,
    @location(1) color: vec4<f32>,
    @location(2) border_color: vec4<f32>,
    @location(3) params: vec4<f32>,
};

struct RectOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) local: vec2<f32>,
    @location(1) half: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) border_color: vec4<f32>,
    @location(4) params: vec4<f32>,
};

@vertex
fn vs_rect(@builtin(vertex_index) vi: u32, inst: RectIn) -> RectOut {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, -1.0), vec2<f32>(-1.0, 1.0),
        vec2<f32>(-1.0, 1.0), vec2<f32>(1.0, -1.0), vec2<f32>(1.0, 1.0),
    );
    let half = inst.rect.zw * 0.5;
    let center = inst.rect.xy + half;
    // 1px of padding so the anti-aliased edge is not cut off.
    let local = corners[vi] * (half + vec2<f32>(1.0));
    var out: RectOut;
    out.pos = to_ndc(center + local);
    out.local = local;
    out.half = half;
    out.color = prep(inst.color);
    out.border_color = prep(inst.border_color);
    out.params = inst.params;
    return out;
}

fn sd_round_box(p: vec2<f32>, half: vec2<f32>, r: f32) -> f32 {
    let q = abs(p) - half + vec2<f32>(r);
    return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - r;
}

@fragment
fn fs_rect(in: RectOut) -> @location(0) vec4<f32> {
    let r = min(in.params.x, min(in.half.x, in.half.y));
    let bw = in.params.y;
    let d = sd_round_box(in.local, in.half, r);
    let outer = clamp(0.5 - d, 0.0, 1.0);
    var col = in.color;
    if (bw > 0.0) {
        let inner = clamp(0.5 - (d + bw), 0.0, 1.0);
        col = mix(in.border_color, in.color, inner);
    }
    return col * outer;
}

// ---- curves / lines (triangle list) ----

struct CurveIn {
    @location(0) pos: vec2<f32>,
    @location(1) color: vec4<f32>,
};

struct CurveOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_curve(v: CurveIn) -> CurveOut {
    var out: CurveOut;
    out.pos = to_ndc(v.pos);
    out.color = prep(v.color);
    return out;
}

@fragment
fn fs_curve(in: CurveOut) -> @location(0) vec4<f32> {
    return in.color;
}
