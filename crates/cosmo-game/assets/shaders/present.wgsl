// Presents the 320x200 virtual screen to the window.
//
// Everything the game draws lands in one offscreen texture at the original's
// exact resolution; this shader is the only thing that ever scales it. That
// separation is the whole point - scaling once, at the end, is what makes
// every source pixel the same size on screen.

#import bevy_sprite::mesh2d_vertex_output::VertexOutput

struct PresentSettings {
    // Size of the virtual screen in texels (320x200).
    source_size: vec2<f32>,
    // Size of the quad on screen, in physical pixels.
    output_size: vec2<f32>,
    // 0 = plain nearest (integer-scaled "authentic"), 1 = sharp bilinear.
    sharp: f32,
    scanline: f32,
    bloom: f32,
    vignette: f32,
    curvature: f32,
    _pad: f32,
}

@group(2) @binding(0) var<uniform> settings: PresentSettings;
@group(2) @binding(1) var screen_texture: texture_2d<f32>;
@group(2) @binding(2) var screen_sampler: sampler;

// "Sharp bilinear": nearest-neighbour within a texel, with the transition
// between texels compressed into a single *output* pixel.
//
// Plain nearest is only correct when the scale factor is a whole number.
// At a fractional scale it has to round, so some source pixels get one more
// output row/column than their neighbours - the blockiness reads as uneven
// rather than crisp. Plain bilinear fixes the unevenness by making
// everything soft. This keeps flat interiors perfectly sharp and only
// blends across the boundary, so a fractional scale looks uniform without
// going blurry.
fn sharp_uv(uv: vec2<f32>) -> vec2<f32> {
    let scale = settings.output_size / settings.source_size;
    let texel = uv * settings.source_size;
    let base = floor(texel);
    let dist = fract(texel) - 0.5;

    // How far from the texel centre we can travel before the ramp starts.
    // Everything inside this band samples the centre exactly, so a linear
    // sampler returns that texel untouched; only the remaining sliver -
    // half an output pixel on each side - interpolates.
    let region = 0.5 - 0.5 / scale;
    let ramp = (dist - clamp(dist, -region, region)) * scale + 0.5;

    return (base + ramp) / settings.source_size;
}

// Plain nearest, for integer-scaled authentic mode. The sampler is linear
// (sharp mode needs it to be), so nearest has to be expressed by snapping
// the coordinate to the texel centre rather than by the sampler.
fn nearest_uv(uv: vec2<f32>) -> vec2<f32> {
    return (floor(uv * settings.source_size) + 0.5) / settings.source_size;
}

// Very slight barrel distortion. Kept subtle on purpose: enough to suggest
// glass, not enough to bend the status bar's straight edges visibly.
fn curve(uv: vec2<f32>) -> vec2<f32> {
    if settings.curvature <= 0.0 {
        return uv;
    }
    let centred = uv * 2.0 - 1.0;
    let r2 = dot(centred, centred);
    return (centred * (1.0 + settings.curvature * r2)) * 0.5 + 0.5;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let uv = curve(in.uv);
    // Past the edge of the tube there is nothing to show.
    if uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0 {
        return vec4(0.0, 0.0, 0.0, 1.0);
    }

    var sample_uv = nearest_uv(uv);
    if settings.sharp > 0.5 {
        sample_uv = sharp_uv(uv);
    }
    var colour = textureSample(screen_texture, screen_sampler, sample_uv).rgb;

    // Bloom: EGA's saturated primaries are what glowed on a real monitor,
    // so only genuinely bright neighbours contribute. An unthresholded
    // version bleeds every edge equally, which doesn't read as glow - it
    // reads as being out of focus.
    if settings.bloom > 0.0 {
        let step = 1.0 / settings.source_size;
        var blur = textureSample(screen_texture, screen_sampler, uv + vec2(step.x, 0.0)).rgb;
        blur += textureSample(screen_texture, screen_sampler, uv - vec2(step.x, 0.0)).rgb;
        blur += textureSample(screen_texture, screen_sampler, uv + vec2(0.0, step.y)).rgb;
        blur += textureSample(screen_texture, screen_sampler, uv - vec2(0.0, step.y)).rgb;
        blur *= 0.25;
        let luma = dot(blur, vec3(0.299, 0.587, 0.114));
        // Nothing below half brightness glows at all, and the response
        // ramps from there rather than switching on.
        let excess = smoothstep(0.5, 1.0, luma);
        colour += blur * excess * settings.bloom;
    }

    // Scanlines, one dark band per *source* row. Brightness is compensated
    // so the picture doesn't simply get dimmer as the effect strengthens.
    if settings.scanline > 0.0 {
        let row = fract(uv.y * settings.source_size.y);
        let band = 1.0 - settings.scanline * pow(sin(row * 3.14159265), 2.0);
        colour *= band / (1.0 - settings.scanline * 0.5);
    }

    if settings.vignette > 0.0 {
        let centred = uv * 2.0 - 1.0;
        let falloff = 1.0 - settings.vignette * dot(centred, centred) * 0.25;
        colour *= falloff;
    }

    return vec4(clamp(colour, vec3(0.0), vec3(1.0)), 1.0);
}
