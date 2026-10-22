//! hd2-watch: watches the game's stratagem HUD and prints what it sees, one JSON object per line on stdout, for
//! another program to act on (the Stratagem Deck plugin starts it and reads its output). It keeps no state about
//! stratagems: it reports the rows it reads, and leaves deciding what they mean to the reader of its output.
//!
//! Lines (`t` is milliseconds since the watcher started):
//!   {"t":0,"ev":"start","version":"0.5.3"}
//!   {"t":120,"ev":"game","state":"running","w":3440,"h":1440}     the game started, or its window size changed
//!   {"t":9000,"ev":"game","state":"closed"}
//!   {"t":4100,"ev":"scale","scale":0.9}                           the HUD scale found on screen (see find_scale)
//!   {"t":4100,"ev":"hud","on":true}                               the stratagem HUD came up (a mission) or went
//!   {"t":5230,"ev":"row","slot":5,"name":"RS-422 Railgun","code":"DRDULR","status":"cooldown","timer":408,"score":0.93,"sure":true}
//!
//! A row is printed when what it shows changes (its stratagem, status or timer), so a visible timer prints once a
//! second. `status` is ready, cooldown, unavailable, active (activating, inbound or impact) or unknown; `timer` is the
//! seconds after "Cooldown T-" or "Inbound T-", or null.
//!
//! It exits when its standard input closes, so that it never outlives the program that started it.
//!
//! Options:
//!   --scale S        the HUD scale to read at, instead of finding it on screen
//!   --period MS      time between two reads while the game runs (default 150)
//!   --frames DIR     read the PNG pictures of DIR in name order instead of the screen, as if each were the game
//!                    (for tests and for checking saved screenshots); stdin is not watched then
//!   --stay           do not exit when stdin closes

use std::collections::HashMap;
use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use hd_scanner::capture::{self, GameWindow, Grabber};
use hd_scanner::data::Data;
use hd_scanner::img::Img;
use hd_scanner::matcher::{Matcher, Row, column_box};
use hd_scanner::status::Status;
use serde_json::{Value, json};

const PROCESS: &str = "helldivers2.exe";
const GAME_POLL: Duration = Duration::from_secs(2); // how often a closed game is looked for
const HUD_GONE: Duration = Duration::from_secs(3); // no Reinforce this long: the HUD is off (ship, menus, map)

// Finding the HUD scale: while the scale in use reads no row, one other scale is tried every PROBE_EVERY, nearest to
// 1 first, until one reads Reinforce surely in the first row; then the scales around it, PROBE_STEP / 2 apart, and
// the middle of those that read it too is kept until the game closes. The game draws its HUD at about 1.11 times its
// own HUD scale setting (0.8 -> 0.89, 1 -> 1.11, 1.2 -> 1.33).
const PROBE_EVERY: Duration = Duration::from_millis(400);
const PROBE_STEP: f64 = 0.02;
const PROBE_AROUND: i32 = 4;
// Reinforce alone pins the scale to about 0.03, but the lower rows of the open menu drift off their places within
// 0.01 of the right one. So the first time the open menu shows (REFINE_ROWS rows or more), the scales around the one
// in use, REFINE_STEP apart, are read too, and the one reading the most rows best is kept.
// (3: Reinforce, SOS Beacon and Resupply, which read at a scale a little off where your own four rows below them do
// not, as on 22 October with four sentries at 0.9 for 0.89)
const REFINE_ROWS: usize = 3;
const REFINE_STEP: f64 = 0.005;
const REFINE_AROUND: i32 = 6;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let opt = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
    let fixed_scale: Option<f64> = opt("--scale").and_then(|s| s.parse().ok()).filter(|s: &f64| (0.5..=2.0).contains(s));
    let period = Duration::from_millis(opt("--period").and_then(|s| s.parse().ok()).unwrap_or(150u64).clamp(30, 5000));
    let frames: Option<Vec<PathBuf>> = opt("--frames").map(|dir| {
        let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
            .unwrap_or_else(|e| fail(&format!("{dir}: {e}")))
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("png")))
            .collect();
        files.sort();
        files
    });
    if frames.is_none() && !args.iter().any(|a| a == "--stay") {
        // the reader is gone when our stdin closes: go too
        std::thread::spawn(|| {
            let mut sink = String::new();
            let stdin = std::io::stdin();
            let mut lock = stdin.lock();
            while lock.read_line(&mut sink).is_ok_and(|n| n > 0) {
                sink.clear();
            }
            std::process::exit(0);
        });
    }
    capture::set_dpi_aware();
    let matcher = Matcher::load(&Data::Embedded).unwrap_or_else(|e| fail(&format!("cannot load data: {e}")));
    let mut w = Watcher {
        out: Out::new(),
        matcher,
        grabber: Grabber::new(),
        scale: fixed_scale,
        fixed: fixed_scale.is_some(),
        probe_next: 0,
        probe_at: None,
        size: None,
        hud: false,
        hud_seen: None,
        refined: false,
        shown: HashMap::new(),
    };
    w.out.emit(json!({ "ev": "start", "version": env!("CARGO_PKG_VERSION") }));
    match frames {
        Some(files) => {
            for f in files {
                let img = Img::load_png(&f).unwrap_or_else(|e| fail(&format!("{}: {e}", f.display())));
                w.out.emit(json!({ "ev": "frame", "file": f.file_name().map(|n| n.to_string_lossy().into_owned()) }));
                w.read(&Source::Frame(img));
            }
        }
        None => w.watch(period),
    }
}

