//! Vyrobí ikony aplikace z `assets/icon/timetrek.svg`:
//! timetrek.png (1024 px), timetrek-128.png, timetrek-32.png a timetrek.ico (16-256 px).
//!
//!   cargo run --manifest-path tools/make-icons/Cargo.toml

use image::codecs::ico::{IcoEncoder, IcoFrame};
use image::ExtendedColorType;
use resvg::{tiny_skia, usvg};
use std::path::Path;

fn render(tree: &usvg::Tree, size: u32) -> tiny_skia::Pixmap {
    let mut pixmap = tiny_skia::Pixmap::new(size, size).expect("pixmap");
    let scale = size as f32 / tree.size().width();
    resvg::render(tree, tiny_skia::Transform::from_scale(scale, scale), &mut pixmap.as_mut());
    pixmap
}

fn main() {
    let dir = Path::new("assets/icon");
    let svg = std::fs::read(dir.join("timetrek.svg")).expect("assets/icon/timetrek.svg (spouštěj z kořene repozitáře)");
    let tree = usvg::Tree::from_data(&svg, &usvg::Options::default()).expect("neplatné SVG");

    for (name, size) in [("timetrek.png", 1024), ("timetrek-128.png", 128), ("timetrek-32.png", 32)] {
        render(&tree, size).save_png(dir.join(name)).expect("zápis PNG");
        println!("{name} ({size} px)");
    }

    let frames: Vec<IcoFrame> = [16u32, 24, 32, 48, 64, 128, 256]
        .iter()
        .map(|&s| {
            let p = render(&tree, s);
            // tiny-skia má předem vynásobenou průhlednost, ICO chce obyčejnou.
            let rgba: Vec<u8> = p.pixels().iter().flat_map(|px| {
                let c = px.demultiply();
                [c.red(), c.green(), c.blue(), c.alpha()]
            }).collect();
            IcoFrame::as_png(&rgba, s, s, ExtendedColorType::Rgba8).expect("ICO snímek")
        })
        .collect();
    let file = std::fs::File::create(dir.join("timetrek.ico")).expect("zápis ICO");
    IcoEncoder::new(file).encode_images(&frames).expect("kódování ICO");
    println!("timetrek.ico");
}
