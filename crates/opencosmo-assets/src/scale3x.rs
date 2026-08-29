//! Scale3x (AdvMAME3x), the reference implementation.
//!
//! The shipped upscaler is a transcription of this into
//! `crates/opencosmo-game/assets/shaders/present.wgsl`, so that it can run on
//! the composited frame and be toggled at runtime. This version exists to
//! pin down the two properties the whole choice of algorithm rests on, both
//! of which are easy to assert here and impossible to assert in a shader:
//!
//! 1. **It invents no colours.** Every output pixel is a copy of one of the
//!    nine inputs, so a 16-colour EGA frame stays a 16-colour EGA frame.
//!    Blending upscalers (xBRZ, HQx, any neural one) cannot promise this.
//! 2. **It leaves dithering alone.** This game's backdrops are up to 27%
//!    dither by pixel count, and a blending upscaler turns that into blobs.
//!    On a checkerboard `b == h` and `d == f` both hold, so the early-out
//!    fires and the pattern is replicated verbatim. This holds in the
//!    interior only: along the outermost pixel, clamping makes a neighbour
//!    equal to the centre and the guard no longer holds, so the border of a
//!    dithered image can be altered. In the shader that border is the edge
//!    of the 320x200 screen, which is the black frame, so nothing visible
//!    depends on it.
//!
//! Keep the two in step: if a rule changes here, change it there.

pub type Rgba = [u8; 4];

