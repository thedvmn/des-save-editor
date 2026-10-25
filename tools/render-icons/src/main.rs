//! Renders assets/icon.svg into the PNG and ICO files used for packaging.
//!
//! Usage (from the repository root):
//!   cargo run --manifest-path tools/render-icons/Cargo.toml -- assets [preview-dir svg...]

use std::fs;
use std::path::Path;

use resvg::{tiny_skia, usvg};

fn render(svg: &Path, width: u32) -> tiny_skia::Pixmap {
    let data = fs::read(svg).unwrap_or_else(|e| panic!("{}: {e}", svg.display()));
    let tree = usvg::Tree::from_data(&data, &usvg::Options::default()).expect("invalid SVG");
    let size = tree.size();
    let scale = width as f32 / size.width();
    let height = (size.height() * scale).round() as u32;
    let mut pixmap = tiny_skia::Pixmap::new(width, height).unwrap();
    resvg::render(&tree, tiny_skia::Transform::from_scale(scale, scale), &mut pixmap.as_mut());
    pixmap
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let assets = Path::new(args.first().map_or("assets", String::as_str));
    let icon_svg = assets.join("icon.svg");
    let out = assets.join("icons");
    fs::create_dir_all(&out).unwrap();

    let mut ico = ico::IconDir::new(ico::ResourceType::Icon);
    for size in [16, 24, 32, 48, 64, 128, 256] {
        let pixmap = render(&icon_svg, size);
        pixmap.save_png(out.join(format!("icon-{size}.png"))).unwrap();
        let image = ico::IconImage::from_rgba_data(size, size, pixmap.data().to_vec());
        ico.add_entry(ico::IconDirEntry::encode(&image).unwrap());
    }
    ico.write(fs::File::create(assets.join("icon.ico")).unwrap()).unwrap();
    println!("wrote {} and assets/icon.ico", out.display());

    // Optional previews: <preview-dir> <svg>...
    if let Some(dir) = args.get(1) {
        fs::create_dir_all(dir).unwrap();
        for svg in &args[2..] {
            let p = Path::new(svg);
            render(p, 640).save_png(Path::new(dir).join(p.file_stem().unwrap()).with_extension("png")).unwrap();
        }
    }
}
