//! Reading the player's card on the loadout screen, before a mission.
//!
//! tests/fixtures/card_<name>.png is the card area (`card_box(2560, 1440)`) of data/samples/card_<name>.png, frames
//! of a play session recorded at 2560x1440 (not in git); `make_card_fixtures` below rebuilds them.

use std::path::{Path, PathBuf};

use hd_scanner::data::Data;
use hd_scanner::img::Img;
use hd_scanner::matcher::{Matcher, card_box};

const FRAME_W: usize = 2560;
const FRAME_H: usize = 1440;

/// What each frame shows: the card's stratagems in menu order, or None for no card. The mission's own stratagem in
/// the first row (Call In Super Destroyer here) is not read: most of those only join the menu later in a mission.
const CARDS: &[(&str, Option<&[&str]>)] = &[
    (
        "card_ready",
        Some(&["Reinforce", "SoS Beacon", "Resupply", "Orbital Laser", "LAS-98 Laser Cannon", "LIFT-182 Warp Pack", "M-102 Gunner FRV"]),
    ),
    // before the player has chosen: the stratagems every mission gives only
    ("card_mission_only", Some(&["Reinforce", "SoS Beacon", "Resupply"])),
    // choosing a stratagem: the card is hidden behind the list
    ("card_choosing", None),
];

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn fixture_path(name: &str) -> PathBuf {
    manifest_dir().join("tests/fixtures").join(format!("{name}.png"))
}

fn matcher() -> Matcher {
    Matcher::load(&Data::Dir(manifest_dir().join("../data"))).expect("catalog")
}

fn read(m: &Matcher, name: &str) -> Option<Vec<(String, bool)>> {
    let (x0, y0, _, _) = card_box(FRAME_W, FRAME_H);
    let img = Img::load_png(&fixture_path(name)).expect(name);
    m.read_card(&img, FRAME_H, (x0, y0)).map(|c| c.into_iter().map(|(i, sure)| (m.catalog[i].name.clone(), sure)).collect())
}

#[test]
fn fixtures_are_the_card_box_of_a_2560x1440_frame() {
    let (x0, y0, x1, y1) = card_box(FRAME_W, FRAME_H);
    for (name, _) in CARDS {
        let img = Img::load_png(&fixture_path(name)).expect(name);
        assert_eq!((img.w, img.h), (x1 - x0, y1 - y0), "{name}");
    }
}

#[test]
fn the_card_reads_in_menu_order_and_surely() {
    let m = matcher();
    for (name, want) in CARDS {
        let got = read(&m, name);
        let names = got.as_ref().map(|c| c.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>());
        assert_eq!(names.as_deref(), *want, "{name}");
        if let Some(c) = got {
            assert!(c.iter().all(|(_, sure)| *sure), "{name}: {c:?}");
        }
    }
}

#[test]
fn the_card_box_scales_with_the_window_and_stays_inside_it() {
    assert_eq!(card_box(2560, 1440), (70, 1029, 526, 1225));
    let (x0, y0, x1, y1) = card_box(1920, 1080);
    assert_eq!((x0, y0), (53, 772));
    assert!(x1 <= 1920 && y1 <= 1080 && (x1, y1) == (395, 919), "{x1},{y1}");
    let (_, _, x1, y1) = card_box(400, 1440);
    assert_eq!((x1, y1), (400, 1225), "clamped to a narrow window");
}

fn save_png(img: &Img, path: &Path) {
    let bytes: Vec<u8> = img.px.iter().map(|&v| v as u8).collect();
    let file = std::fs::File::create(path).expect("create fixture");
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), img.w as u32, img.h as u32);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    enc.set_compression(png::Compression::High);
    enc.write_header().expect("png header").write_image_data(&bytes).expect("png data");
}

/// Crops data/samples/<name>.png to the card area into tests/fixtures/<name>.png for every frame in CARDS.
/// Run: cargo test --test card -- --ignored --nocapture make_card_fixtures
#[test]
#[ignore = "generator: needs data/samples (full screenshots, not in git); rebuilds the card fixtures"]
fn make_card_fixtures() {
    let (x0, y0, x1, y1) = card_box(FRAME_W, FRAME_H);
    for (name, _) in CARDS {
        let frame = Img::load_png(&manifest_dir().join("../data/samples").join(format!("{name}.png"))).expect(name);
        assert_eq!((frame.w, frame.h), (FRAME_W, FRAME_H), "{name}");
        let out = fixture_path(name);
        save_png(&frame.crop(x0 as isize, y0 as isize, x1 - x0, y1 - y0), &out);
        println!("{name}: {} bytes", std::fs::metadata(&out).unwrap().len());
    }
}