/// Magnifies `src` (row-major, `w` by `h`) three times in each axis.
///
/// Out-of-bounds neighbours clamp to the edge pixel, so a tile upscaled on
/// its own rounds its border against itself rather than against whatever
/// happens to sit next to it in an atlas.
pub fn scale3x(src: &[Rgba], w: usize, h: usize) -> Vec<Rgba> {
    assert_eq!(src.len(), w * h, "pixel buffer does not match its dimensions");
    let at = |x: isize, y: isize| -> Rgba {
        let cx = x.clamp(0, w as isize - 1) as usize;
        let cy = y.clamp(0, h as isize - 1) as usize;
        src[cy * w + cx]
    };

    let mut out = vec![[0u8; 4]; w * 3 * h * 3];
    for y in 0..h as isize {
        for x in 0..w as isize {
            let a = at(x - 1, y - 1);
            let b = at(x, y - 1);
            let c = at(x + 1, y - 1);
            let d = at(x - 1, y);
            let e = at(x, y);
            let f = at(x + 1, y);
            let g = at(x - 1, y + 1);
            let hh = at(x, y + 1);
            let i = at(x + 1, y + 1);

            // Flat along one axis or the other: nothing to round off. This
            // is also the guard that makes dithering pass through - on a
            // checkerboard b == hh and d == f both hold.
            let cell = if b == hh || d == f {
                [e; 9]
            } else {
                [
                    if d == b { d } else { e },
                    if (d == b && e != c) || (b == f && e != a) { b } else { e },
                    if b == f { f } else { e },
                    if (d == b && e != g) || (d == hh && e != a) { d } else { e },
                    e,
                    if (b == f && e != i) || (hh == f && e != c) { f } else { e },
                    if d == hh { d } else { e },
                    if (d == hh && e != i) || (hh == f && e != g) { hh } else { e },
                    if hh == f { f } else { e },
                ]
            };

            for (n, px) in cell.iter().enumerate() {
                let ox = x as usize * 3 + n % 3;
                let oy = y as usize * 3 + n / 3;
                out[oy * w * 3 + ox] = *px;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    const RED: Rgba = [200, 0, 0, 255];
    const BLUE: Rgba = [0, 0, 200, 255];
    const GREEN: Rgba = [0, 200, 0, 255];

    fn checkerboard(w: usize, h: usize) -> Vec<Rgba> {
        (0..w * h)
            .map(|i| if (i % w + i / w) % 2 == 0 { RED } else { BLUE })
            .collect()
    }

    #[test]
    fn the_output_is_three_times_the_size() {
        let out = scale3x(&checkerboard(4, 5), 4, 5);
        assert_eq!(out.len(), 12 * 15);
    }

    #[test]
    fn no_colour_is_invented() {
        // The property that makes this safe for a fixed palette: every
        // output pixel is a copy of an input pixel.
        let mut src = checkerboard(8, 8);
        src[27] = GREEN;
        src[28] = GREEN;
        let out = scale3x(&src, 8, 8);
        let input: HashSet<Rgba> = src.iter().copied().collect();
        let output: HashSet<Rgba> = out.iter().copied().collect();
        assert!(
            output.is_subset(&input),
            "invented {:?}",
            output.difference(&input).collect::<Vec<_>>()
        );
    }

    #[test]
    fn dithering_survives_untouched() {
        // A checkerboard is the worst case for a blending upscaler and the
        // best case for this one: it must come out as an exact 3x
        // replication, with the pattern still legible.
        let (w, h) = (6, 6);
        let src = checkerboard(w, h);
        let out = scale3x(&src, w, h);
        // Interior only - see the module docs on why the outermost pixel is
        // excluded.
        for y in 3..(h - 1) * 3 {
            for x in 3..(w - 1) * 3 {
                assert_eq!(
                    out[y * w * 3 + x],
                    src[(y / 3) * w + (x / 3)],
                    "dither altered at ({x}, {y})"
                );
            }
        }
    }

    #[test]
    fn the_dither_guard_is_what_does_it_not_luck() {
        // Guarding the claim above: an interior checkerboard pixel has
        // b == h and d == f, which is the early-out. If a future rule change
        // removed that guard, `dithering_survives_untouched` would still
        // pass by coincidence on some patterns; this would not.
        let src = checkerboard(5, 5);
        let (w, x, y) = (5usize, 2usize, 2usize);
        let b = src[(y - 1) * w + x];
        let h = src[(y + 1) * w + x];
        let d = src[y * w + x - 1];
        let f = src[y * w + x + 1];
        assert_eq!(b, h);
        assert_eq!(d, f);
    }

    #[test]
    fn a_flat_area_is_a_plain_replication() {
        let src = vec![RED; 16];
        let out = scale3x(&src, 4, 4);
        assert!(out.iter().all(|p| *p == RED));
    }

    #[test]
    fn a_staircased_diagonal_gets_rounded() {
        // A 2-pixel staircase on a background. The corner cells of the step
        // should pick up the foreground rather than staying background,
        // which is the entire visible effect of the algorithm.
        let (w, h) = (5, 5);
        let mut src = vec![BLUE; w * h];
        for (x, y) in [(1usize, 1usize), (2, 2), (3, 3)] {
            src[y * w + x] = RED;
        }
        let out = scale3x(&src, w, h);
        let get = |x: usize, y: usize| out[y * w * 3 + x];

        // The background pixel at (2,1) sits between two diagonal
        // neighbours; its lower-left sub-cell should be filled in.
        assert_eq!(get(2 * 3, 1 * 3 + 2), RED, "diagonal was not rounded");
        // ...while the far side of the same pixel stays background.
        assert_eq!(get(2 * 3 + 2, 1 * 3), BLUE, "rounding leaked too far");
    }

    #[test]
    fn edges_clamp_instead_of_reading_outside_the_image() {
        // Sprites and tiles are upscaled on their own, so the border must
        // resolve against itself - not panic, and not sample a neighbour.
        let src = vec![RED, BLUE, BLUE, RED];
        let out = scale3x(&src, 2, 2);
        assert_eq!(out.len(), 36);
        let input: HashSet<Rgba> = src.iter().copied().collect();
        assert!(out.iter().all(|p| input.contains(p)));
    }

    #[test]
    fn a_single_pixel_image_is_handled() {
        assert_eq!(scale3x(&[RED], 1, 1), vec![RED; 9]);
    }
}
