//! Reads a menu row's status line: the arrow code when it can be called, otherwise a state word.
//! Templates are greyscale cuts from 2560x1440 frames (tools/make_status_templates.py).
//! The words are the English HUD text; other game languages need their own cuts.

use std::path::Path;

use crate::img::Img;

const BAND_Y: f64 = 34.0; // status line top, below the row's icon top
const WORD_X: f64 = 154.0;
const ARROW_X: f64 = 155.0;
const SLACK: isize = 3; // position tolerance in pixels at 1440p
const STATUS_MIN: f32 = 0.55;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Status {
    Ready,
    Cooldown,
    Unavailable,
    Active, // "Activating" / "Inbound" / "Impact": already called in and on its way
    Unknown,
}

const ARROW_STEP: f64 = 29.0; // distance between arrows of a code at 1440p
const ARROW_W: f64 = 26.0;
// arrows read when lookalike icons need the code to tell them apart: the Rover and K-9 backpacks
// only differ at the 6th, and the longest code (Aquifer Drill) has 8
pub const CODE_READ: usize = 8;

struct Template {
    status: Status,
    dir: Option<char>, // arrow templates: the direction letter
    x: f64,
    w: usize,
    h: usize,
    v: Vec<f32>, // zero-mean, unit norm
}

pub struct StatusReader {
    raw: Vec<(Status, Option<char>, f64, Img)>,
    scaled: Option<(usize, Vec<Template>)>, // keyed by icon size
}

/// Right edge (at 1440p) the capture must reach for the status words and the first CODE_READ arrows.
pub const NEEDS_X: f64 = ARROW_X + ARROW_STEP * (CODE_READ - 1) as f64 + ARROW_W + SLACK as f64 + 1.0;

fn gray(img: &Img) -> Vec<f32> {
    img.px.as_chunks::<3>().0.iter().map(|p| 0.299 * p[0] + 0.587 * p[1] + 0.114 * p[2]).collect()
}

impl StatusReader {
    pub fn load(dir: &Path) -> StatusReader {
        let table = [
            ("word_ready", Status::Ready, None, WORD_X),
            ("word_cooldown", Status::Cooldown, None, WORD_X),
            ("word_unavailable", Status::Unavailable, None, WORD_X),
            ("word_impact", Status::Active, None, WORD_X),
            ("word_inbound", Status::Active, None, WORD_X),
            ("word_activating", Status::Active, None, WORD_X),
            ("arrow_up", Status::Ready, Some('U'), ARROW_X),
            ("arrow_down", Status::Ready, Some('D'), ARROW_X),
            ("arrow_left", Status::Ready, Some('L'), ARROW_X),
            ("arrow_right", Status::Ready, Some('R'), ARROW_X),
        ];
        let raw = table
            .iter()
            .filter_map(|(name, status, d, x)| Img::load_png(&dir.join(format!("{name}.png"))).ok().map(|img| (*status, *d, *x, img)))
            .collect();
        StatusReader { raw, scaled: None }
    }

    fn prepare(&mut self, f: f64, size: usize) {
        if self.scaled.as_ref().is_some_and(|s| s.0 == size) {
            return;
        }
        let tpls = self
            .raw
            .iter()
            .map(|(status, dir, x, img)| {
                let (w, h) = (((img.w as f64) * f).round().max(4.0) as usize, ((img.h as f64) * f).round().max(4.0) as usize);
                let mut v = gray(&img.resize_area(w, h));
                let mean = v.iter().sum::<f32>() / v.len() as f32;
                v.iter_mut().for_each(|p| *p -= mean);
                let n = v.iter().map(|p| p * p).sum::<f32>().sqrt().max(1e-3);
                v.iter_mut().for_each(|p| *p /= n);
                Template { status: *status, dir: *dir, x: *x, w, h, v }
            })
            .collect();
        self.scaled = Some((size, tpls));
    }

    /// Status of the row whose icon top is at `top` (1440p units), in a capture starting at `origin`.
    pub fn read(&mut self, col: &Img, f: f64, size: usize, top: f64, origin: (usize, usize)) -> (Status, f32) {
        self.prepare(f, size);
        let Some((_, tpls)) = &self.scaled else { return (Status::Unknown, 0.0) };
        let mut best = (Status::Unknown, STATUS_MIN);
        for t in tpls {
            let c = best_at(col, f, t, t.x, top, origin);
            if c > best.1 {
                best = (t.status, c);
            }
        }
        best
    }

