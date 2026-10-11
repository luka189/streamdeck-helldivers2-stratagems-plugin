use std::path::Path;

/// RGB image, interleaved f32 channels.
#[derive(Clone)]
pub struct Img {
    pub w: usize,
    pub h: usize,
    pub px: Vec<f32>,
}

impl Img {
    pub fn new(w: usize, h: usize) -> Self {
        Img { w, h, px: vec![0.0; w * h * 3] }
    }

    /// Crop clamped to the image bounds; the result may be smaller than asked.
    pub fn crop(&self, x: isize, y: isize, w: usize, h: usize) -> Img {
        let x0 = x.max(0) as usize;
        let y0 = y.max(0) as usize;
        let x1 = ((x + w as isize).max(0) as usize).min(self.w);
        let y1 = ((y + h as isize).max(0) as usize).min(self.h);
        let (cw, ch) = (x1.saturating_sub(x0), y1.saturating_sub(y0));
        if cw == 0 || ch == 0 {
            // wholly outside: x0 or y0 may lie past the edge, where the copy below would index out of range
            return Img::new(cw, ch);
        }
        let mut out = Img::new(cw, ch);
        for row in 0..ch {
            let src = ((y0 + row) * self.w + x0) * 3;
            out.px[row * cw * 3..(row + 1) * cw * 3].copy_from_slice(&self.px[src..src + cw * 3]);
        }
        out
    }

    /// Area-average resampling (what OpenCV calls INTER_AREA when shrinking).
    pub fn resize_area(&self, nw: usize, nh: usize) -> Img {
        let wx = weights(self.w, nw);
        let wy = weights(self.h, nh);
        let mut tmp = vec![0.0f32; nw * self.h * 3];
        for y in 0..self.h {
            for (ox, ws) in wx.iter().enumerate() {
                let mut acc = [0.0f32; 3];
                for &(sx, wgt) in ws {
                    let p = (y * self.w + sx) * 3;
                    for (a, v) in acc.iter_mut().zip(&self.px[p..p + 3]) {
                        *a += v * wgt;
                    }
                }
                tmp[(y * nw + ox) * 3..(y * nw + ox) * 3 + 3].copy_from_slice(&acc);
            }
        }
        let mut out = Img::new(nw, nh);
        for (oy, ws) in wy.iter().enumerate() {
            for x in 0..nw {
                let mut acc = [0.0f32; 3];
                for &(sy, wgt) in ws {
                    let p = (sy * nw + x) * 3;
                    for (a, v) in acc.iter_mut().zip(&tmp[p..p + 3]) {
                        *a += v * wgt;
                    }
                }
                out.px[(oy * nw + x) * 3..(oy * nw + x) * 3 + 3].copy_from_slice(&acc);
            }
        }
        out
    }

    /// Load an 8-bit PNG file (see `decode_png`); errors name the file.
    pub fn load_png(path: &Path) -> Result<Img, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        Img::decode_png(&bytes).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// Decode an 8-bit PNG; alpha is premultiplied onto black, as the thresholds were tuned with.
    pub fn decode_png(png_bytes: &[u8]) -> Result<Img, String> {
        let mut decoder = png::Decoder::new(std::io::Cursor::new(png_bytes));
        decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
        let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
        let mut buf = vec![0u8; reader.output_buffer_size().ok_or("png too large")?];
        let info = reader.next_frame(&mut buf).map_err(|e| e.to_string())?;
        let bytes = &buf[..info.buffer_size()];
        let (w, h) = (info.width as usize, info.height as usize);
        let channels = match info.color_type {
            png::ColorType::Rgb => 3,
            png::ColorType::Rgba => 4,
            png::ColorType::Grayscale => 1,
            png::ColorType::GrayscaleAlpha => 2,
            other => return Err(format!("unsupported colour type {other:?}")),
        };
        let mut img = Img::new(w, h);
        for i in 0..w * h {
            let p = &bytes[i * channels..(i + 1) * channels];
            let (rgb, a) = match channels {
                1 => ([p[0]; 3], 255),
                2 => ([p[0]; 3], p[1]),
                3 => ([p[0], p[1], p[2]], 255),
                _ => ([p[0], p[1], p[2]], p[3]),
            };
            let a = a as f32 / 255.0;
            for (o, &v) in img.px[i * 3..i * 3 + 3].iter_mut().zip(&rgb) {
                *o = v as f32 * a;
            }
        }
        Ok(img)
    }

