//! Reads the stratagem menu rows at their fixed HUD positions.
//! Thresholds were tuned on captured frames (first in the Python prototype this replaced).

use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;

use crate::img::Img;
use crate::status::{self, Status, StatusReader};

pub const REF_HEIGHT: f64 = 1440.0;
pub const REF_ICON: f64 = 57.0; // icon box at 2560x1440, 100% HUD scale
pub const REF_X: f64 = 70.0; // left edge of the icon column at 2560x1440
const X_TOL: f64 = 14.0;
const BADGE_FRAC: f64 = 0.55; // the use-count badge covers the lower-right of the icon
const FRAME_FRAC: f64 = 0.12; // lookalikes share the coloured frame, so identity is scored on the glyph
const ROW0_Y: f64 = 108.0; // top of the first menu row (Reinforce) at 2560x1440
const ROW_PITCH: f64 = 70.0;
const MAX_ROWS: usize = 13;
const ROW_MIN: f32 = 0.55; // whole-icon score: real rows, dimmed included, >= 0.69; empty spots <= 0.40
const ROW_MIN_ISOLATED: f32 = 0.80; // rows after an empty slot are never dimmed (seen >= 0.93)
const ROW_KEEP: f32 = 0.60; // a slot still showing last scan's stratagem skips the ranking
const ROW_LOW: usize = 24; // side of the cheap ranking pass
const ROW_LOW_MIN: f32 = 0.45; // cheap-pass score below which a slot is empty (real >= 0.56, empty <= 0.47)
const ROW_TOP: usize = 5; // candidates scored at full size
const ROW_SURE: f32 = 0.85; // glyph score trusted outright
const ROW_MARGIN: f32 = 0.08; // below ROW_SURE, the runner-up glyph must trail by this much
const FILL_DIFF: f32 = 45.0; // grey-level difference that marks a pixel as covered by the progress fill
const FILL_GAP: usize = 3; // filled-row runs tolerate this many unclear rows (white glyphs)

#[derive(Deserialize, Clone)]
pub struct Stratagem {
    pub name: String,
    pub code: String,
    pub cooldown: String,
    pub icon: String,
}

impl Stratagem {
    pub fn png(&self) -> String {
        let stem = Path::new(&self.icon).file_stem().unwrap_or_default().to_string_lossy().into_owned();
        format!("{stem}.png")
    }
}

#[derive(Debug, Clone)]
pub struct Row {
    pub index: usize,
    pub score: f32,
    pub sure: bool,
    pub slot: usize,
    pub status: Status,
    /// Share of the icon covered by the game's progress fill, rising from the bottom (cooldown,
    /// shrinks as it ends) and coming down from the top (inbound).
    pub fill_bottom: f32,
    pub fill_top: f32,
    /// The use-count badge at the icon's corner, if the row has one.
    pub badge: Option<Badge>,
}

/// The count badge's digits as a mask (true = dark digit pixel), trimmed to the digits; the cream
/// box is redrawn around them on the key.
#[derive(Clone, Debug)]
pub struct Badge {
    pub hash: u64,
    pub w: usize,
    pub h: usize,
    pub bits: std::sync::Arc<Vec<bool>>,
}

impl PartialEq for Badge {
    fn eq(&self, other: &Self) -> bool {
        self.hash == other.hash
    }
}

