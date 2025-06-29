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
    Document,
    Photo,
    Music,
    Video,
    Code,
    Game,
    Download,
    Archive,
    Finance,
    Study,
    Travel,
    Lock,
}
impl Badge {
    pub const ALL: [Self; 18] = [
        Self::None,
        Self::Star,
        Self::Check,
        Self::Heart,
        Self::Work,
        Self::Alert,
        Self::Document,
        Self::Photo,
        Self::Music,
        Self::Video,
        Self::Code,
        Self::Game,
        Self::Download,
        Self::Archive,
        Self::Finance,
        Self::Study,
        Self::Travel,
        Self::Lock,
    ];
    pub const GROUPS: [(&'static str, &'static [Self]); 3] = [
        (
            "Content",
            &[
                Self::Document,
                Self::Photo,
                Self::Music,
                Self::Video,
                Self::Code,
                Self::Game,
            ],
        ),
        (
            "Organization",
            &[
                Self::Work,
                Self::Download,
                Self::Archive,
                Self::Finance,
                Self::Study,
                Self::Travel,
                Self::Lock,
            ],
        ),
        (
            "Status",
            &[Self::Star, Self::Check, Self::Heart, Self::Alert],
        ),
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Star => "Star",
            Self::Check => "Done",
            Self::Heart => "Favorite",
            Self::Work => "Work",
            Self::Alert => "Priority",
            Self::Document => "Documents",
            Self::Photo => "Photos",
            Self::Music => "Music",
            Self::Video => "Videos",
            Self::Code => "Code",
            Self::Game => "Games",
            Self::Download => "Downloads",
            Self::Archive => "Archive",
            Self::Finance => "Finance",
            Self::Study => "Study",
            Self::Travel => "Travel",
            Self::Lock => "Lock",
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
    let stroke = |a, b| line(x, y, a, b, 2.2);
    let ring = |r: f32| {
        let d = x * x + y * y;
        d < r * r && d > (r - 2.4).powi(2)
    };
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
        Badge::Document => {
            outline(x, y, [-13., -18., 13., 18.], 2., 2.2)
                || stroke([-7., -5.], [7., -5.])
                || stroke([-7., 2.], [7., 2.])
                || stroke([-7., 9.], [3., 9.])
        }
        Badge::Photo => {
            outline(x, y, [-18., -13., 18., 13.], 2., 2.2)
                || (x + 8.).powi(2) + (y + 5.).powi(2) < 9.
                || polygon(
                    x,
                    y,
                    &[[-14., 9.], [-3., -3.], [3., 4.], [9., -2.], [15., 9.]],
                )
        }
        Badge::Music => {
            stroke([0., -15.], [0., 9.])
                || stroke([0., -15.], [13., -10.])
                || ((x + 5.) / 6.).powi(2) + ((y - 10.) / 4.).powi(2) < 1.
        }
        Badge::Video => {
            outline(x, y, [-18., -13., 18., 13.], 3., 2.2)
                || polygon(x, y, &[[-5., -8.], [-5., 8.], [9., 0.]])
        }
        Badge::Code => {
            stroke([-7., -11.], [-17., 0.])
                || stroke([-17., 0.], [-7., 11.])
                || stroke([7., -11.], [17., 0.])
                || stroke([17., 0.], [7., 11.])
                || stroke([3., -13.], [-3., 13.])
        }
        Badge::Game => {
            outline(x, y, [-19., -10., 19., 13.], 6., 2.2)
                || stroke([-14., 1.], [-4., 1.])
                || stroke([-9., -4.], [-9., 6.])
                || (x - 9.).powi(2) + (y + 3.).powi(2) < 5.
                || (x - 13.).powi(2) + (y - 3.).powi(2) < 5.
        }
        Badge::Download => {
            stroke([0., -16.], [0., 5.])
                || stroke([-8., -3.], [0., 5.])
                || stroke([0., 5.], [8., -3.])
                || stroke([-16., 8.], [-16., 16.])
                || stroke([-16., 16.], [16., 16.])
                || stroke([16., 16.], [16., 8.])
        }
        Badge::Archive => {
            outline(x, y, [-15., -10., 15., 17.], 2., 2.2)
                || outline(x, y, [-18., -17., 18., -8.], 2., 2.2)
                || stroke([-5., -1.], [5., -1.])
        }
        Badge::Finance => {
            ring(19.)
                || stroke([-6., -8.], [7., -8.])
                || stroke([-6., -8.], [-6., 0.])
                || stroke([-6., 0.], [6., 0.])
                || stroke([6., 0.], [6., 8.])
                || stroke([6., 8.], [-7., 8.])
                || stroke([0., -13.], [0., 13.])
        }
        Badge::Study => {
            stroke([-17., -15.], [0., -10.])
                || stroke([0., -10.], [17., -15.])
                || stroke([-17., -15.], [-17., 12.])
                || stroke([17., -15.], [17., 12.])
                || stroke([-17., 12.], [0., 17.])
                || stroke([0., 17.], [17., 12.])
                || stroke([0., -10.], [0., 17.])
                || stroke([-12., -6.], [-5., -4.])
                || stroke([5., -4.], [12., -6.])
        }
        Badge::Travel => polygon(
            x,
            y,
            &[
                [0., -20.],
                [4., -4.],
                [18., 4.],
                [18., 8.],
                [4., 3.],
                [4., 13.],
                [9., 17.],
                [9., 20.],
                [0., 16.],
                [-9., 20.],
                [-9., 17.],
                [-4., 13.],
                [-4., 3.],
                [-18., 8.],
                [-18., 4.],
                [-4., -4.],
            ],
        ),
        Badge::Lock => {
            let body = inside_round(x, y, [-13., -1., 13., 17.], 3.)
                && !(x * x + (y - 6.).powi(2) < 7. || line(x, y, [0., 8.], [0., 12.], 1.4));
            body || (y < 1. && outline(x, y, [-10., -18., 10., 8.], 9., 3.))
        }
    }
}

