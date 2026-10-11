use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use hd_scanner::data::Data;
use hd_scanner::img::Img;
use hd_scanner::matcher::{Matcher, column_box};

fn usage() -> ! {
    eprintln!("usage: hd-scanner bench <data dir> <frame.png | dir>...");
    eprintln!("       hd-scanner live <data dir> <seconds> [period ms]");
    std::process::exit(2);
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("bench") if args.len() >= 3 => bench(Path::new(&args[1]), &args[2..]),
        Some("lookalikes") if args.len() >= 2 => {
            let m = Matcher::load(&Data::Dir(PathBuf::from(&args[1]))).unwrap_or_else(|e| {
                eprintln!("{e}");
                std::process::exit(1)
            });
            for g in m.lookalike_groups() {
                println!("{}", g.join("  =  "));
            }
        }
        Some("window") => {
            hd_scanner::capture::set_dpi_aware();
            for _ in 0..5 {
                let found = hd_scanner::capture::find_window("helldivers2.exe")
                    .map(|g| format!("pid {} at {},{} {}x{}", g.pid, g.x, g.y, g.w, g.h));
                println!("find_window: {:?} | {}", found, hd_scanner::capture::diagnose("helldivers2.exe"));
                std::thread::sleep(Duration::from_millis(500));
            }
        }
        Some("live") if args.len() >= 3 => {
            let secs: f64 = args[2].parse().unwrap_or_else(|_| usage());
            let period: u64 = args.get(3).and_then(|p| p.parse().ok()).unwrap_or(150);
            live(Path::new(&args[1]), secs, period)
        }
        _ => usage(),
    }
}

/// Scan the running game at a fixed period, the way the plugin does, and report timings: finding the
/// game, grab, read.
fn live(data: &Path, secs: f64, period_ms: u64) {
    hd_scanner::capture::set_dpi_aware();
    let mut m = Matcher::load(&Data::Dir(data.to_path_buf())).unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(1)
    });
    let mut grabber = hd_scanner::capture::Grabber::new();
    let (mut find_ms, mut grab_ms, mut read_ms, mut total_ms) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let mut unfocused = 0;
    let mut last = String::new();
    let start = Instant::now();
    let mut polled = None;
    while start.elapsed().as_secs_f64() < secs {
        let tick = Instant::now();
        // the plugin checks that the game runs every two seconds, which also refreshes its process IDs
        if polled.is_none_or(|p: Instant| p.elapsed() >= Duration::from_secs(2)) {
            hd_scanner::capture::running("helldivers2.exe");
            polled = Some(Instant::now());
        }
        let t = Instant::now();
        let found = hd_scanner::capture::locate("helldivers2.exe");
        find_ms.push(t.elapsed().as_secs_f64() * 1000.0);
        match found {
            None => unfocused += 1,
            Some(fg) => {
                let (x0, y0, x1, y1) = column_box(fg.w, fg.h, 1.0);
                let t = Instant::now();
                match grabber.grab(fg.x + x0 as i32, fg.y + y0 as i32, (x1 - x0) as i32, (y1 - y0) as i32) {
                    Err(e) => eprintln!("{e}"),
                    Ok(col) => {
                        let g = t.elapsed().as_secs_f64() * 1000.0;
                        let t = Instant::now();
                        let rows = m.read_rows(&col, fg.h, (x0, y0), 1.0);
                        read_ms.push(t.elapsed().as_secs_f64() * 1000.0);
                        grab_ms.push(g);
                        total_ms.push(tick.elapsed().as_secs_f64() * 1000.0);
                        last = rows
                            .iter()
                            .map(|r| format!("{}{}", m.catalog[r.index].name, if r.sure { "" } else { "?" }))
                            .collect::<Vec<_>>()
                            .join(", ");
                    }
                }
            }
        }
        if let Some(rest) = Duration::from_millis(period_ms).checked_sub(tick.elapsed()) {
            std::thread::sleep(rest);
        }
    }
    let stat = |v: &mut Vec<f64>| {
        if v.is_empty() {
            return "n/a".to_string();
        }
        v.sort_by(f64::total_cmp);
        format!("median {:.1} ms, p95 {:.1} ms, max {:.1} ms", v[v.len() / 2], v[(v.len() as f64 * 0.95) as usize], v[v.len() - 1])
    };
    println!("scans {} (game not found {unfocused})", total_ms.len());
    println!("find  {}", stat(&mut find_ms));
    println!("grab  {}", stat(&mut grab_ms));
    println!("read  {}", stat(&mut read_ms));
    println!("total {}", stat(&mut total_ms));
    println!("last rows: {last}");
}

