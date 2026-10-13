//! Integrity of data/stratagems.json and the icons it points to, and of the copy built into the binaries.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use hd_scanner::data::Data;
use hd_scanner::img::Img;
use hd_scanner::matcher::{Matcher, Stratagem};
use hd_scanner::status::CODE_READ;

fn data() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../data")
}

fn entries() -> Vec<Stratagem> {
    let text = std::fs::read_to_string(data().join("stratagems.json")).expect("stratagems.json");
    serde_json::from_str(&text).expect("stratagems.json parses")
}

#[test]
fn every_entry_has_every_field() {
    let all = entries();
    assert!(all.len() > 100, "only {} entries", all.len());
    for s in &all {
        assert!(!s.name.is_empty() && s.name == s.name.trim(), "name {:?}", s.name);
        assert!(!s.code.is_empty() && !s.cooldown.is_empty(), "{}", s.name);
        assert!(s.icon.ends_with(".svg"), "{}: icon {}", s.name, s.icon);
    }
}

#[test]
fn names_are_unique() {
    let mut seen = HashSet::new();
    for s in entries() {
        assert!(seen.insert(s.name.clone()), "{} listed twice", s.name);
    }
}

#[test]
fn codes_use_only_arrows() {
    for s in entries() {
        assert!(!s.code.is_empty() && s.code.chars().all(|c| "UDLR".contains(c)), "{}: {:?}", s.name, s.code);
        assert!(s.code.len() <= 10, "{}: code of {} arrows", s.name, s.code.len());
    }
}

/// The game gives every stratagem its own code; the one exception shares its icon too, so the
/// scanner treats the two as interchangeable.
#[test]
fn codes_are_unique_except_upload_data_and_reinforcement_pods() {
    let mut by_code: HashMap<String, Vec<String>> = HashMap::new();
    for s in entries() {
        by_code.entry(s.code).or_default().push(s.name);
    }
    let mut shared: Vec<Vec<String>> = by_code.into_values().filter(|v| v.len() > 1).collect();
    for v in &mut shared {
        v.sort();
    }
    assert_eq!(shared, [["Reinforcement Pods", "Upload Data"]]);
}

#[test]
fn cooldowns_are_seconds_or_not_applicable() {
    for s in entries() {
        let ok = s.cooldown == "N/A" || s.cooldown.strip_suffix('s').is_some_and(|n| n.parse::<u32>().is_ok());
        assert!(ok, "{}: cooldown {:?}", s.name, s.cooldown);
    }
}

#[test]
fn every_icon_exists_as_a_144px_png_with_content() {
    let dir = data().join("icons_144");
    for s in entries() {
        let path = dir.join(s.png());
        let img = Img::load_png(&path).unwrap_or_else(|e| panic!("{}: {e}", s.name));
        assert_eq!((img.w, img.h), (144, 144), "{}", s.name);
        // Matcher::load silently drops entries whose icon is all black
        assert!(img.px.iter().any(|&v| v > 0.0), "{}: blank icon", s.name);
    }
}

#[test]
fn the_matcher_keeps_every_entry_in_order() {
    let m = Matcher::load(&Data::Dir(data())).expect("catalog");
    let names: Vec<String> = entries().into_iter().map(|s| s.name).collect();
    let loaded: Vec<String> = m.catalog.iter().map(|s| s.name.clone()).collect();
    assert_eq!(loaded, names);
}

/// Lookalike groups by name, from Matcher::lookalike_groups ("Name (CODE)" strings).
fn lookalike_groups(m: &Matcher) -> Vec<Vec<String>> {
    m.lookalike_groups()
        .into_iter()
        .map(|g| g.into_iter().map(|e| e.rsplit_once(" (").expect("Name (CODE)").0.to_string()).collect())
        .collect()
}

#[test]
fn entries_sharing_an_icon_are_lookalikes() {
    let m = Matcher::load(&Data::Dir(data())).expect("catalog");
    let groups = lookalike_groups(&m);
    let group_of = |name: &str| groups.iter().position(|g| g.iter().any(|n| n == name));
    let mut by_icon: HashMap<String, Vec<String>> = HashMap::new();
    for s in entries() {
        by_icon.entry(s.icon).or_default().push(s.name);
    }
    for (icon, names) in by_icon.iter().filter(|(_, v)| v.len() > 1) {
        let first = group_of(&names[0]);
        assert!(first.is_some(), "{icon}: {names:?} not in any lookalike group");
        for n in names {
            assert_eq!(group_of(n), first, "{icon}: {names:?} split across groups");
        }
    }
    // the groups known when this test was written, by name
    let known: [&[&str]; 3] = [
        &["SSSD Delivery", "Upload Data", "Reinforcement Pods"],
        &["Hive Breaker Drill", "Activate E-711 Extraction Drill", "Prospecting Drill", "Aquifer Drill", "Tectonic Drill"],
        &["Portable Comms Relay", "Cargo Container"],
    ];
    for k in known {
        let g = group_of(k[0]).map(|i| &groups[i]).expect(k[0]);
        assert!(k.iter().all(|n| g.iter().any(|x| x == n)), "{k:?} vs {g:?}");
    }
}

