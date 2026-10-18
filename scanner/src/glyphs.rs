//! The digits of the game's count badges, drawn as strokes in the style of its font (a geometric sans: round bowls,
//! a flagged and footed 1, an open 4). The reader tells a badge's number by comparing its captured digits with them,
//! and the plugin draws the number with them at the key's size, cleanly, instead of blowing the capture up.

/// A digit's design box, in glyph units: 10 wide, 16 tall, y downwards. Strokes run through these points.
pub const BOX: (f32, f32) = (10.0, 16.0);
const STROKE: f32 = 2.2; // stroke width, in glyph units
const GAP: f32 = 1.6; // between two digits of a number, in glyph units

// a captured digit must look like one glyph clearly more than any other to be read as it
const READ_MIN: f32 = 0.62;
const READ_MARGIN: f32 = 0.08;

type Stroke = Vec<(f32, f32)>;

/// A polyline through `points` whose corners are rounded off, each by its radius in `radii` (one per corner, the
/// first and last point excepted unless `closed`; a radius of 0 keeps the corner sharp).
fn path(points: &[(f32, f32)], radii: &[f32], closed: bool) -> Stroke {
    let n = points.len();
    let corner = |i: usize| -> Option<(usize, usize)> {
        if closed {
            Some(((i + n - 1) % n, (i + 1) % n))
        } else if i > 0 && i + 1 < n {
            Some((i - 1, i + 1))
        } else {
            None
        }
    };
    let mut out = Vec::new();
    for i in 0..n {
        let p = points[i];
        let r = if closed { radii[i % radii.len()] } else { radii.get(i.wrapping_sub(1)).copied().unwrap_or(0.0) };
        match corner(i) {
            Some((a, b)) if r > 0.0 => {
                let toward = |q: (f32, f32)| {
                    let (dx, dy) = (q.0 - p.0, q.1 - p.1);
                    let len = dx.hypot(dy);
                    let t = r.min(len / 2.0) / len;
                    (p.0 + dx * t, p.1 + dy * t)
                };
                let (pa, pb) = (toward(points[a]), toward(points[b]));
                out.extend((0..=8).map(|k| {
                    let t = k as f32 / 8.0;
                    let u = 1.0 - t;
                    (u * u * pa.0 + 2.0 * u * t * p.0 + t * t * pb.0, u * u * pa.1 + 2.0 * u * t * p.1 + t * t * pb.1)
                }));
            }
            _ => out.push(p),
        }
    }
    if closed {
        out.push(out[0]);
    }
    out
}

/// A rectangle with rounded corners, as a closed stroke.
fn rounded(x0: f32, y0: f32, x1: f32, y1: f32, r: f32) -> Stroke {
    path(&[(x0, y0), (x1, y0), (x1, y1), (x0, y1)], &[r], true)
}

/// The strokes of digit `d` in its design box: square shapes with rounded corners, as in the game's font.
pub fn strokes(d: u8) -> Vec<Stroke> {
    let turned = |s: Vec<Stroke>| s.into_iter().map(|s| s.into_iter().map(|(x, y)| (BOX.0 - x, BOX.1 - y)).collect()).collect();
    let nine = || vec![rounded(1.3, 1.3, 8.7, 8.1, 2.8), path(&[(8.7, 4.5), (8.7, 14.6), (1.6, 14.6), (1.6, 12.8)], &[3.0, 0.8], false)];
    match d {
        0 => vec![rounded(1.3, 1.3, 8.7, 14.7, 3.0)],
        1 => vec![vec![(1.4, 1.4), (5.4, 1.4), (5.4, 14.6)], vec![(1.3, 14.6), (8.7, 14.6)]],
        2 => vec![path(
            &[(1.3, 3.4), (1.3, 1.4), (8.7, 1.4), (8.7, 6.4), (1.3, 11.4), (1.3, 14.4), (8.7, 14.4)],
            &[0.8, 2.8, 1.5, 1.5, 0.0],
            false,
        )],
        3 => vec![
            path(&[(1.6, 3.2), (1.6, 1.4), (8.7, 1.4), (8.7, 7.7), (4.4, 7.7)], &[0.8, 2.6, 2.0], false),
            path(&[(8.7, 7.7), (8.7, 14.6), (1.3, 14.6), (1.3, 12.6)], &[2.8, 0.8], false),
        ],
        4 => vec![vec![(5.6, 1.3), (1.3, 10.9), (9.0, 10.9)], vec![(7.4, 5.4), (7.4, 14.7)]],
        5 => vec![path(
            &[(8.7, 1.4), (1.6, 1.4), (1.6, 7.4), (8.7, 7.4), (8.7, 14.6), (1.3, 14.6), (1.3, 12.6)],
            &[0.0, 0.0, 2.6, 2.8, 0.8],
            false,
        )],
        6 => turned(nine()),
        7 => vec![path(&[(1.3, 1.4), (8.7, 1.4), (3.4, 14.6)], &[0.8], false)],
        8 => vec![rounded(1.8, 1.3, 8.2, 7.7, 2.6), rounded(1.3, 7.7, 8.7, 14.7, 2.8)],
        _ => nine(),
    }
}