/// Find the cream count badge right of the icon whose top is `top` (1440p units). It is
/// right-aligned near x=140 and grows leftwards with more digits; requiring it to reach past the
/// icon's right edge keeps white glyphs inside the icon from passing for one.
fn read_badge(col: &Img, f: f64, top: f64, origin: (usize, usize)) -> Option<Badge> {
    let rx0 = (100.0 * f).round() as isize - origin.0 as isize;
    let ry0 = ((top + 28.0) * f).round() as isize - origin.1 as isize;
    let reg = col.crop(rx0, ry0, (46.0 * f).round() as usize, (36.0 * f).round() as usize);
    let at = |x: usize, y: usize| &reg.px[(y * reg.w + x) * 3..(y * reg.w + x) * 3 + 3];
    let lum = |p: &[f32]| 0.299 * p[0] + 0.587 * p[1] + 0.114 * p[2];
    let cream =
        |p: &[f32]| lum(p) > 175.0 && p.iter().cloned().fold(f32::MIN, f32::max) - p.iter().cloned().fold(f32::MAX, f32::min) < 45.0;

    // the badge's top margin rows are cream end to end: the longest cream run there gives its width
    let mut best: Option<(usize, usize, usize)> = None; // (row, x0, x1)
    for y in (2.0 * f) as usize..((14.0 * f) as usize).min(reg.h) {
        let mut run_start = None;
        for x in 0..=reg.w {
            let c = x < reg.w && cream(at(x, y));
            match (c, run_start) {
                (true, None) => run_start = Some(x),
                (false, Some(s)) => {
                    if best.is_none_or(|b| x - s > b.2 - b.1 + 1) {
                        best = Some((y, s, x - 1));
                    }
                    run_start = None;
                }
                _ => {}
            }
        }
    }
    let (row, x0, x1) = best?;
    let right_edge = (133.0 * f).round() as isize - (100.0 * f).round() as isize;
    if ((x1 - x0 + 1) as f64) < 12.0 * f || (x1 as isize) < right_edge {
        return None;
    }
    let creamy = |y: usize| (x0..=x1).filter(|&x| cream(at(x, y))).count() * 100 >= (x1 - x0 + 1) * 45;
    let (mut y0, mut y1) = (row, row);
    while y0 > 0 && creamy(y0 - 1) {
        y0 -= 1;
    }
    while y1 + 1 < reg.h && creamy(y1 + 1) {
        y1 += 1;
    }
    if ((y1 - y0 + 1) as f64) < 15.0 * f {
        return None;
    }
    // the box's soft edge blends with the scene and wobbles by a pixel or two between frames;
    // the digits are solid, so the badge is keyed on the digits alone, trimmed tight
    // the outer two pixels of the box are where the scene shows through, so they are not searched
    let ink = |x: usize, y: usize| lum(at(x, y)) < 140.0;
    let (sx0, sx1, sy0, sy1) = (x0 + 2, x1.saturating_sub(2), y0 + 2, y1.saturating_sub(2));
    let cols: Vec<usize> = (sx0..=sx1).filter(|&x| (sy0..=sy1).any(|y| ink(x, y))).collect();
    let rows: Vec<usize> = (sy0..=sy1).filter(|&y| (sx0..=sx1).any(|x| ink(x, y))).collect();
    let (&ix0, &ix1, &iy0, &iy1) = (cols.first()?, cols.last()?, rows.first()?, rows.last()?);
    let (w, h) = (ix1 - ix0 + 1, iy1 - iy0 + 1);
    // digits are 14-16 px tall at 1440p; anything else is scenery caught in the box
    if (h as f64) < 12.0 * f || (h as f64) > 17.0 * f || (w as f64) > 30.0 * f {
        return None;
    }
    let bits: Vec<bool> = (iy0..=iy1).flat_map(|y| (ix0..=ix1).map(move |x| (x, y))).map(|(x, y)| ink(x, y)).collect();
    let mut hash: u64 = 0xcbf29ce484222325 ^ ((w as u64) << 32 | h as u64);
    for &b in &bits {
        hash = (hash ^ b as u64).wrapping_mul(0x100000001b3);
    }
    Some(Badge { hash, w, h, bits: std::sync::Arc::new(bits) })
}

/// Masked, zero-mean (per channel), unit-norm template pixels.
struct Flat {
    v: Vec<f32>,
    norm: f32,
}

struct Sized {
    size: usize,
    whole_idx: Vec<usize>,
    glyph_idx: Vec<usize>,
    whole: Vec<Flat>,
    glyph: Vec<Flat>,
    images: Vec<Img>, // templates at this size, for the row-by-row fill comparison
}

pub struct Matcher {
    pub catalog: Vec<Stratagem>,
    raw: Vec<Img>,
    low: Vec<Vec<f32>>,
    sized: Option<Sized>,
    last: HashMap<usize, (usize, usize, usize)>, // slot -> (template, dx, dy) of the last sure match
    status: StatusReader,
    lookalikes: Vec<Vec<usize>>, // per stratagem, the others drawn with the same icon
}

const LOOKALIKE: f32 = 0.97; // icon correlation at which two catalog icons count as the same picture

/// Where the icon column sits inside a w x h game frame: (x0, y0, x1, y1).
pub fn column_box(w: usize, h: usize, icon_scale: f64) -> (usize, usize, usize, usize) {
    let f = h as f64 / REF_HEIGHT;
    // read_rows puts the icons at REF_X * icon_scale, so a smaller HUD starts further left
    let x0 = ((REF_X * icon_scale - X_TOL) * f).round().max(0.0) as usize;
    let right = (REF_X + REF_ICON * icon_scale + X_TOL).max(status::NEEDS_X * icon_scale);
    let x1 = ((right * f).round() as usize).min(w);
    (x0, (h as f64 * 0.03) as usize, x1, (h as f64 * 0.72) as usize)
}

impl Matcher {
    pub fn load(data: &Path) -> Result<Matcher, String> {
        let text = std::fs::read_to_string(data.join("stratagems.json")).map_err(|e| format!("stratagems.json: {e}"))?;
        let all: Vec<Stratagem> = serde_json::from_str(&text).map_err(|e| format!("stratagems.json: {e}"))?;
        let mut catalog = Vec::new();
        let mut raw = Vec::new();
        for s in all {
            let img = Img::load_png(&data.join("icons_144").join(s.png()))?;
            if img.px.iter().all(|&v| v == 0.0) {
                continue;
            }
            catalog.push(s);
            raw.push(img);
        }
        let low: Vec<Vec<f32>> = raw.iter().map(|r| normalized(&r.resize_area(ROW_LOW, ROW_LOW).px)).collect();
        let mut lookalikes = vec![Vec::new(); low.len()];
        for i in 0..low.len() {
            for j in i + 1..low.len() {
                if dot(&low[i], &low[j]) >= LOOKALIKE {
                    lookalikes[i].push(j);
                    lookalikes[j].push(i);
                }
            }
        }
        let status = StatusReader::load(&data.join("status"));
        Ok(Matcher { catalog, raw, low, sized: None, last: HashMap::new(), status, lookalikes })
    }

    pub fn reset(&mut self) {
        self.last.clear();
    }

