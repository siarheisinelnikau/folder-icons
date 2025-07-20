fn main() -> anyhow::Result<()> {
    use image::{
        ImageEncoder,
        codecs::{
            ico::{IcoEncoder, IcoFrame},
            png::PngEncoder,
        },
    };
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
    std::fs::create_dir_all(&root)?;
    let artwork = folder_icons::render::render(&Default::default(), None);
    artwork.save(root.join("icon-256.png"))?;
    let mut frames = Vec::new();
    for size in [16, 20, 24, 32, 40, 48, 64, 128, 256] {
        let image =
            image::imageops::resize(&artwork, size, size, image::imageops::FilterType::Lanczos3);
        let mut png = Vec::new();
        PngEncoder::new(&mut png).write_image(
            image.as_raw(),
            size,
            size,
            image::ExtendedColorType::Rgba8,
        )?;
        frames.push(IcoFrame::with_encoded(
            png,
            size,
            size,
            image::ExtendedColorType::Rgba8,
        )?);
    }
    IcoEncoder::new(std::fs::File::create(root.join("FolderIcons.ico"))?).encode_images(&frames)?;
    Ok(())
}