/// resolve_lookalike reads at most CODE_READ arrows and accepts a read that is a prefix of a code
/// (or the other way round): lookalikes must have codes within that length that never fit each other.
#[test]
fn lookalike_codes_can_be_told_apart_by_the_code_reader() {
    let m = Matcher::load(&Data::Dir(data())).expect("catalog");
    let code: HashMap<String, String> = m.catalog.iter().map(|s| (s.name.clone(), s.code.clone())).collect();
    for g in lookalike_groups(&m) {
        for a in &g {
            assert!(code[a].len() <= CODE_READ, "{a}: {} arrows, the reader stops at {CODE_READ}", code[a].len());
            for b in g.iter().filter(|b| *b != a && code[*b] != code[a]) {
                let fits = code[b].starts_with(&code[a]) || code[a].starts_with(&code[b]);
                assert!(!fits, "{a} ({}) and {b} ({}) cannot be told apart", code[a], code[b]);
            }
        }
    }
}

/// What the plugin uses from data/: the stratagem list and every file in icons_144/, keys_144/ and status/.
fn data_files() -> Vec<(String, Vec<u8>)> {
    let mut files = vec![("stratagems.json".to_string(), std::fs::read(data().join("stratagems.json")).unwrap())];
    for dir in ["icons_144", "keys_144", "status"] {
        for entry in std::fs::read_dir(data().join(dir)).unwrap() {
            let path = entry.unwrap().path();
            files.push((format!("{dir}/{}", path.file_name().unwrap().to_str().unwrap()), std::fs::read(&path).unwrap()));
        }
    }
    files.sort();
    files
}

#[test]
fn the_built_in_data_is_exactly_the_data_folder() {
    let built_in: Vec<(String, Vec<u8>)> = Data::embedded_files().iter().map(|&(name, bytes)| (name.to_string(), bytes.to_vec())).collect();
    let on_disk = data_files();
    let names = |files: &[(String, Vec<u8>)]| files.iter().map(|f| f.0.clone()).collect::<Vec<_>>();
    assert_eq!(names(&built_in), names(&on_disk), "built in and in data/ (a new picture builds in on the next build)");
    for ((name, a), (_, b)) in built_in.iter().zip(&on_disk) {
        assert!(a == b, "{name}: built in differs from data/");
    }
    assert!(built_in.len() > 100, "{} files", built_in.len());
}

#[test]
fn the_matcher_reads_the_same_catalog_built_in() {
    let (built_in, on_disk) = (Matcher::load(&Data::Embedded).expect("built in"), Matcher::load(&Data::Dir(data())).expect("data/"));
    let fields =
        |m: &Matcher| m.catalog.iter().map(|s| (s.name.clone(), s.code.clone(), s.cooldown.clone(), s.icon.clone())).collect::<Vec<_>>();
    assert_eq!(fields(&built_in), fields(&on_disk));
    assert_eq!(built_in.lookalike_groups(), on_disk.lookalike_groups());
}

/// Every key picture belongs to a stratagem (named as Stratagem::key_path names it, which tools/make_key_icons.py
/// mirrors), is 144x144, and all but a few stratagems have one.
#[test]
fn every_key_picture_belongs_to_a_stratagem() {
    let m = Matcher::load(&Data::Dir(data())).expect("catalog");
    let expected: Vec<String> = m.catalog.iter().map(|s| s.key_path()).collect();
    let dir = data().join("keys_144");
    let mut found = 0;
    for entry in std::fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        let rel = format!("keys_144/{}", path.file_name().unwrap().to_str().unwrap());
        assert!(expected.contains(&rel), "{rel}: no stratagem of that name");
        let img = Img::load_png(&path).unwrap();
        assert_eq!((img.w, img.h), (144, 144), "{rel}");
        found += 1;
    }
    assert!(found >= m.catalog.len() - 3, "only {found} of {} stratagems have a key picture", m.catalog.len());
}

#[test]
fn key_paths_are_plain_file_names() {
    let m = Matcher::load(&Data::Dir(data())).expect("catalog");
    let k = |name: &str| m.catalog.iter().find(|s| s.name == name).unwrap().key_path();
    assert_eq!(k("AX/LAS-5 Rover"), "keys_144/AX_LAS_5_Rover.png");
    assert_eq!(k("StA-X3 W.A.S.P. Launcher"), "keys_144/StA_X3_W_A_S_P_Launcher.png");
    assert_eq!(k("Orbital 120mm HE Barrage"), "keys_144/Orbital_120mm_HE_Barrage.png");
}