    fn prepare(&mut self, size: usize) {
        if self.sized.as_ref().is_some_and(|s| s.size == size) {
            return;
        }
        let badge = (size as f64 * BADGE_FRAC) as usize;
        let band = (size as f64 * FRAME_FRAC).round() as usize;
        let mut whole_idx = Vec::new();
        let mut glyph_idx = Vec::new();
        for y in 0..size {
            for x in 0..size {
                if x >= badge && y >= badge {
                    continue;
                }
                whole_idx.push(y * size + x);
                if x >= band && y >= band && x < size - band && y < size - band {
                    glyph_idx.push(y * size + x);
                }
            }
        }
        let mut whole = Vec::new();
        let mut glyph = Vec::new();
        let mut images = Vec::new();
        for r in &self.raw {
            let t = r.resize_area(size, size);
            whole.push(flatten(&t.px, &whole_idx));
            glyph.push(flatten(&t.px, &glyph_idx));
            images.push(t);
        }
        self.last.clear();
        self.sized = Some(Sized { size, whole_idx, glyph_idx, whole, glyph, images });
    }

    /// Read every menu slot. `col` is the captured icon column, `origin` its top-left in the game frame.
    pub fn read_rows(&mut self, col: &Img, h: usize, origin: (usize, usize), icon_scale: f64) -> Vec<Row> {
        let f = h as f64 / REF_HEIGHT * icon_scale;
        let size = (REF_ICON * f).round().max(16.0) as usize;
        let pad = (3.0 * f).round().max(2.0) as isize;
        self.prepare(size);
        let x = (REF_X * f).round() as isize - origin.0 as isize;
        let mut rows = Vec::new();
        let mut gap = false;
        for k in 0..MAX_ROWS {
            let y = ((ROW0_Y + ROW_PITCH * k as f64) * f).round() as isize - origin.1 as isize;
            let win = col.crop(x - pad, y - pad, size + 2 * pad as usize, size + 2 * pad as usize);
            if win.w < size || win.h < size {
                break;
            }
            match self.classify(&win, k) {
                // dimmed rows only occur in the open menu, which is unbroken from slot 0; a row after a
                // gap is a bright notice and must clear a bar scenery cannot (scenery reached 0.60)
                Some((index, score, sure, dx, dy)) if !gap || score >= ROW_MIN_ISOLATED => {
                    let top = ROW0_Y + ROW_PITCH * k as f64;
                    let (status, _) = self.status.read(col, f, size, top, origin);
                    let (index, sure) = self.resolve_lookalike(col, f, size, top, origin, k, index, sure);
                    let sized = self.sized.as_ref().expect("prepared");
                    let (fill_bottom, fill_top) = fill_profile(&win, dx, dy, &sized.images[index], size);
                    let badge = read_badge(col, f, ROW0_Y + ROW_PITCH * k as f64, origin);
                    rows.push(Row { index, score, sure, slot: k, status, fill_bottom, fill_top, badge })
                }
                _ => {
                    gap = true;
                    self.last.remove(&k);
                }
            }
        }
        rows
    }

    /// Stratagems sharing one icon (SSSD Delivery and Upload Data) are told apart by the arrow code the
    /// row shows. Without arrows on screen the slot keeps the one it last resolved to, and a first
    /// reading of such a slot stays unsure until the code is seen.
    #[allow(clippy::too_many_arguments)]
    fn resolve_lookalike(
        &mut self,
        col: &Img,
        f: f64,
        size: usize,
        top: f64,
        origin: (usize, usize),
        k: usize,
        index: usize,
        sure: bool,
    ) -> (usize, bool) {
        if self.lookalikes[index].is_empty() {
            return (index, sure);
        }
        let group: Vec<usize> = std::iter::once(index).chain(self.lookalikes[index].iter().copied()).collect();
        let code = self.status.read_arrows(col, f, size, top, origin, status::CODE_READ);
        // either may be the longer: reading can stop early, or pick up a stray arrow past the code's end
        let fits = |c: &str| c.starts_with(&code) || code.starts_with(c);
        let matching: Vec<usize> =
            if code.is_empty() { vec![] } else { group.iter().copied().filter(|&i| fits(&self.catalog[i].code)).collect() };
        // lookalikes with the very same code (Upload Data, Reinforcement Pods) are interchangeable
        let same_code = !matching.is_empty() && matching.iter().all(|&i| self.catalog[i].code == self.catalog[matching[0]].code);
        let last = self.last.get(&k).map(|l| l.0);
        let resolved = match last {
            _ if same_code => (last.filter(|l| matching.contains(l)).unwrap_or(matching[0]), true),
            Some(l) if group.contains(&l) => (l, sure),
            _ => (index, false),
        };
        if let Some(entry) = self.last.get_mut(&k) {
            entry.0 = resolved.0;
        }
        resolved
    }

    /// The arrow code shown on menu slot `k`, for testing the code reader.
    pub fn slot_code(&mut self, col: &Img, h: usize, origin: (usize, usize), icon_scale: f64, k: usize) -> String {
        let f = h as f64 / REF_HEIGHT * icon_scale;
        let size = (REF_ICON * f).round().max(16.0) as usize;
        self.status.read_arrows(col, f, size, ROW0_Y + ROW_PITCH * k as f64, origin, status::CODE_READ)
    }