    /// From a top-down BGRA buffer, as GetDIBits returns it.
    pub fn from_bgra(buf: &[u8], w: usize, h: usize) -> Img {
        let mut img = Img::new(w, h);
        for i in 0..w * h {
            img.px[i * 3] = buf[i * 4 + 2] as f32;
            img.px[i * 3 + 1] = buf[i * 4 + 1] as f32;
            img.px[i * 3 + 2] = buf[i * 4] as f32;
        }
        img
    }
}

/// For each output index, the source indices it covers and their coverage weights.
fn weights(src: usize, dst: usize) -> Vec<Vec<(usize, f32)>> {
    let s = src as f64 / dst as f64;
    (0..dst)
        .map(|o| {
            let (a, b) = (o as f64 * s, (o + 1) as f64 * s);
            let mut v = Vec::new();
            let mut i = a.floor() as usize;
            while (i as f64) < b && i < src {
                let lo = a.max(i as f64);
                let hi = b.min(i as f64 + 1.0);
                if hi > lo {
                    v.push((i, ((hi - lo) / s) as f32));
                }
                i += 1;
            }
            v
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    /// A w x h image whose values count up from 0, so every channel of every pixel differs.
    fn ramp(w: usize, h: usize) -> Img {
        let mut img = Img::new(w, h);
        img.px.iter_mut().enumerate().for_each(|(i, v)| *v = i as f32);
        img
    }

    fn at(img: &Img, x: usize, y: usize) -> [f32; 3] {
        let p = (y * img.w + x) * 3;
        [img.px[p], img.px[p + 1], img.px[p + 2]]
    }

    fn mean(img: &Img, c: usize) -> f32 {
        img.px.iter().skip(c).step_by(3).sum::<f32>() / (img.w * img.h) as f32
    }

    /// Encode a PNG in memory and write it to a file unique to this test process.
    fn write_png(name: &str, (w, h): (u32, u32), color: png::ColorType, depth: png::BitDepth, data: &[u8]) -> PathBuf {
        let mut bytes = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut bytes, w, h);
            enc.set_color(color);
            enc.set_depth(depth);
            if color == png::ColorType::Indexed {
                enc.set_palette(vec![0u8, 0, 0, 200, 100, 50, 10, 20, 30]);
            }
            enc.write_header().unwrap().write_image_data(data).unwrap();
        }
        let path = std::env::temp_dir().join(format!("hd-scanner-img-{}-{name}.png", std::process::id()));
        std::fs::write(&path, bytes).unwrap();
        path
    }

    fn load_tmp(path: PathBuf) -> Img {
        let img = Img::load_png(&path);
        let _ = std::fs::remove_file(&path);
        img.unwrap()
    }

    #[test]
    fn new_is_black() {
        let img = Img::new(4, 3);
        assert_eq!((img.w, img.h, img.px.len()), (4, 3, 36));
        assert!(img.px.iter().all(|&v| v == 0.0));
    }

    #[test]
    fn crop_inside_copies_the_pixels() {
        let img = ramp(6, 5);
        let c = img.crop(2, 1, 3, 2);
        assert_eq!((c.w, c.h), (3, 2));
        for y in 0..2 {
            for x in 0..3 {
                assert_eq!(at(&c, x, y), at(&img, x + 2, y + 1));
            }
        }
        assert_eq!(img.crop(0, 0, 6, 5).px, img.px, "a crop of the whole image is the image");
    }

    #[test]
    fn crop_is_clamped_to_the_bounds() {
        let img = ramp(6, 5);
        let c = img.crop(-2, -1, 4, 3);
        assert_eq!((c.w, c.h), (2, 2));
        assert_eq!(at(&c, 0, 0), at(&img, 0, 0));
        assert_eq!(at(&c, 1, 1), at(&img, 1, 1));
        let c = img.crop(4, 3, 10, 10);
        assert_eq!((c.w, c.h), (2, 2));
        assert_eq!(at(&c, 1, 1), at(&img, 5, 4));
    }

    #[test]
    fn crop_outside_left_top_or_bottom_is_empty() {
        let img = ramp(6, 5);
        for (x, y) in [(-10, 0), (0, -10), (0, 5), (0, 50), (6, 0), (-3, -3), (100, 100)] {
            let c = img.crop(x, y, 3, 3);
            assert!(c.px.is_empty() && c.w * c.h == 0, "crop at {x},{y} gave {}x{}", c.w, c.h);
        }
    }

    /// The doc promises a clamped (here: empty) result, but a crop starting more than one pixel
    /// past the right edge, on rows inside the image, slices `px` out of range and panics.
    #[test]
    fn crop_outside_right_is_empty() {
        let img = ramp(6, 5);
        for (x, y) in [(7, 4), (100, 0)] {
            let c = img.crop(x, y, 3, 3);
            assert!(c.px.is_empty() && c.w * c.h == 0, "crop at {x},{y} gave {}x{}", c.w, c.h);
        }
    }

    #[test]
    fn resize_to_the_same_size_is_exact() {
        let img = ramp(5, 4);
        assert_eq!(img.resize_area(5, 4).px, img.px);
    }

    #[test]
    fn halving_averages_two_by_two_blocks() {
        let img = ramp(4, 2);
        let r = img.resize_area(2, 1);
        assert_eq!((r.w, r.h), (2, 1));
        for x in 0..2 {
            for c in 0..3 {
                let block: f32 = [(2 * x, 0), (2 * x + 1, 0), (2 * x, 1), (2 * x + 1, 1)].iter().map(|&(sx, sy)| at(&img, sx, sy)[c]).sum();
                assert!((r.px[x * 3 + c] - block / 4.0).abs() < 1e-4);
            }
        }
    }

    #[test]
    fn resizing_keeps_the_mean_brightness() {
        let img = ramp(13, 7);
        for (w, h) in [(5, 3), (6, 4), (1, 1), (26, 14), (13, 2)] {
            let r = img.resize_area(w, h);
            assert_eq!((r.w, r.h, r.px.len()), (w, h, w * h * 3));
            for c in 0..3 {
                let (a, b) = (mean(&img, c), mean(&r, c));
                assert!((a - b).abs() < 1e-3 * a, "{w}x{h} channel {c}: {a} vs {b}");
            }
        }
    }

    #[test]
    fn area_weights_cover_each_source_pixel_equally() {
        for (src, dst) in [(144, 57), (144, 24), (63, 27), (58, 44), (5, 5), (3, 7)] {
            let w = weights(src, dst);
            assert_eq!(w.len(), dst);
            let mut cover = vec![0.0f64; src];
            for ws in &w {
                let sum: f32 = ws.iter().map(|x| x.1).sum();
                assert!((sum - 1.0).abs() < 1e-4, "{src}->{dst}: weights sum to {sum}");
                for &(i, wt) in ws {
                    assert!(i < src && wt > 0.0);
                    cover[i] += wt as f64;
                }
            }
            for (i, c) in cover.iter().enumerate() {
                assert!((c - dst as f64 / src as f64).abs() < 1e-4, "{src}->{dst}: source {i} covered {c}");
            }
        }
    }

    #[test]
    fn from_bgra_swaps_to_rgb_and_drops_alpha() {
        let img = Img::from_bgra(&[10, 20, 30, 255, 1, 2, 3, 0], 2, 1);
        assert_eq!((img.w, img.h), (2, 1));
        assert_eq!(img.px, [30.0, 20.0, 10.0, 3.0, 2.0, 1.0]);
    }

    #[test]
    fn load_png_rgb() {
        let img = load_tmp(write_png("rgb", (2, 1), png::ColorType::Rgb, png::BitDepth::Eight, &[1, 2, 3, 250, 251, 252]));
        assert_eq!((img.w, img.h), (2, 1));
        assert_eq!(img.px, [1.0, 2.0, 3.0, 250.0, 251.0, 252.0]);
    }

    #[test]
    fn load_png_premultiplies_alpha_onto_black() {
        let data = [200, 100, 50, 255, 200, 100, 50, 0, 255, 255, 255, 51];
        let img = load_tmp(write_png("rgba", (3, 1), png::ColorType::Rgba, png::BitDepth::Eight, &data));
        let want = [200.0, 100.0, 50.0, 0.0, 0.0, 0.0, 51.0, 51.0, 51.0];
        assert!(img.px.iter().zip(want).all(|(a, b)| (a - b).abs() < 1e-3), "{:?}", img.px);
    }

    #[test]
    fn load_png_grey_and_grey_alpha() {
        let img = load_tmp(write_png("grey", (2, 1), png::ColorType::Grayscale, png::BitDepth::Eight, &[7, 9]));
        assert_eq!(img.px, [7.0, 7.0, 7.0, 9.0, 9.0, 9.0]);
        let img = load_tmp(write_png("greya", (2, 1), png::ColorType::GrayscaleAlpha, png::BitDepth::Eight, &[200, 255, 200, 0]));
        assert_eq!(img.px, [200.0, 200.0, 200.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn load_png_expands_palettes_and_low_bit_depths() {
        let img = load_tmp(write_png("indexed", (3, 1), png::ColorType::Indexed, png::BitDepth::Eight, &[1, 2, 0]));
        assert_eq!(img.px, [200.0, 100.0, 50.0, 10.0, 20.0, 30.0, 0.0, 0.0, 0.0]);
        // 1-bit grey, 8 pixels in one byte: 1010_0000
        let img = load_tmp(write_png("bits", (8, 1), png::ColorType::Grayscale, png::BitDepth::One, &[0b1010_0000]));
        let grey: Vec<f32> = img.px.iter().step_by(3).copied().collect();
        assert_eq!(grey, [255.0, 0.0, 255.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn load_png_strips_16_bit_samples() {
        let img = load_tmp(write_png("deep", (1, 1), png::ColorType::Rgb, png::BitDepth::Sixteen, &[0x12, 0x34, 0xAB, 0xCD, 0xFF, 0x00]));
        assert_eq!(img.px, [18.0, 171.0, 255.0]);
    }

    #[test]
    fn load_png_reports_the_path_on_errors() {
        let Err(err) = Img::load_png(Path::new("definitely/not/here.png")) else { panic!("a missing file loaded") };
        assert!(err.contains("here.png"), "{err}");
        let bogus = std::env::temp_dir().join(format!("hd-scanner-img-{}-bogus.png", std::process::id()));
        std::fs::write(&bogus, b"not a png at all").unwrap();
        let result = Img::load_png(&bogus);
        let _ = std::fs::remove_file(&bogus);
        let Err(err) = result else { panic!("a text file loaded as a PNG") };
        assert!(err.contains("bogus.png"), "{err}");
    }

    #[test]
    fn decode_png_reads_bytes_as_load_png_reads_files() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../data/icons_144/Reinforce_Stratagem_Icon_Background.png");
        let (file, bytes) = (Img::load_png(&path).unwrap(), Img::decode_png(&std::fs::read(&path).unwrap()).unwrap());
        assert_eq!((file.w, file.h, &file.px), (bytes.w, bytes.h, &bytes.px));
        assert!(Img::decode_png(b"not a png at all").is_err());
        assert!(Img::decode_png(&[]).is_err());
    }

    #[test]
    fn load_png_reads_the_data_files() {
        let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("../data");
        let icon = Img::load_png(&data.join("icons_144/Reinforce_Stratagem_Icon_Background.png")).unwrap();
        assert_eq!((icon.w, icon.h), (144, 144));
        assert!(icon.px.iter().all(|v| (0.0..=255.0).contains(v)));
        let arrow = Img::load_png(&data.join("status/arrow_up.png")).unwrap();
        assert_eq!((arrow.w, arrow.h), (26, 25));
        assert!(arrow.px.chunks(3).all(|p| p[0] == p[1] && p[1] == p[2]), "greyscale template loads as equal channels");
    }
}