/// The left and right edge of a digit's ink, in glyph units.
fn span(d: u8) -> (f32, f32) {
    let xs = strokes(d).into_iter().flatten().map(|p| p.0);
    let (lo, hi) = xs.fold((f32::MAX, f32::MIN), |(lo, hi), x| (lo.min(x), hi.max(x)));
    (lo - STROKE / 2.0, hi + STROKE / 2.0)
}

fn segment_distance(p: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len2 = dx * dx + dy * dy;
    let t = if len2 > 0.0 { (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / len2).clamp(0.0, 1.0) } else { 0.0 };
    (p.0 - a.0 - t * dx).hypot(p.1 - a.1 - t * dy)
}

/// Whether the point (glyph units) is on one of the strokes.
fn inked(strokes: &[Stroke], p: (f32, f32)) -> bool {
    strokes.iter().any(|s| s.windows(2).any(|w| segment_distance(p, w[0], w[1]) <= STROKE / 2.0))
}

/// The ink of `number` laid out in a `w` x `h` pixel box, digits side by side as tall as the box, each pixel's share
/// of ink found by sampling it 4x4 times: 0 (none) to 1 (full), row by row.
pub fn render(number: &str, w: usize, h: usize) -> Vec<f32> {
    let digits: Vec<u8> = number.bytes().filter(u8::is_ascii_digit).map(|b| b - b'0').collect();
    let mut out = vec![0.0f32; w * h];
    if digits.is_empty() || w == 0 || h == 0 {
        return out;
    }
    // each digit's inked width side by side, the gap between them; scaled so the number is as tall as the box
    let spans: Vec<(f32, f32)> = digits.iter().map(|&d| span(d)).collect();
    let total: f32 = spans.iter().map(|(lo, hi)| hi - lo).sum::<f32>() + GAP * (digits.len() - 1) as f32;
    let scale = h as f32 / BOX.1;
    let left = (w as f32 / scale - total) / 2.0; // centred
    let glyphs: Vec<(f32, Vec<Stroke>)> = {
        let mut x = left;
        digits
            .iter()
            .zip(&spans)
            .map(|(&d, &(lo, hi))| {
                let at = x - lo;
                x += hi - lo + GAP;
                (at, strokes(d))
            })
            .collect()
    };
    for py in 0..h {
        for px in 0..w {
            let mut ink = 0u32;
            for (i, j) in (0..4).flat_map(|i| (0..4).map(move |j| (i, j))) {
                let (x, y) = ((px as f32 + (i as f32 + 0.5) / 4.0) / scale, (py as f32 + (j as f32 + 0.5) / 4.0) / scale);
                if glyphs.iter().any(|(at, s)| inked(s, (x - at, y))) {
                    ink += 1;
                }
            }
            out[py * w + px] = ink as f32 / 16.0;
        }
    }
    out
}

/// The number a badge shows, from its captured digits (`bits`, `w` x `h`, true where ink, trimmed to the digits):
/// each run of inked columns is one digit, read as the glyph it matches best when it matches it clearly. None when a
/// digit is unclear, so that the capture is shown rather than a wrong number.
pub fn read(bits: &[bool], w: usize, h: usize) -> Option<String> {
    if bits.len() != w * h || w == 0 || h == 0 {
        return None;
    }
    let column = |x: usize| (0..h).any(|y| bits[y * w + x]);
    let mut runs: Vec<(usize, usize)> = Vec::new();
    for x in 0..w {
        match (column(x), runs.last_mut()) {
            (true, Some(r)) if r.1 + 1 == x => r.1 = x,
            (true, _) => runs.push((x, x)),
            _ => {}
        }
    }
    let mut number = String::new();
    for (x0, x1) in runs {
        let cw = x1 - x0 + 1;
        let ink: Vec<f32> = (0..h).flat_map(|y| (x0..=x1).map(move |x| (y, x))).map(|(y, x)| bits[y * w + x] as u8 as f32).collect();
        let mut scores: Vec<(f32, u8)> = (0..10).map(|d| (likeness(&ink, &render_digit(d, cw, h)), d)).collect();
        scores.sort_by(|a, b| b.0.total_cmp(&a.0));
        let (best, second) = (scores[0], scores[1].0);
        if best.0 < READ_MIN || best.0 - second < READ_MARGIN {
            return None;
        }
        number.push((b'0' + best.1) as char);
    }
    (!number.is_empty() && number.len() <= 3).then_some(number)
}