/// Replay saved frames in order (row cache warm, like a live scan) and summarise what was read.
fn bench(data: &Path, inputs: &[String]) {
    let mut m = Matcher::load(&Data::Dir(data.to_path_buf())).unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(1)
    });
    for input in inputs {
        let p = PathBuf::from(input);
        let files: Vec<PathBuf> = if p.is_dir() {
            let mut v: Vec<PathBuf> = std::fs::read_dir(&p)
                .expect("read dir")
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|f| f.extension().is_some_and(|x| x == "png"))
                .collect();
            v.sort();
            v
        } else {
            vec![p.clone()]
        };
        m.reset();
        let mut times = Vec::new();
        let mut shapes: BTreeMap<usize, usize> = BTreeMap::new();
        let mut names: BTreeMap<String, usize> = BTreeMap::new();
        for (i, f) in files.iter().enumerate() {
            let frame = match Img::load_png(f) {
                Ok(img) => img,
                Err(e) => {
                    eprintln!("{e}");
                    continue;
                }
            };
            let (x0, y0, x1, y1) = column_box(frame.w, frame.h, 1.0);
            let col = frame.crop(x0 as isize, y0 as isize, x1 - x0, y1 - y0);
            let t = Instant::now();
            let rows = m.read_rows(&col, frame.h, (x0, y0), 1.0);
            let ms = t.elapsed().as_secs_f64() * 1000.0;
            if i > 0 {
                times.push(ms);
            }
            *shapes.entry(rows.len()).or_default() += 1;
            for r in &rows {
                let n = &m.catalog[r.index].name;
                let n = if r.sure { n.clone() } else { format!("{n} (unsure)") };
                *names.entry(format!("{n} [{:?}]", r.status)).or_default() += 1;
            }
            if files.len() == 1 {
                let codes: Vec<String> = rows.iter().map(|r| m.slot_code(&col, frame.h, (x0, y0), 1.0, r.slot)).collect();
                println!(
                    "codes read: {}",
                    rows.iter()
                        .zip(&codes)
                        .map(|(r, c)| format!(
                            "{}={} (wiki {})",
                            m.catalog[r.index].name,
                            if c.is_empty() { "-" } else { c },
                            m.catalog[r.index].code
                        ))
                        .collect::<Vec<_>>()
                        .join(" | ")
                );
                let detail: Vec<String> = rows
                    .iter()
                    .map(|r| {
                        format!(
                            "{} {:.3} slot {}{} [{:?}{} fill b{:.2} t{:.2}{}]",
                            m.catalog[r.index].name,
                            r.score,
                            r.slot,
                            if r.sure { "" } else { " unsure" },
                            r.status,
                            r.timer.map_or(String::new(), |s| format!(" T-{:02}:{:02}", s / 60, s % 60)),
                            r.fill_bottom,
                            r.fill_top,
                            r.badge.as_ref().map_or(String::new(), |b| format!(" badge {}x{}", b.w, b.h))
                        )
                    })
                    .collect();
                println!("{}: {}", f.file_name().unwrap().to_string_lossy(), detail.join(" | "));
            }
        }
        if !times.is_empty() {
            times.sort_by(f64::total_cmp);
            let med = times[times.len() / 2];
            let p95 = times[(times.len() as f64 * 0.95) as usize];
            println!(
                "{}: {} frames, median {:.1} ms, p95 {:.1} ms, max {:.1} ms | rows per frame {:?}",
                p.file_name().unwrap().to_string_lossy(),
                files.len(),
                med,
                p95,
                times[times.len() - 1],
                shapes
            );
            println!("  names: {names:?}");
        }
    }
}