    /// The arrow code shown on the status line, as "UDLR" letters, up to `max` arrows; stops at the
    /// first position without a clear arrow (none at all when the row shows a word instead).
    pub fn read_arrows(&mut self, col: &Img, f: f64, size: usize, top: f64, origin: (usize, usize), max: usize) -> String {
        self.prepare(f, size);
        let Some((_, tpls)) = &self.scaled else { return String::new() };
        let mut code = String::new();
        for i in 0..max {
            let x = ARROW_X + ARROW_STEP * i as f64;
            let best =
                tpls.iter().filter_map(|t| t.dir.map(|d| (d, best_at(col, f, t, x, top, origin)))).max_by(|a, b| a.1.total_cmp(&b.1));
            match best {
                Some((d, c)) if c >= STATUS_MIN => code.push(d),
                _ => break,
            }
        }
        code
    }
}

/// Best normalised correlation of template `t` placed at `x` (1440p) on the status line, within the slack.
fn best_at(col: &Img, f: f64, t: &Template, x: f64, top: f64, origin: (usize, usize)) -> f32 {
    let slack = ((SLACK as f64) * f).round().max(1.0) as isize;
    let x = (x * f).round() as isize - origin.0 as isize - slack;
    let y = ((top + BAND_Y) * f).round() as isize - origin.1 as isize - slack;
    let win = col.crop(x, y, t.w + 2 * slack as usize, t.h + 2 * slack as usize);
    if win.w < t.w || win.h < t.h {
        return -1.0;
    }
    let g = gray(&win);
    let mut best = -1.0f32;
    for dy in 0..=win.h - t.h {
        for dx in 0..=win.w - t.w {
            let (mut s, mut ss, mut d) = (0.0f32, 0.0f32, 0.0f32);
            for r in 0..t.h {
                let row = &g[(dy + r) * win.w + dx..(dy + r) * win.w + dx + t.w];
                let tr = &t.v[r * t.w..(r + 1) * t.w];
                for (p, q) in row.iter().zip(tr) {
                    s += p;
                    ss += p * p;
                    d += p * q;
                }
            }
            let n = (t.w * t.h) as f32;
            let var = ss - s * s / n;
            if var > 1e-3 {
                best = best.max(d / var.sqrt());
            }
        }
    }
    best
}

#[cfg(test)]
mod tests {
    //! Status lines drawn from the templates themselves onto a flat background.

    use std::path::PathBuf;

    use super::*;

    const TOP: f64 = 200.0; // icon top of the test row, 1440p units