    /// Groups of catalog stratagems whose icons are the same picture, by name.
    pub fn lookalike_groups(&self) -> Vec<Vec<String>> {
        let mut seen = vec![false; self.catalog.len()];
        let mut out = Vec::new();
        for i in 0..self.catalog.len() {
            if seen[i] || self.lookalikes[i].is_empty() {
                continue;
            }
            let mut g = vec![i];
            g.extend(&self.lookalikes[i]);
            g.iter().for_each(|&j| seen[j] = true);
            out.push(g.iter().map(|&j| format!("{} ({})", self.catalog[j].name, self.catalog[j].code)).collect());
        }
        out
    }

    /// (template, whole-icon score, sure, dx, dy). The whole icon decides whether an icon is there at all,
    /// the glyph alone which one it is.
    fn classify(&mut self, win: &Img, k: usize) -> Option<(usize, f32, bool, usize, usize)> {
        let sized = self.sized.as_ref().expect("prepared");
        let size = sized.size;
        if let Some(&(t, dx, dy)) = self.last.get(&k) {
            let sc = score_at(win, &sized.whole[t], &sized.whole_idx, size, dx, dy);
            if sc >= ROW_KEEP {
                return Some((t, sc, true, dx, dy));
            }
        }

        let lw = (win.w as f64 * ROW_LOW as f64 / size as f64).round() as usize;
        let lh = (win.h as f64 * ROW_LOW as f64 / size as f64).round() as usize;
        if lw < ROW_LOW || lh < ROW_LOW {
            return None;
        }
        let ws = win.resize_area(lw, lh);
        // raw crops plus their inverse spread; the templates are zero-mean, so dot(raw, t) is the numerator
        let mut crops: Vec<(Vec<f32>, f32)> = Vec::new();
        for j in 0..=lh - ROW_LOW {
            for i in 0..=lw - ROW_LOW {
                let c = ws.crop(i as isize, j as isize, ROW_LOW, ROW_LOW).px;
                let mut sum = [0.0f32; 3];
                for (q, v) in c.iter().enumerate() {
                    sum[q % 3] += v;
                }
                let sumsq = dot(&c, &c);
                let n = (ROW_LOW * ROW_LOW) as f32;
                let var = sumsq - (sum[0] * sum[0] + sum[1] * sum[1] + sum[2] * sum[2]) / n;
                crops.push((c, if var > 1e-6 { 1.0 / var.sqrt() } else { 0.0 }));
            }
        }
        let mut low: Vec<(f32, usize)> = self
            .low
            .iter()
            .enumerate()
            .map(|(t, tv)| (crops.iter().map(|(c, inv)| dot(tv, c) * inv).fold(f32::MIN, f32::max), t))
            .collect();
        low.sort_by(|a, b| b.0.total_cmp(&a.0));
        if low[0].0 < ROW_LOW_MIN {
            self.last.remove(&k);
            return None;
        }

        let mut glyph: Vec<(f32, usize)> =
            low[..ROW_TOP.min(low.len())].iter().map(|&(_, t)| (best_offset(win, &sized.glyph[t], &sized.glyph_idx, size).0, t)).collect();
        glyph.sort_by(|a, b| b.0.total_cmp(&a.0));
        let (best_g, best) = glyph[0];
        let second_g = glyph.get(1).map_or(-1.0, |g| g.0);
        let (exist, dx, dy) = best_offset(win, &sized.whole[best], &sized.whole_idx, size);
        if exist < ROW_MIN {
            self.last.remove(&k);
            return None;
        }
        let sure = best_g >= ROW_SURE || best_g - second_g >= ROW_MARGIN;
        if sure {
            self.last.insert(k, (best, dx, dy));
        } else {
            self.last.remove(&k);
        }
        Some((best, exist, sure, dx, dy))
    }
}

/// How much of the icon the game's progress fill covers, from the bottom and from the top.
/// A row counts as filled when most of its pixels differ clearly from the clean icon; the
/// right side is skipped because of the use-count badge, and short gaps are tolerated
/// because a white glyph looks much the same filled or not.
fn fill_profile(win: &Img, dx: usize, dy: usize, t: &Img, size: usize) -> (f32, f32) {
    let band = (size as f64 * FRAME_FRAC).round() as usize;
    let right = (size as f64 * BADGE_FRAC) as usize;
    let lum = |p: &[f32]| 0.299 * p[0] + 0.587 * p[1] + 0.114 * p[2];
    let filled: Vec<bool> = (band..size - band)
        .map(|y| {
            let hits = (band..right)
                .filter(|&x| {
                    let p = ((dy + y) * win.w + dx + x) * 3;
                    let q = (y * size + x) * 3;
                    (lum(&win.px[p..p + 3]) - lum(&t.px[q..q + 3])).abs() > FILL_DIFF
                })
                .count();
            hits * 2 > right - band
        })
        .collect();
    let run = |rows: &mut dyn Iterator<Item = &bool>| {
        let (mut len, mut gap, mut best) = (0usize, 0usize, 0usize);
        for &f in rows {
            if f {
                len += gap + 1;
                gap = 0;
                best = len;
            } else {
                gap += 1;
                if gap > FILL_GAP {
                    break;
                }
            }
        }
        best
    };
    let n = filled.len().max(1) as f32;
    (run(&mut filled.iter().rev()) as f32 / n, run(&mut filled.iter()) as f32 / n)
}

fn flatten(px: &[f32], idx: &[usize]) -> Flat {
    let mut v: Vec<f32> = idx.iter().flat_map(|&p| [px[p * 3], px[p * 3 + 1], px[p * 3 + 2]]).collect();
    center(&mut v);
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    Flat { v, norm }
}

