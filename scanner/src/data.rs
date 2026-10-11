//! Where the reader's data comes from: the stratagem list and the pictures it matches, named by their
//! path in the data folder ("stratagems.json", "icons_144/<icon>.png", "status/<template>.png").

use std::borrow::Cow;
use std::path::PathBuf;

use crate::img::Img;

mod embedded {
    include!(concat!(env!("OUT_DIR"), "/embedded_data.rs"));
}

#[derive(Clone, Debug)]
pub enum Data {
    /// A data folder on disk: the repository's data/, for hd-scanner, the tests and the tools.
    Dir(PathBuf),
    /// The copy built into the binary from ../data (build.rs). The plugin uses it: under the Elgato
    /// Marketplace's DRM a plugin cannot read its own files.
    Embedded,
}

impl Data {
    /// The bytes of the file at `rel`, borrowed when built in.
    pub fn read(&self, rel: &str) -> Result<Cow<'static, [u8]>, String> {
        match self {
            Data::Dir(dir) => {
                let path = dir.join(rel);
                std::fs::read(&path).map(Cow::Owned).map_err(|e| format!("{}: {e}", path.display()))
            }
            Data::Embedded => match embedded::FILES.binary_search_by(|&(name, _)| name.cmp(rel)) {
                Ok(i) => Ok(Cow::Borrowed(embedded::FILES[i].1)),
                Err(_) => Err(format!("{rel}: not built in")),
            },
        }
    }

    /// The PNG picture at `rel`, decoded.
    pub fn png(&self, rel: &str) -> Result<Img, String> {
        let bytes = self.read(rel)?;
        Img::decode_png(&bytes).map_err(|e| match self {
            Data::Dir(dir) => format!("{}: {e}", dir.join(rel).display()),
            Data::Embedded => format!("{rel}: {e}"),
        })
    }

    /// Every file built in, by path, sorted.
    pub fn embedded_files() -> &'static [(&'static str, &'static [u8])] {
        embedded::FILES
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    fn dir() -> Data {
        Data::Dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("../data"))
    }

    #[test]
    fn the_table_is_sorted_with_no_duplicates() {
        let names: Vec<&str> = Data::embedded_files().iter().map(|f| f.0).collect();
        assert!(names.windows(2).all(|w| w[0] < w[1]), "the table must be sorted for the binary search");
    }

    #[test]
    fn both_sources_give_the_same_bytes_and_pictures() {
        for rel in ["stratagems.json", "icons_144/Reinforce_Stratagem_Icon_Background.png", "status/digit_7.png"] {
            let (built_in, on_disk) = (Data::Embedded.read(rel).unwrap(), dir().read(rel).unwrap());
            assert!(matches!(built_in, Cow::Borrowed(_)), "{rel}: built-in files are not copied");
            assert_eq!(built_in, on_disk, "{rel}");
        }
        let (a, b) = (Data::Embedded.png("status/arrow_up.png").unwrap(), dir().png("status/arrow_up.png").unwrap());
        assert_eq!((a.w, a.h, &a.px), (b.w, b.h, &b.px));
    }

    #[test]
    fn missing_or_broken_files_say_which() {
        let Err(e) = Data::Embedded.read("icons_144/Nothing.png") else { panic!("read a file that is not built in") };
        assert!(e.contains("icons_144/Nothing.png"), "{e}");
        let Err(e) = dir().read("icons_144/Nothing.png") else { panic!("read a file that is not there") };
        assert!(e.contains("Nothing.png"), "{e}");
        let Err(e) = Data::Embedded.png("stratagems.json") else { panic!("decoded JSON as a picture") };
        assert!(e.starts_with("stratagems.json: "), "{e}");
        let Err(e) = dir().png("stratagems.json") else { panic!("decoded JSON as a picture") };
        assert!(e.contains("stratagems.json"), "{e}");
        assert!(Data::Dir("no/such/data".into()).read("stratagems.json").is_err());
    }
}