/// One digit drawn to fill a `w` x `h` box (its ink stretched to the box's width), as the reader compares it.
fn render_digit(d: u8, w: usize, h: usize) -> Vec<f32> {
    let (lo, hi) = span(d);
    let (sx, sy) = (w as f32 / (hi - lo), h as f32 / BOX.1);
    let s = strokes(d);
    let mut out = vec![0.0f32; w * h];
    for py in 0..h {
        for px in 0..w {
            let mut ink = 0u32;
            for (i, j) in (0..4).flat_map(|i| (0..4).map(move |j| (i, j))) {
                let p = ((px as f32 + (i as f32 + 0.5) / 4.0) / sx + lo, (py as f32 + (j as f32 + 0.5) / 4.0) / sy);
                ink += inked(&s, p) as u32;
            }
            out[py * w + px] = ink as f32 / 16.0;
        }
    }
    out
}

/// Normalised correlation of two pictures of the same size, -1 to 1 (0 when either is flat).
fn likeness(a: &[f32], b: &[f32]) -> f32 {
    let n = a.len() as f32;
    let (ma, mb) = (a.iter().sum::<f32>() / n, b.iter().sum::<f32>() / n);
    let (mut ab, mut aa, mut bb) = (0.0, 0.0, 0.0);
    for (x, y) in a.iter().zip(b) {
        let (x, y) = (x - ma, y - mb);
        ab += x * y;
        aa += x * x;
        bb += y * y;
    }
    if aa < 1e-6 || bb < 1e-6 { 0.0 } else { ab / (aa * bb).sqrt() }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A digit as a capture would hold it: drawn small, then cut to ink or not.
    fn captured(number: &str, w: usize, h: usize) -> Vec<bool> {
        render(number, w, h).into_iter().map(|v| v >= 0.5).collect()
    }

    #[test]
    fn every_digit_reads_back_from_a_small_capture_of_itself() {
        for d in 0..10u8 {
            let n = d.to_string();
            let w = ((span(d).1 - span(d).0) * 15.0 / BOX.1).round() as usize;
            let bits = captured(&n, w, 15);
            // trimmed as the reader trims captures: no empty columns at the sides
            let cols: Vec<usize> = (0..w).filter(|&x| (0..15).any(|y| bits[y * w + x])).collect();
            let (x0, x1) = (cols[0], *cols.last().unwrap());
            let trimmed: Vec<bool> = (0..15).flat_map(|y| (x0..=x1).map(move |x| (y, x))).map(|(y, x)| bits[y * w + x]).collect();
            assert_eq!(read(&trimmed, x1 - x0 + 1, 15).as_deref(), Some(n.as_str()), "{d}");
        }
    }

    #[test]
    fn a_number_of_two_digits_is_read_digit_by_digit() {
        let (w, h) = (24, 15);
        let bits = captured("12", w, h);
        assert_eq!(read(&bits, w, h).as_deref(), Some("12"));
    }

    #[test]
    fn a_blot_reads_as_nothing() {
        assert_eq!(read(&[true; 8 * 15], 8, 15), None, "a full block is no digit");
        assert_eq!(read(&[false; 8 * 15], 8, 15), None, "no ink");
        assert_eq!(read(&[true; 3], 2, 2), None, "the wrong size");
    }

    #[test]
    fn render_keeps_the_number_inside_its_box_and_centred() {
        let (w, h) = (40, 32);
        let ink = render("5", w, h);
        assert!(ink.iter().any(|&v| v > 0.9), "solid strokes");
        let cols: Vec<usize> = (0..w).filter(|&x| (0..h).any(|y| ink[y * w + x] > 0.0)).collect();
        let (l, r) = (cols[0], w - 1 - cols.last().unwrap());
        assert!(l.abs_diff(r) <= 1, "centred: {l} left, {r} right");
    }
}