fn fail(msg: &str) -> ! {
    eprintln!("hd2-watch: {msg}");
    std::process::exit(1)
}

/// Where the game's picture comes from.
enum Source {
    Screen(GameWindow),
    Frame(Img),
}

struct Out {
    started: Instant,
}

impl Out {
    fn new() -> Out {
        Out { started: Instant::now() }
    }
    fn emit(&self, mut v: Value) {
        if let Value::Object(m) = &mut v {
            let mut o = serde_json::Map::new();
            o.insert("t".into(), json!(self.started.elapsed().as_millis() as u64));
            o.extend(std::mem::take(m));
            v = Value::Object(o);
        }
        let mut stdout = std::io::stdout().lock();
        if writeln!(stdout, "{v}").and_then(|_| stdout.flush()).is_err() {
            std::process::exit(0); // nobody reads us any more
        }
    }
}

struct Watcher {
    out: Out,
    matcher: Matcher,
    grabber: Grabber,
    scale: Option<f64>,                                  // the HUD scale read at: given, or found on screen
    fixed: bool,                                         // given with --scale: never searched for
    probe_next: usize,                                   // the next of probe_scales() to try
    probe_at: Option<Instant>,                           // when one was last tried
    size: Option<(usize, usize)>,                        // the game's window size, as last reported
    hud: bool,                                           // the stratagem HUD is up, as last reported
    hud_seen: Option<Instant>,                           // last time Reinforce was read
    refined: bool,                                       // the scale in use was refined on the open menu (see REFINE_ROWS)
    shown: HashMap<usize, (usize, Status, Option<u32>)>, // menu slot -> what it last showed, as printed
}

impl Watcher {
    fn watch(&mut self, period: Duration) {
        let mut running = false;
        let mut looked: Option<Instant> = None;
        loop {
            let t = Instant::now();
            if !running {
                if looked.is_none_or(|l| l.elapsed() >= GAME_POLL) {
                    looked = Some(t);
                    running = capture::running(PROCESS);
                }
                if !running {
                    std::thread::sleep(Duration::from_millis(250));
                    continue;
                }
            }
            match capture::locate(PROCESS) {
                Some(g) => self.read(&Source::Screen(g)),
                None => {
                    running = false;
                    self.closed();
                }
            }
            if let Some(rest) = period.checked_sub(t.elapsed()) {
                std::thread::sleep(rest);
            }
        }
    }