fn outline(x: f32, y: f32, r: [f32; 4], radius: f32, width: f32) -> bool {
    inside_round(x, y, r, radius)
        && !inside_round(
            x,
            y,
            [r[0] + width, r[1] + width, r[2] - width, r[3] - width],
            (radius - width).max(0.5),
        )
}
fn polygon(x: f32, y: f32, points: &[[f32; 2]]) -> bool {
    let mut inside = false;
    let mut previous = points[points.len() - 1];
    for &point in points {
        if (point[1] > y) != (previous[1] > y)
            && x < (previous[0] - point[0]) * (y - point[1]) / (previous[1] - point[1]) + point[0]
        {
            inside = !inside;
        }
        previous = point;
    }
    inside
}
fn badge_pixel(badge: Badge, x: f32, y: f32, step: f32) -> [u8; 4] {
    if badge == Badge::None {
        return [0; 4];
    }
    let mut sums = [0u32; 3];
    let mut covered = 0;
    for dx in [-0.25, 0.25] {
        for dy in [-0.25, 0.25] {
            let (x, y) = (x + dx * step, y + dy * step);
            let distance = x * x + y * y;
            if distance > 35. * 35. {
                continue;
            }
            let rgb = if distance > 30. * 30. || symbol(badge, x, y) {
                [255, 255, 255]
            } else {
                [29, 42, 62]
            };
            for i in 0..3 {
                sums[i] += rgb[i];
            }
            covered += 1;
        }
    }
    let divisor = covered.max(1);
    [
        (sums[0] / divisor) as u8,
        (sums[1] / divisor) as u8,
        (sums[2] / divisor) as u8,
        (covered * 255 / 4) as u8,
    ]
}
pub fn symbol_catalog() -> Vec<(Badge, RgbaImage)> {
    Badge::ALL
        .into_iter()
        .filter(|b| *b != Badge::None)
        .map(|badge| {
            let image = RgbaImage::from_fn(64, 64, |x, y| {
                Rgba(badge_pixel(
                    badge,
                    (x as f32 + 0.5) * 76. / 64. - 38.,
                    (y as f32 + 0.5) * 76. / 64. - 38.,
                    76. / 64.,
                ))
            });
            (badge, image)
        })
        .collect()
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
        if style.badge != Badge::None && (169. ..=241.).contains(&x) && (159. ..=231.).contains(&y)
        {
            let bx = x - 205.;
            let by = y - 195.;
            let source = badge_pixel(style.badge, bx, by, 1.);
            let alpha = source[3] as u32;
            let out_alpha = alpha + rgba[3] as u32 * (255 - alpha) / 255;
            let divisor = out_alpha.max(1);
            for i in 0..3 {
                rgba[i] = ((source[i] as u32 * alpha
                    + rgba[i] as u32 * rgba[3] as u32 * (255 - alpha) / 255)
                    / divisor) as u8;
            }
            rgba[3] = out_alpha as u8;
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
        let mut unique = std::collections::HashSet::new();
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
            assert!(
                unique.insert(crate::digest(img.as_raw())),
                "Duplicate symbol artwork: {badge:?}"
            );
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

    #[test]
    fn bounded_photo_loader_crops_png_and_rejects_oversized_dimensions() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(".test-output");
        std::fs::create_dir_all(&root).unwrap();
        let dir = tempfile::tempdir_in(root).unwrap();
        let file = dir.path().join("cover.png");
        RgbaImage::from_pixel(400, 100, Rgba([10, 20, 30, 255]))
            .save(&file)
            .unwrap();
        let cover = load_photo(&file).unwrap();
        assert_eq!(cover.dimensions(), (208, 132));
        RgbaImage::new(12001, 1).save(&file).unwrap();
        assert!(load_photo(&file).is_err());
    }
}