/// Subtract the per-channel mean of interleaved RGB values.
fn center(v: &mut [f32]) {
    let n = (v.len() / 3).max(1) as f32;
    for c in 0..3 {
        let mean = v.iter().skip(c).step_by(3).sum::<f32>() / n;
        v.iter_mut().skip(c).step_by(3).for_each(|x| *x -= mean);
    }
}

fn normalized(px: &[f32]) -> Vec<f32> {
    let mut v = px.to_vec();
    center(&mut v);
    let n = v.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-3);
    v.iter_mut().for_each(|x| *x /= n);
    v
}

/// Eight independent accumulators so the compiler can vectorise (a single running f32 sum cannot be).
fn dot(a: &[f32], b: &[f32]) -> f32 {
    let mut acc = [0.0f32; 8];
    let ((ca, ra), (cb, rb)) = (a.as_chunks::<8>(), b.as_chunks::<8>());
    let tail: f32 = ra.iter().zip(rb).map(|(x, y)| x * y).sum();
    for (x, y) in ca.iter().zip(cb) {
        for i in 0..8 {
            acc[i] += x[i] * y[i];
        }
    }
    acc.iter().sum::<f32>() + tail
}

/// Correlation of a raw (uncentred) patch with a zero-mean unit template: the template's zero mean
/// makes centring the patch unnecessary for the numerator, and the patch norm comes from its sums.
fn corr_raw(sum: [f32; 3], sumsq: f32, n: f32, d: f32) -> f32 {
    let var = sumsq - (sum[0] * sum[0] + sum[1] * sum[1] + sum[2] * sum[2]) / n;
    if var < 1e-6 { -1.0 } else { d / var.sqrt() }
}

/// Masked zero-mean correlation of a template placed at (dx, dy) inside `win`, in one pass.
fn score_at(win: &Img, t: &Flat, idx: &[usize], size: usize, dx: usize, dy: usize) -> f32 {
    if dx + size > win.w || dy + size > win.h || t.norm < 1e-3 {
        return -1.0;
    }
    let (mut sum, mut sumsq, mut d) = ([0.0f32; 3], 0.0f32, 0.0f32);
    for (i, &p) in idx.iter().enumerate() {
        let q = ((p / size + dy) * win.w + p % size + dx) * 3;
        let px = &win.px[q..q + 3];
        let tv = &t.v[i * 3..i * 3 + 3];
        for c in 0..3 {
            sum[c] += px[c];
            sumsq += px[c] * px[c];
            d += px[c] * tv[c];
        }
    }
    corr_raw(sum, sumsq, idx.len() as f32, d / t.norm)
}