    /// The game is gone: say so once, and forget what it showed.
    fn closed(&mut self) {
        if self.size.take().is_some() {
            self.out.emit(json!({ "ev": "game", "state": "closed" }));
        }
        self.set_hud(false);
        self.shown.clear();
        if !self.fixed {
            self.scale = None; // the next game may run with another HUD scale
            self.refined = false;
        }
    }

    fn set_hud(&mut self, on: bool) {
        if self.hud != on {
            self.hud = on;
            self.out.emit(json!({ "ev": "hud", "on": on }));
            if !on {
                self.shown.clear();
            }
        }
    }

    /// One look at the game: its rows at the scale in use, or a probe for the scale while none reads.
    fn read(&mut self, src: &Source) {
        let (w, h) = match src {
            Source::Screen(g) => (g.w, g.h),
            Source::Frame(img) => (img.w, img.h),
        };
        if self.size != Some((w, h)) {
            self.size = Some((w, h));
            self.out.emit(json!({ "ev": "game", "state": "running", "w": w, "h": h }));
        }
        let mut rows = match self.scale {
            Some(s) => self.rows_at(src, s),
            None => Vec::new(),
        };
        if rows.is_empty() && !self.fixed && self.hud_seen.is_none_or(|t| t.elapsed() >= HUD_GONE) {
            // nothing at the scale in use for a while (or none found yet): the HUD may be up at another one
            if let Some((s, found)) = self.find_scale(src) {
                self.scale = Some(s);
                self.refined = false;
                self.out.emit(json!({ "ev": "scale", "scale": s }));
                rows = found;
            }
        }
        if !self.fixed
            && !self.refined
            && rows.len() >= REFINE_ROWS
            && let Some(cur) = self.scale
        {
            self.refined = true;
            let (s, best) = self.refine(src, cur, rows);
            if s != cur {
                self.scale = Some(s);
                self.out.emit(json!({ "ev": "scale", "scale": s, "refined": true }));
            }
            rows = best;
        }
        if self.reinforce_first(&rows) {
            self.hud_seen = Some(Instant::now());
            self.set_hud(true);
        } else if self.hud_seen.is_none_or(|t| t.elapsed() >= HUD_GONE) || matches!(src, Source::Frame(_)) && rows.is_empty() {
            self.set_hud(false);
        }
        let mut now: HashMap<usize, (usize, Status, Option<u32>)> = HashMap::new();
        for r in &rows {
            let what = (r.index, r.status, r.timer);
            now.insert(r.slot, what);
            if self.shown.get(&r.slot) != Some(&what) {
                let s = &self.matcher.catalog[r.index];
                self.out.emit(json!({
                    "ev": "row", "slot": r.slot, "name": s.name, "code": s.code, "status": status_name(r.status),
                    "timer": r.timer, "score": (f64::from(r.score) * 100.0).round() / 100.0, "sure": r.sure,
                }));
            }
        }
        // a slot whose row went away prints again when it comes back
        self.shown = now;
    }

    fn rows_at(&mut self, src: &Source, scale: f64) -> Vec<Row> {
        let (w, h) = match src {
            Source::Screen(g) => (g.w, g.h),
            Source::Frame(img) => (img.w, img.h),
        };
        let (x0, y0, x1, y1) = column_box(w, h, scale);
        let col = match src {
            Source::Screen(g) => match self.grabber.grab(g.x + x0 as i32, g.y + y0 as i32, (x1 - x0) as i32, (y1 - y0) as i32) {
                Ok(img) => img,
                Err(_) => return Vec::new(),
            },
            Source::Frame(img) => img.crop(x0 as isize, y0 as isize, x1 - x0, y1 - y0),
        };
        self.matcher.read_rows(&col, h, (x0, y0), scale)
    }

