//! Original, procedural artwork; no external theme assets are required.
use anyhow::{Result, ensure};
use image::{
    ImageEncoder, Rgba, RgbaImage,
    codecs::{
        ico::{IcoEncoder, IcoFrame},
        png::PngEncoder,
    },
};
use serde::{Deserialize, Serialize};
use std::{io::Cursor, path::Path};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Badge {
    None,
    Star,
    Check,
    Heart,
    Work,
    Alert,
}
impl Badge {
    pub const ALL: [Self; 6] = [
        Self::None,
        Self::Star,
        Self::Check,
        Self::Heart,
        Self::Work,
        Self::Alert,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Star => "Star",
            Self::Check => "Done",
            Self::Heart => "Favorite",
            Self::Work => "Work",
            Self::Alert => "Priority",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Style {
    pub color: [u8; 3],
    pub badge: Badge,
}
impl Default for Style {
    fn default() -> Self {
        Self {
            color: [57, 139, 228],
            badge: Badge::None,
        }
    }
}
pub const THEMES: [(&str, [u8; 3]); 6] = [
    ("Ocean", [57, 139, 228]),
    ("Meadow", [48, 166, 119]),
    ("Amber", [231, 162, 49]),
    ("Coral", [222, 101, 105]),
    ("Iris", [143, 107, 207]),
    ("Slate", [104, 125, 147]),
];

pub fn load_photo(path: &Path) -> Result<RgbaImage> {
    ensure!(
        std::fs::metadata(path)?.len() <= 25 * 1024 * 1024,
        "Photo exceeds 25 MB"
    );
    let mut reader = image::ImageReader::open(path)?.with_guessed_format()?;
    ensure!(
        matches!(
            reader.format(),
            Some(image::ImageFormat::Png | image::ImageFormat::Jpeg)
        ),
        "Choose a PNG or JPEG photo"
    );
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(12000);
    limits.max_image_height = Some(12000);
    limits.max_alloc = Some(128 * 1024 * 1024);
    reader.limits(limits);
    Ok(reader
        .decode()?
        .resize_to_fill(208, 132, image::imageops::FilterType::Lanczos3)
        .to_rgba8())
}

fn inside_round(x: f32, y: f32, rect: [f32; 4], r: f32) -> bool {
    let [l, t, rr, b] = rect;
    if x < l || x > rr || y < t || y > b {
        return false;
    }
    let cx = x.clamp(l + r, rr - r);
    let cy = y.clamp(t + r, b - r);
    (x - cx).powi(2) + (y - cy).powi(2) <= r * r
}
fn line(x: f32, y: f32, a: [f32; 2], b: [f32; 2], width: f32) -> bool {
    let dx = b[0] - a[0];
    let dy = b[1] - a[1];
    let t = (((x - a[0]) * dx + (y - a[1]) * dy) / (dx * dx + dy * dy)).clamp(0., 1.);
    (x - a[0] - t * dx).powi(2) + (y - a[1] - t * dy).powi(2) < width * width
}
fn symbol(badge: Badge, x: f32, y: f32) -> bool {
    match badge {
        Badge::None => false,
        Badge::Check => {
            line(x, y, [-12., 0.], [-3., 9.], 3.) || line(x, y, [-3., 9.], [14., -11.], 3.)
        }
        Badge::Alert => {
            inside_round(x, y, [-3., -15., 3., 5.], 2.) || x * x + (y - 13.).powi(2) < 10.
        }
        Badge::Work => {
            inside_round(x, y, [-16., -8., 16., 14.], 3.)
                || (inside_round(x, y, [-8., -16., 8., -5.], 2.)
                    && !inside_round(x, y, [-3., -12., 3., -4.], 1.))
        }
        Badge::Heart => {
            let yy = (y + 3.) / 16.;
            let xx = x / 16.;
            (xx * xx + yy * yy - 1.).powi(3) - xx * xx * (-yy).powi(3) < 0.
        }
        Badge::Star => {
            let angle = x.atan2(-y);
            let radius = (x * x + y * y).sqrt();
            let theta = (angle + std::f32::consts::PI / 5.).rem_euclid(std::f32::consts::TAU / 5.)
                - std::f32::consts::PI / 5.;
            radius < 10. / (theta.abs() + 0.35)
        }
    }
}

pub fn render(style: &Style, photo: Option<&RgbaImage>) -> RgbaImage {
    let mut img = RgbaImage::new(256, 256);
    let c = style.color;
    for (x, y, p) in img.enumerate_pixels_mut() {
        let x = x as f32;
        let y = y as f32;
        let mut rgba = [0, 0, 0, 0];
        if inside_round(x, y, [18., 64., 240., 225.], 14.) {
            rgba = [11, 23, 39, 35];
        }
        if inside_round(x, y, [16., 45., 114., 102.], 10.)
            || inside_round(x, y, [16., 65., 240., 214.], 12.)
        {
            rgba = [
                (c[0] as f32 * 0.72) as u8,
                (c[1] as f32 * 0.72) as u8,
                (c[2] as f32 * 0.72) as u8,
                255,
            ];
        }
        if inside_round(x, y, [16., 82., 240., 217.], 12.) {
            let light = (1. - (y - 82.) / 135.) * 0.17;
            rgba = [0, 0, 0, 255];
            for i in 0..3 {
                rgba[i] = (c[i] as f32 + (255. - c[i] as f32) * light) as u8;
            }
            if let Some(photo) = photo
                && inside_round(x, y, [26., 92., 230., 207.], 7.)
            {
                let px = ((x - 26.) / 204. * (photo.width() - 1) as f32) as u32;
                let py = ((y - 92.) / 115. * (photo.height() - 1) as f32) as u32;
                let sample = photo.get_pixel(px, py).0;
                for i in 0..3 {
                    rgba[i] = ((sample[i] as u32 * sample[3] as u32
                        + rgba[i] as u32 * (255 - sample[3] as u32))
                        / 255) as u8;
                }
            }
        }
        if style.badge != Badge::None {
            let bx = x - 205.;
            let by = y - 195.;
            if bx * bx + by * by < 35. * 35. {
                rgba = [255, 255, 255, 255];
            }
            if bx * bx + by * by < 30. * 30. {
                rgba = [29, 42, 62, 255];
            }
            if symbol(style.badge, bx, by) {
                rgba = [255, 255, 255, 255];
            }
        }
        *p = Rgba(rgba);
    }
    img
}

pub fn ico(image: &RgbaImage) -> Result<Vec<u8>> {
    let mut pngs = Vec::new();
    for size in [16, 24, 32, 48, 64, 128, 256] {
        let frame =
            image::imageops::resize(image, size, size, image::imageops::FilterType::Lanczos3);
        let mut bytes = Vec::new();
        PngEncoder::new(&mut bytes).write_image(
            frame.as_raw(),
            size,
            size,
            image::ExtendedColorType::Rgba8,
        )?;
        pngs.push((bytes, size));
    }
    let frames: Vec<_> = pngs
        .iter()
        .map(|(data, size)| {
            IcoFrame::with_encoded(data, *size, *size, image::ExtendedColorType::Rgba8)
        })
        .collect::<std::result::Result<_, _>>()?;
    let mut out = Cursor::new(Vec::new());
    IcoEncoder::new(&mut out).encode_images(&frames)?;
    Ok(out.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn valid_multisize_ico_and_original_artwork() {
        let plain = render(&Style::default(), None);
        assert_eq!(plain.get_pixel(0, 0)[3], 0);
        for badge in Badge::ALL {
            let img = render(
                &Style {
                    badge,
                    ..Style::default()
                },
                None,
            );
            let bytes = ico(&img).unwrap();
            assert_eq!(&bytes[..6], &[0, 0, 1, 0, 7, 0]);
            assert_eq!(
                image::load_from_memory_with_format(&bytes, image::ImageFormat::Ico)
                    .unwrap()
                    .width(),
                256
            );
            if badge != Badge::None {
                assert_ne!(img, plain);
            }
        }
    }
    #[test]
    fn photo_is_embedded_and_alpha_is_composited() {
        let photo = RgbaImage::from_pixel(208, 132, Rgba([255, 0, 0, 255]));
        let img = render(&Style::default(), Some(&photo));
        assert_eq!(img.get_pixel(100, 120).0, [255, 0, 0, 255]);
    }
}