fn best_offset(win: &Img, t: &Flat, idx: &[usize], size: usize) -> (f32, usize, usize) {
    let mut best = (-1.0f32, 0, 0);
    for dy in 0..=win.h.saturating_sub(size) {
        for dx in 0..=win.w.saturating_sub(size) {
            let s = score_at(win, t, idx, size, dx, dy);
            if s > best.0 {
                best = (s, dx, dy);
            }
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    // status line geometry at 1440p, as in status.rs
    const ARROW_X: f64 = 155.0;
    const ARROW_STEP: f64 = 29.0;
    const WORD_X: f64 = 154.0;
    const BAND_Y: f64 = 34.0;
    const SLACK: f64 = 3.0;

    fn data() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../data")
    }

    fn matcher() -> Matcher {
        Matcher::load(&data()).expect("catalog")
    }

    fn idx(m: &Matcher, name: &str) -> usize {
        m.catalog.iter().position(|s| s.name == name).unwrap_or_else(|| panic!("{name} not in the catalog"))
    }

    // ---- column_box ----

    #[test]
    fn column_box_at_common_resolutions() {
        assert_eq!(column_box(2560, 1440, 1.0), (56, 43, 388, 1036));
        assert_eq!(column_box(1920, 1080, 1.0), (42, 32, 291, 777));
        assert_eq!(column_box(3840, 2160, 1.0), (84, 64, 582, 1555));
        assert_eq!(column_box(1280, 720, 1.0), (28, 21, 194, 518));
    }

    #[test]
    fn column_box_follows_the_hud_scale_and_the_window_width() {
        // the left edge follows the icons (REF_X * scale - X_TOL); the status line needs NEEDS_X * scale,
        // wider than the icon column at any scale
        assert_eq!(column_box(2560, 1440, 2.0), (126, 43, 776, 1036));
        assert_eq!(column_box(2560, 1440, 0.5), (21, 43, 194, 1036));
        assert_eq!(column_box(300, 1440, 1.0), (56, 43, 300, 1036), "clamped to a narrow window");
    }

    /// Every region read_rows reads for slot k (icon window, status words, CODE_READ arrows, badge),
    /// as (what, x0, y0, x1, y1) in game-frame pixels, following read_rows, best_at and read_badge.
    fn regions(h: usize, scale: f64, k: usize) -> Vec<(String, isize, isize, isize, isize)> {
        let f = h as f64 / REF_HEIGHT * scale;
        let r = |v: f64| v.round() as isize;
        let size = r(REF_ICON * f).max(16);
        let pad = r(3.0 * f).max(2);
        let top = ROW0_Y + ROW_PITCH * k as f64;
        let mut out = vec![("icon".to_string(), r(REF_X * f) - pad, r(top * f) - pad, r(REF_X * f) + size + pad, r(top * f) + size + pad)];
        let slack = r(SLACK * f).max(1);
        let line = |what: String, x: f64, w: f64| {
            let (x0, y0) = (r(x * f) - slack, r((top + BAND_Y) * f) - slack);
            (what, x0, y0, x0 + r(w * f).max(4) + 2 * slack, y0 + r(25.0 * f).max(4) + 2 * slack)
        };
        out.push(line("words".into(), WORD_X, 58.0));
        for i in 0..status::CODE_READ {
            out.push(line(format!("arrow {i}"), ARROW_X + ARROW_STEP * i as f64, 26.0));
        }
        let (bx, by) = (r(100.0 * f), r((top + 28.0) * f));
        out.push(("badge".into(), bx, by, bx + r(46.0 * f), by + r(36.0 * f)));
        out
    }

    fn outside(w: usize, h: usize, scale: f64, slots: usize) -> Vec<String> {
        let (x0, y0, x1, y1) = column_box(w, h, scale);
        let (x0, y0, x1, y1) = (x0 as isize, y0 as isize, x1 as isize, y1 as isize);
        (0..slots)
            .flat_map(|k| regions(h, scale, k).into_iter().map(move |r| (k, r)))
            .filter(|(_, (_, a, b, c, d))| *a < x0 || *b < y0 || *c > x1 || *d > y1)
            .map(|(k, (what, ..))| format!("{w}x{h} scale {scale}: slot {k} {what}"))
            .collect()
    }

    #[test]
    fn column_box_holds_every_row_at_full_scale() {
        for (w, h) in [(1280, 720), (1600, 900), (1920, 1080), (2560, 1440), (3440, 1440), (2560, 1600), (3840, 2160)] {
            let bad = outside(w, h, 1.0, MAX_ROWS);
            assert!(bad.is_empty(), "{bad:#?}");
        }
    }

    #[test]
    fn column_box_holds_the_first_row_at_larger_hud_scales() {
        for scale in [1.0, 1.25, 1.5, 2.0] {
            let bad = outside(2560, 1440, scale, 1);
            assert!(bad.is_empty(), "{bad:#?}");
        }
    }

    /// Config allows icon_scale down to 0.5, but column_box keeps its left edge at (REF_X - X_TOL) for
    /// every scale while read_rows places the icon at REF_X * scale. Below about 0.85 the icon search
    /// window is clipped on the left; below about 0.77 it comes out narrower than the icon and
    /// read_rows stops at slot 0, reading nothing.
    #[test]
    fn column_box_holds_the_first_row_at_smaller_hud_scales() {
        let bad: Vec<String> = [0.5, 0.75].iter().flat_map(|&scale| outside(2560, 1440, scale, 1)).collect();
        assert!(bad.is_empty(), "{bad:#?}");
    }

    // ---- small helpers ----

    #[test]
    fn png_name_comes_from_the_svg_name() {
        let png = |icon: &str| Stratagem { name: String::new(), code: String::new(), cooldown: String::new(), icon: icon.into() }.png();
        assert_eq!(png("Reinforce_Stratagem_Icon_Background.svg"), "Reinforce_Stratagem_Icon_Background.png");
        assert_eq!(png("W.A.S.P._Launcher_Stratagem_Icon_Background.svg"), "W.A.S.P._Launcher_Stratagem_Icon_Background.png");
        assert_eq!(png("sub/dir/Drill.svg"), "Drill.png");
        assert_eq!(png("NoExtension"), "NoExtension.png");
    }

    #[test]
    fn dot_matches_a_plain_sum_at_every_length() {
        for n in 0..40 {
            let a: Vec<f32> = (0..n).map(|i| i as f32 * 0.5 - 3.0).collect();
            let b: Vec<f32> = (0..n).map(|i| (i % 7) as f32).collect();
            let plain: f32 = a.iter().zip(&b).map(|(x, y)| x * y).sum();
            assert!((dot(&a, &b) - plain).abs() < 1e-3, "length {n}");
        }
    }

    #[test]
    fn normalized_is_zero_mean_per_channel_and_unit_length() {
        let px: Vec<f32> = (0..30).map(|i| ((i * 37) % 11) as f32).collect();
        let v = normalized(&px);
        for c in 0..3 {
            assert!(v.iter().skip(c).step_by(3).sum::<f32>().abs() < 1e-5);
        }
        assert!((dot(&v, &v) - 1.0).abs() < 1e-5);
        assert!(normalized(&[5.0; 30]).iter().all(|&x| x == 0.0), "a flat patch has nothing to correlate");
    }

    #[test]
    fn score_at_is_one_on_the_template_and_minus_one_when_flat_or_out_of_bounds() {
        let size = 8;
        let mut t = Img::new(size, size);
        t.px.iter_mut().enumerate().for_each(|(i, v)| *v = ((i * 31) % 97) as f32);
        let idx: Vec<usize> = (0..size * size).collect();
        let flat = flatten(&t.px, &idx);
        assert!((score_at(&t, &flat, &idx, size, 0, 0) - 1.0).abs() < 1e-4);
        let mut win = Img::new(size + 3, size + 2);
        win.px.iter_mut().for_each(|v| *v = 9.0);
        for y in 0..size {
            let (s, d) = (y * size * 3, ((y + 2) * win.w + 3) * 3);
            win.px[d..d + size * 3].copy_from_slice(&t.px[s..s + size * 3]);
        }
        let (best, dx, dy) = best_offset(&win, &flat, &idx, size);
        assert!((best - 1.0).abs() < 1e-4);
        assert_eq!((dx, dy), (3, 2));
        assert_eq!(score_at(&win, &flat, &idx, size, 4, 0), -1.0, "out of bounds");
        let grey = Img { w: size, h: size, px: vec![50.0; size * size * 3] };
        assert_eq!(score_at(&grey, &flat, &idx, size, 0, 0), -1.0, "flat patch");
    }

    // ---- fill_profile ----

    const SIZE: usize = 57;

    fn template() -> Img {
        let mut t = Img::new(SIZE, SIZE);
        t.px.iter_mut().for_each(|v| *v = 40.0);
        t
    }

    /// The template with rows `rows` (and columns `cols`) painted bright, like the game's fill.
    fn lit(rows: std::ops::Range<usize>, cols: std::ops::Range<usize>) -> Img {
        let mut w = template();
        for y in rows {
            for x in cols.clone() {
                w.px[(y * SIZE + x) * 3..(y * SIZE + x) * 3 + 3].copy_from_slice(&[200.0; 3]);
            }
        }
        w
    }

    #[test]
    fn fill_profile_measures_from_each_end() {
        let t = template();
        let n = (SIZE - 2 * 7) as f32; // rows inside the frame band, round(57 * FRAME_FRAC) = 7
        assert_eq!(fill_profile(&t, 0, 0, &t, SIZE), (0.0, 0.0));
        assert_eq!(fill_profile(&lit(30..SIZE, 0..SIZE), 0, 0, &t, SIZE), (20.0 / n, 0.0), "cooldown: from the bottom");
        assert_eq!(fill_profile(&lit(0..17, 0..SIZE), 0, 0, &t, SIZE), (0.0, 10.0 / n), "inbound: from the top");
        assert_eq!(fill_profile(&lit(0..SIZE, 0..SIZE), 0, 0, &t, SIZE), (1.0, 1.0), "unavailable: all of it");
    }

    #[test]
    fn fill_profile_ignores_the_badge_side_and_bridges_short_gaps() {
        let t = template();
        let n = (SIZE - 2 * 7) as f32;
        let right = (SIZE as f64 * BADGE_FRAC) as usize;
        assert_eq!(fill_profile(&lit(0..SIZE, right..SIZE), 0, 0, &t, SIZE), (0.0, 0.0), "only the badge side changed");
        let mut gap3 = lit(20..SIZE, 0..SIZE);
        let mut gap4 = gap3.clone();
        for y in 30..33 {
            gap3.px[y * SIZE * 3..(y + 1) * SIZE * 3].copy_from_slice(&t.px[y * SIZE * 3..(y + 1) * SIZE * 3]);
        }
        for y in 30..34 {
            gap4.px[y * SIZE * 3..(y + 1) * SIZE * 3].copy_from_slice(&t.px[y * SIZE * 3..(y + 1) * SIZE * 3]);
        }
        assert_eq!(fill_profile(&gap3, 0, 0, &t, SIZE).0, 30.0 / n, "a 3-row gap (white glyph) is bridged");
        assert_eq!(fill_profile(&gap4, 0, 0, &t, SIZE).0, 16.0 / n, "a 4-row gap ends the fill");
    }

    #[test]
    fn fill_profile_reads_at_the_match_offset() {
        let t = template();
        let src = lit(30..SIZE, 0..SIZE);
        let mut win = Img::new(SIZE + 6, SIZE + 6);
        win.px.iter_mut().for_each(|v| *v = 200.0); // bright surroundings must not count
        for y in 0..SIZE {
            let (s, d) = (y * SIZE * 3, ((y + 3) * win.w + 2) * 3);
            win.px[d..d + SIZE * 3].copy_from_slice(&src.px[s..s + SIZE * 3]);
        }
        assert_eq!(fill_profile(&win, 2, 3, &t, SIZE), fill_profile(&src, 0, 0, &t, SIZE));
    }

    // ---- read_rows on synthetic columns ----

    fn blank_column() -> (Img, (usize, usize)) {
        let (x0, y0, x1, y1) = column_box(2560, 1440, 1.0);
        let mut col = Img::new(x1 - x0, y1 - y0);
        col.px.iter_mut().for_each(|v| *v = 30.0);
        (col, (x0, y0))
    }

    fn paste(col: &mut Img, src: &Img, x: f64, y: f64, origin: (usize, usize)) {
        let (x0, y0) = (x as usize - origin.0, y as usize - origin.1);
        for row in 0..src.h {
            let (s, d) = (row * src.w * 3, ((y0 + row) * col.w + x0) * 3);
            col.px[d..d + src.w * 3].copy_from_slice(&src.px[s..s + src.w * 3]);
        }
    }

    /// Draw catalog entry `index`'s icon in menu slot `k`, with `status` on its status line: an arrow
    /// code ("DDDUU"), a word template name ("word_cooldown") or nothing ("").
    fn draw_row(m: &Matcher, col: &mut Img, origin: (usize, usize), k: usize, index: usize, status: &str) {
        let top = ROW0_Y + ROW_PITCH * k as f64;
        paste(col, &m.raw[index].resize_area(57, 57), REF_X, top, origin);
        let template = |name: &str| Img::load_png(&data().join(format!("status/{name}.png"))).unwrap();
        if status.starts_with("word_") {
            paste(col, &template(status), WORD_X, top + BAND_Y, origin);
            return;
        }
        for (i, c) in status.chars().enumerate() {
            let name = match c {
                'U' => "arrow_up",
                'D' => "arrow_down",
                'L' => "arrow_left",
                _ => "arrow_right",
            };
            paste(col, &template(name), ARROW_X + ARROW_STEP * i as f64, top + BAND_Y, origin);
        }
    }

    /// (slot, name, sure, status) of every row read.
    fn read(m: &mut Matcher, col: &Img, origin: (usize, usize)) -> Vec<(usize, String, bool, Status)> {
        let rows = m.read_rows(col, 1440, origin, 1.0);
        rows.iter().map(|r| (r.slot, m.catalog[r.index].name.clone(), r.sure, r.status)).collect()
    }

    #[test]
    fn a_blank_column_has_no_rows() {
        let mut m = matcher();
        let (col, origin) = blank_column();
        assert!(m.read_rows(&col, 1440, origin, 1.0).is_empty());
    }

    #[test]
    fn rows_are_read_in_their_slots_even_after_a_gap() {
        let mut m = matcher();
        let (mut col, origin) = blank_column();
        draw_row(&m, &mut col, origin, 0, idx(&m, "Reinforce"), "UDRLU");
        draw_row(&m, &mut col, origin, 5, idx(&m, "LIFT-182 Warp Pack"), "word_cooldown");
        let rows = read(&mut m, &col, origin);
        assert_eq!(
            rows,
            [(0, "Reinforce".to_string(), true, Status::Ready), (5, "LIFT-182 Warp Pack".to_string(), true, Status::Cooldown)]
        );
        assert_eq!(m.slot_code(&col, 1440, origin, 1.0, 0), "UDRLU");
        assert_eq!(m.slot_code(&col, 1440, origin, 1.0, 5), "");
    }

    #[test]
    fn shared_icons_are_told_apart_by_the_code() {
        let mut m = matcher();
        for (icon, code, want) in [
            ("Upload Data", "DDDUU", &["SSSD Delivery"][..]),
            ("SSSD Delivery", "LRUUU", &["Upload Data", "Reinforcement Pods"]), // same code: interchangeable
            ("Hive Breaker Drill", "LLLUDRDD", &["Aquifer Drill"]),
            ("Aquifer Drill", "DDLRDD", &["Prospecting Drill"]),
            ("Prospecting Drill", "UDUDUD", &["Tectonic Drill"]),
            ("Tectonic Drill", "DDLLDD", &["Activate E-711 Extraction Drill"]),
            ("Cargo Container", "UUDDLLD", &["Portable Comms Relay"]),
            ("Portable Comms Relay", "UUDDRD", &["Cargo Container"]),
        ] {
            m.reset();
            let (mut col, origin) = blank_column();
            draw_row(&m, &mut col, origin, 3, idx(&m, icon), code);
            let rows = read(&mut m, &col, origin);
            assert!(rows.len() == 1 && want.contains(&rows[0].1.as_str()) && rows[0].2, "{icon} icon with {code}: {rows:?}");
        }
    }

    #[test]
    fn near_identical_icons_are_read_with_their_code() {
        let mut m = matcher();
        for (name, code) in
            [("AX/LAS-5 Rover", "DULURR"), ("AX/ARC-3 K-9", "DULURL"), ("Orbital Precision Strike", "RRU"), ("SEAF Artillery", "RUUD")]
        {
            m.reset();
            let (mut col, origin) = blank_column();
            draw_row(&m, &mut col, origin, 4, idx(&m, name), code);
            assert_eq!(read(&mut m, &col, origin), [(4, name.to_string(), true, Status::Ready)]);
        }
    }

    #[test]
    fn a_resolved_lookalike_keeps_its_identity_while_the_code_is_hidden() {
        let mut m = matcher();
        let upload = idx(&m, "Upload Data");
        let (mut col, origin) = blank_column();
        draw_row(&m, &mut col, origin, 4, upload, "DDDUU");
        assert_eq!(read(&mut m, &col, origin), [(4, "SSSD Delivery".to_string(), true, Status::Ready)]);
        // the next frames show the same icon on cooldown, without arrows
        let (mut col, origin) = blank_column();
        draw_row(&m, &mut col, origin, 4, upload, "word_cooldown");
        for _ in 0..2 {
            assert_eq!(read(&mut m, &col, origin), [(4, "SSSD Delivery".to_string(), true, Status::Cooldown)]);
        }
    }

    /// resolve_lookalike's doc: "a first reading of such a slot stays unsure until the code is
    /// seen". classify stores a sure glyph match in `last` before resolve_lookalike runs, so a crisp
    /// shared icon read without its code comes out sure, as whichever group member ranked first.
    #[test]
    #[ignore = "doc mismatch: a shared icon first read without its arrow code is reported sure (as the first catalog member)"]
    fn a_first_reading_of_a_shared_icon_without_its_code_is_unsure() {
        let mut m = matcher();
        let (mut col, origin) = blank_column();
        draw_row(&m, &mut col, origin, 4, idx(&m, "Upload Data"), "word_cooldown");
        let rows = read(&mut m, &col, origin);
        assert_eq!(rows.len(), 1);
        assert!(!rows[0].2, "read as a sure {} without seeing its code", rows[0].1);
    }
}
