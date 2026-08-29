//! Renders "下载默认设置" with Microsoft YaHei UI bold under different Skia
//! hinting modes to diagnose the blurry/jagged bold-text rendering.

#![allow(clippy::needless_borrows_for_generic_args)]
use skia_safe::{surfaces, Color, Font, FontHinting, FontStyle, FontMgr, Paint};

#[test]
fn hinting_comparison_render() {
    let mgr = FontMgr::new();
    let bold = mgr
        .match_family_style("Microsoft YaHei UI", FontStyle::bold())
        .expect("Microsoft YaHei UI bold face must exist");
    let regular = mgr
        .match_family_style("Microsoft YaHei UI", FontStyle::normal())
        .expect("Microsoft YaHei UI regular face must exist");

    println!("bold typeface family: {:?}", bold.family_name());
    println!("regular typeface family: {:?}", regular.family_name());

    let mut surface = surfaces::raster_n32_premul((760, 320)).expect("surface");
    let canvas = surface.canvas();
    canvas.clear(Color::from_rgb(0xf8, 0xf9, 0xfa));

    let mut paint = Paint::default();
    paint.set_color(Color::from_rgb(0x84, 0xcc, 0x16));

    // Row 1-4: bold face under the four hinting modes
    let modes = [
        ("Normal", FontHinting::Normal),
        ("Slight", FontHinting::Slight),
        ("No", FontHinting::None),
        ("Full", FontHinting::Full),
    ];
    for (i, (name, mode)) in modes.iter().enumerate() {
        let mut font = Font::from_typeface(bold.clone(), 23.0);
        font.set_subpixel(true);
        font.set_hinting(*mode);
        canvas.draw_str(
            &format!("下载默认设置  bold {name}"),
            (10.0, 40.0 + 62.0 * i as f32),
            &font,
            &paint,
        );
    }

    // Row 5: regular face, default hinting (the "clear" reference)
    let mut font = Font::from_typeface(regular, 23.0);
    font.set_subpixel(true);
    canvas.draw_str(
        "下载默认设置  regular",
        (10.0, 40.0 + 62.0 * 4.0),
        &font,
        &paint,
    );


    // ── Part 2: reproduce the app's conditions (scaled canvas + logical 13px) ──
    let mut surface2 = surfaces::raster_n32_premul((760, 320)).expect("surface2");
    let canvas2 = surface2.canvas();
    canvas2.clear(Color::from_rgb(0xf8, 0xf9, 0xfa));

    let mut paint2 = Paint::default();
    paint2.set_anti_alias(true);
    paint2.set_color(Color::from_rgb(0x84, 0xcc, 0x16));

    let combos = [
        ("scale1.75 13px Normal", 1.75f32, 13.0f32, FontHinting::Normal),
        ("scale1.75 13px Slight", 1.75, 13.0, FontHinting::Slight),
        ("scale1.75 13px No     ", 1.75, 13.0, FontHinting::None),
        ("scale1.75 23px Normal", 1.75, 23.0, FontHinting::Normal),
        ("scale1.0  23px Normal", 1.0, 23.0, FontHinting::Normal),
    ];
    for (i, (name, scale, size, mode)) in combos.iter().enumerate() {
        canvas2.save();
        canvas2.scale((*scale, *scale));
        let mut font = Font::from_typeface(bold.clone(), *size);
        font.set_subpixel(true);
        font.set_hinting(*mode);
        let y = (40.0 + 62.0 * i as f32) / scale;
        canvas2.draw_str(
            &format!("下载默认设置  {name}"),
            (10.0, y),
            &font,
            &paint2,
        );
        canvas2.restore();
    }

    let image2 = surface2.image_snapshot();
    #[allow(deprecated)]
    let data2 = image2.encode_to_data(skia_safe::EncodedImageFormat::PNG).expect("png2");
    let out2 = std::env::temp_dir().join("skia-scaled-comparison.png");
    std::fs::write(&out2, data2.as_bytes()).expect("write png2");
    println!("written: {}", out2.display());
    let image = surface.image_snapshot();
        #[allow(deprecated)]
    #[allow(deprecated)]
    let data = image.encode_to_data(skia_safe::EncodedImageFormat::PNG).expect("png encode");
    let out = std::env::temp_dir().join("skia-hinting-comparison.png");
    std::fs::write(&out, data.as_bytes()).expect("write png");
    println!("written: {}", out.display());
}