    fn dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../data/status")
    }

    fn template(name: &str) -> Img {
        Img::load_png(&dir().join(format!("{name}.png"))).unwrap()
    }

    /// A flat capture covering the status line of the row at TOP, for a game frame `f` times 1440p.
    fn line(f: f64) -> Img {
        let (w, h) = ((420.0 * f) as usize, ((TOP + 80.0) * f) as usize);
        let mut img = Img::new(w, h);
        img.px.iter_mut().for_each(|v| *v = 35.0);
        img
    }

    /// Draw a template scaled by `f` with its top-left at (x, TOP + BAND_Y + dy) in 1440p units.
    fn draw(img: &mut Img, name: &str, x: f64, dy: isize, f: f64) {
        let t = template(name);
        let t = t.resize_area(((t.w as f64) * f).round() as usize, ((t.h as f64) * f).round() as usize);
        let (x0, y0) = ((x * f).round() as usize, (((TOP + BAND_Y) * f).round() as isize + dy) as usize);
        for y in 0..t.h {
            for x in 0..t.w {
                let (s, d) = ((y * t.w + x) * 3, ((y0 + y) * img.w + x0 + x) * 3);
                img.px[d..d + 3].copy_from_slice(&t.px[s..s + 3]);
            }
        }
    }

    fn draw_code(img: &mut Img, code: &str, f: f64) {
        for (i, c) in code.chars().enumerate() {
            let name = match c {
                'U' => "arrow_up",
                'D' => "arrow_down",
                'L' => "arrow_left",
                _ => "arrow_right",
            };
            draw(img, name, ARROW_X + ARROW_STEP * i as f64, 0, f);
        }
    }

    fn size(f: f64) -> usize {
        (57.0 * f).round() as usize
    }

    #[test]
    fn capture_width_reaches_the_last_code_arrow() {
        assert_eq!(NEEDS_X, 388.0);
        assert!(NEEDS_X >= WORD_X + template("word_unavailable").w as f64 + SLACK as f64);
    }

    #[test]
    fn loads_all_ten_templates() {
        let r = StatusReader::load(&dir());
        assert_eq!(r.raw.len(), 10);
        assert_eq!(r.raw.iter().filter(|t| t.1.is_some()).count(), 4, "four arrows");
        assert_eq!(r.raw.iter().filter(|t| t.0 == Status::Active).count(), 3, "impact, inbound and activating");
    }

    #[test]
    fn missing_templates_read_unknown() {
        let mut r = StatusReader::load(Path::new("no/such/dir"));
        let mut img = line(1.0);
        draw(&mut img, "word_ready", WORD_X, 0, 1.0);
        assert_eq!(r.read(&img, 1.0, size(1.0), TOP, (0, 0)).0, Status::Unknown);
        assert_eq!(r.read_arrows(&img, 1.0, size(1.0), TOP, (0, 0), CODE_READ), "");
    }

    #[test]
    fn an_empty_line_reads_unknown() {
        let mut r = StatusReader::load(&dir());
        let img = line(1.0);
        assert_eq!(r.read(&img, 1.0, size(1.0), TOP, (0, 0)).0, Status::Unknown);
        assert_eq!(r.read_arrows(&img, 1.0, size(1.0), TOP, (0, 0), CODE_READ), "");
    }

    #[test]
    fn each_word_reads_as_its_status() {
        for (word, status) in [
            ("word_ready", Status::Ready),
            ("word_cooldown", Status::Cooldown),
            ("word_unavailable", Status::Unavailable),
            ("word_impact", Status::Active),
            ("word_inbound", Status::Active),
            ("word_activating", Status::Active),
        ] {
            for f in [1.0, 0.75, 1.5] {
                let mut r = StatusReader::load(&dir());
                let mut img = line(f);
                draw(&mut img, word, WORD_X, 0, f);
                let (got, score) = r.read(&img, f, size(f), TOP, (0, 0));
                assert_eq!(got, status, "{word} at scale {f} (score {score})");
            }
        }
    }

    #[test]
    fn a_word_off_by_the_slack_still_reads() {
        for dy in [-SLACK, SLACK] {
            let mut r = StatusReader::load(&dir());
            let mut img = line(1.0);
            draw(&mut img, "word_cooldown", WORD_X + SLACK as f64, dy, 1.0);
            assert_eq!(r.read(&img, 1.0, size(1.0), TOP, (0, 0)).0, Status::Cooldown, "dy {dy}");
        }
    }

    #[test]
    fn arrows_read_as_the_code_and_count_as_ready() {
        for code in ["UDRU", "DDUR", "LDRDRDU", "LLLUDRDD", "RRU"] {
            for f in [1.0, 0.75] {
                let mut r = StatusReader::load(&dir());
                let mut img = line(f);
                draw_code(&mut img, code, f);
                assert_eq!(r.read_arrows(&img, f, size(f), TOP, (0, 0), CODE_READ), code, "scale {f}");
                assert_eq!(r.read(&img, f, size(f), TOP, (0, 0)).0, Status::Ready, "{code} at scale {f}");
            }
        }
    }

    #[test]
    fn arrow_reading_stops_at_max_and_at_the_first_gap() {
        let mut r = StatusReader::load(&dir());
        let mut img = line(1.0);
        draw_code(&mut img, "LDRDRDU", 1.0);
        assert_eq!(r.read_arrows(&img, 1.0, size(1.0), TOP, (0, 0), 3), "LDR");
        assert_eq!(r.read_arrows(&img, 1.0, size(1.0), TOP, (0, 0), 0), "");
        let mut img = line(1.0);
        draw_code(&mut img, "UD", 1.0);
        draw(&mut img, "arrow_right", ARROW_X + ARROW_STEP * 3.0, 0, 1.0); // after an empty position
        assert_eq!(r.read_arrows(&img, 1.0, size(1.0), TOP, (0, 0), CODE_READ), "UD");
    }

    #[test]
    fn the_capture_origin_is_honoured() {
        let mut r = StatusReader::load(&dir());
        let mut full = line(1.0);
        draw_code(&mut full, "DLRD", 1.0);
        let origin = (56, 43);
        let col = full.crop(origin.0 as isize, origin.1 as isize, full.w - origin.0, full.h - origin.1);
        assert_eq!(r.read_arrows(&col, 1.0, size(1.0), TOP, origin, CODE_READ), "DLRD");
    }
}
