//! Finite, isolated client-area captures using the production UI and CPU rasterizer.
use crate::{App, render};
use anyhow::{Context, Result, bail, ensure};
use egui_software_backend::{BufferMutRef, ColorFieldOrder, EguiSoftwareRender};
use image::ImageEncoder;
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    time::Instant,
};

pub fn fixture(ctx: egui::Context, root: Option<&Path>, scenario: &str) -> Result<App> {
    let paths = if scenario == "empty" {
        Vec::new()
    } else {
        let root = root.context("Populated capture requires --fixture-root")?;
        ensure!(root.is_absolute(), "Fixture root must be absolute");
        let mut paths = std::fs::read_dir(root)?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect::<Vec<_>>();
        paths.sort();
        paths.truncate(3);
        ensure!(!paths.is_empty(), "Fixture has no folders");
        paths
    };
    let mut app = App::new(ctx.clone(), paths, None, true);
    app.style.color = render::THEMES[1].1;
    app.style.badge = render::Badge::Star;
    let photo_path = root
        .map(|root| root.join("cover.png"))
        .filter(|p| p.is_file());
    let photo = photo_path
        .as_ref()
        .map(|p| render::load_photo(p))
        .transpose()?;
    let rgba = render::render(&app.style, photo.as_ref());
    app.ico = Some(render::ico(&rgba)?);
    app.texture = Some(ctx.load_texture(
        "folder-preview",
        egui::ColorImage::from_rgba_unmultiplied([256, 256], rgba.as_raw()),
        egui::TextureOptions::LINEAR,
    ));
    app.photo = photo_path;
    app.preview_revision = app.revision;
    app.dirty_since = None;
    if scenario == "review" {
        for (_, plan) in
            crate::engine::batch_plan(&app.paths, crate::Action::Apply, app.ico.as_deref())
        {
            app.plans.push(plan?);
        }
        app.message = "Review the selected folders, then apply when you are ready.".into();
    } else if scenario == "error" {
        app.plan_errors.push("Skipped folder: desktop.ini changed after the plan was created. Review it and build a new plan.".into());
    } else if !app.paths.is_empty() {
        app.message = "Folders selected. Customize their look, then review your changes.".into();
    }
    Ok(app)
}

pub fn run(args: &[OsString]) -> Result<()> {
    let output = PathBuf::from(
        args.first()
            .context("--capture requires an output PNG path")?,
    );
    ensure!(output.is_absolute(), "Capture path must be absolute");
    let mut screen = "main".to_owned();
    let mut scenario = "empty".to_owned();
    let mut root = None;
    let mut width = 1080u32;
    let mut height = 800u32;
    let mut scale = 1.0f32;
    for pair in args[1..].chunks(2) {
        ensure!(pair.len() == 2, "Capture option needs a value");
        let value = pair[1].to_str().context("Invalid capture option")?;
        match pair[0].to_str() {
            Some("--screen") => screen = value.into(),
            Some("--scenario") => scenario = value.into(),
            Some("--fixture-root") => root = Some(PathBuf::from(&pair[1])),
            Some("--width") => width = value.parse()?,
            Some("--height") => height = value.parse()?,
            Some("--scale") => scale = value.parse()?,
            _ => bail!("Unknown capture option"),
        }
    }
    ensure!(
        ["main", "settings"].contains(&screen.as_str()),
        "Unknown screen"
    );
    ensure!(
        ["empty", "populated", "review", "error"].contains(&scenario.as_str()),
        "Unknown scenario"
    );
    ensure!(
        (820..=1600).contains(&width)
            && (620..=1200).contains(&height)
            && (1.0..=2.0).contains(&scale),
        "Capture dimensions or scale out of range"
    );
    let started = Instant::now();
    let ctx = egui::Context::default();
    let mut app = fixture(ctx.clone(), root.as_deref(), &scenario)?;
    app.settings_open = screen == "settings";
    let physical_width = (width as f32 * scale).round() as usize;
    let physical_height = (height as f32 * scale).round() as usize;
    let mut pixels = vec![[0u8; 4]; physical_width * physical_height];
    let mut renderer = EguiSoftwareRender::new(ColorFieldOrder::Rgba);
    for frame in 0..4 {
        ensure!(
            started.elapsed().as_secs() < 8,
            "Capture readiness deadline exceeded"
        );
        let mut input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(width as f32, height as f32),
            )),
            time: Some(frame as f64 * 0.25),
            ..Default::default()
        };
        input
            .viewports
            .get_mut(&egui::ViewportId::ROOT)
            .context("No root viewport")?
            .native_pixels_per_point = Some(scale);
        let output = ctx.run(input, |ctx| app.ui(ctx));
        let primitives = ctx.tessellate(output.shapes, output.pixels_per_point);
        pixels.fill([244, 247, 250, 255]);
        renderer.render(
            &mut BufferMutRef::new(&mut pixels, physical_width, physical_height),
            &primitives,
            &output.textures_delta,
            output.pixels_per_point,
        );
    }
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut png = Vec::new();
    image::codecs::png::PngEncoder::new(&mut png).write_image(
        bytemuck::cast_slice(&pixels),
        physical_width as u32,
        physical_height as u32,
        image::ExtendedColorType::Rgba8,
    )?;
    folder_icons::platform::atomic_write(&output, &png)?;
    let metadata = serde_json::json!({"version":crate::VERSION,"screen":screen,"scenario":scenario,"logical_size":[width,height],"scale":scale,"physical_size":[physical_width,physical_height],"backend":"egui_software_backend CPU; production client UI; no native chrome or presentation","frames":4,"elapsed_ms":started.elapsed().as_millis(),"network":false,"folder_mutations":false});
    folder_icons::platform::atomic_write(
        &output.with_extension("json"),
        &serde_json::to_vec_pretty(&metadata)?,
    )?;
    Ok(())
}