    /// The scale around `cur` that reads the most rows, and the best scores among equals, with its rows.
    fn refine(&mut self, src: &Source, cur: f64, rows: Vec<Row>) -> (f64, Vec<Row>) {
        let rank = |rows: &[Row]| (rows.iter().filter(|r| r.sure).count(), rows.iter().map(|r| r.score).sum::<f32>());
        let mut best = (rank(&rows), cur, rows);
        for d in (-REFINE_AROUND..=REFINE_AROUND).filter(|&d| d != 0) {
            let s = ((cur + REFINE_STEP * d as f64) * 1000.0).round() / 1000.0;
            if !(0.5..=2.0).contains(&s) {
                continue;
            }
            let rows = self.rows_at(src, s);
            let r = rank(&rows);
            if r.0 > best.0.0 || (r.0 == best.0.0 && r.1 > best.0.1) {
                best = (r, s, rows);
            }
        }
        (best.1, best.2)
    }

    fn reinforce_first(&self, rows: &[Row]) -> bool {
        rows.first().is_some_and(|r| r.slot == 0 && r.sure && self.matcher.catalog[r.index].name == "Reinforce")
    }

    /// Try the next scale (on the screen, at most one each PROBE_EVERY; on a saved frame, all of them): on a hit,
    /// the middle of the scales around it that read Reinforce too, and the rows read at it.
    fn find_scale(&mut self, src: &Source) -> Option<(f64, Vec<Row>)> {
        let scales = probe_scales();
        let tries = match src {
            Source::Screen(_) => {
                if self.probe_at.is_some_and(|t| t.elapsed() < PROBE_EVERY) {
                    return None;
                }
                self.probe_at = Some(Instant::now());
                1
            }
            Source::Frame(_) => scales.len(),
        };
        for _ in 0..tries {
            let s = scales[self.probe_next % scales.len()];
            self.probe_next = (self.probe_next + 1) % scales.len();
            if self.scale.is_some_and(|cur| (cur - s).abs() < PROBE_STEP / 4.0) {
                continue; // read already
            }
            let rows = self.rows_at(src, s);
            if !self.reinforce_first(&rows) {
                continue;
            }
            let mut hits = Vec::new();
            for d in -PROBE_AROUND..=PROBE_AROUND {
                let t = s + PROBE_STEP / 2.0 * d as f64;
                if (0.5..=2.0).contains(&t) {
                    let around = self.rows_at(src, t);
                    if self.reinforce_first(&around) {
                        hits.push(t);
                    }
                }
            }
            let middle = match (hits.first(), hits.last()) {
                (Some(lo), Some(hi)) => ((lo + hi) / 2.0 * 100.0).round() / 100.0,
                _ => s,
            };
            self.probe_next = 0;
            return Some((middle, self.rows_at(src, middle)));
        }
        None
    }
}

/// The scales a probe tries, in order: 1, then further and further from it either way, within 0.5 to 2.
fn probe_scales() -> Vec<f64> {
    let mut out = vec![1.0];
    for d in 1..=(1.0 / PROBE_STEP) as i32 {
        let step = |sign: f64| ((1.0 + sign * PROBE_STEP * d as f64) * 100.0).round() / 100.0;
        for s in [step(1.0), step(-1.0)] {
            if (0.5..=2.0).contains(&s) {
                out.push(s);
            }
        }
    }
    out
}

fn status_name(s: Status) -> &'static str {
    match s {
        Status::Ready => "ready",
        Status::Cooldown => "cooldown",
        Status::Unavailable => "unavailable",
        Status::Active => "active",
        Status::Unknown => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scales_are_probed_nearest_to_one_first_over_the_whole_range() {
        let s = probe_scales();
        assert_eq!(s[..5], [1.0, 1.02, 0.98, 1.04, 0.96]);
        assert!(s.contains(&0.5) && s.contains(&2.0) && s.contains(&0.88) && s.contains(&1.1), "{s:?}");
        assert!(s.iter().all(|x| (0.5..=2.0).contains(x)));
    }

    #[test]
    fn statuses_have_lower_case_names() {
        assert_eq!(status_name(Status::Cooldown), "cooldown");
        assert_eq!(status_name(Status::Active), "active");
    }
}
