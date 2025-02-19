use anyhow::Result;
use image::GrayImage;
use log::info;
use resvg::render;
use resvg::tiny_skia::Pixmap;
use resvg::usvg;
use resvg::usvg::{fontdb, Options, Tree};
use std::collections::HashMap;
use std::sync::Arc;
use rust_embed::RustEmbed;

pub type OptionMap = HashMap<String, String>;

#[derive(RustEmbed)]
#[folder = "assets/"]
pub struct Asset;

pub fn svg_to_bitmap(svg_data: &str, width: u32, height: u32) -> Result<Vec<Vec<bool>>> {
    let mut opt = Options::default();
    let mut fontdb = fontdb::Database::new();
    
    // 加载嵌入的字体
    if let Some(font_data) = Asset::get("LXGWWenKaiScreen-Regular.ttf") {
        fontdb.load_font_data(font_data.data.to_vec());
    }
    
    opt.fontdb = Arc::new(fontdb);

    let tree = match Tree::from_str(svg_data, &opt) {
        Ok(tree) => tree,
        Err(e) => {
            info!("Error parsing SVG: {}. Using fallback SVG.", e);
            let fallback_svg = r#"<svg width='768' height='1024' xmlns='http://www.w3.org/2000/svg'><text x='100' y='900' font-family='Noto Sans' font-size='24'>ERROR!</text></svg>"#;
            Tree::from_str(fallback_svg, &opt)?
        }
    };

    let mut pixmap = Pixmap::new(width, height).unwrap();
    render(&tree, usvg::Transform::default(), &mut pixmap.as_mut());

    let bitmap = pixmap
        .pixels()
        .chunks(width as usize)
        .map(|row| row.iter().map(|p| p.alpha() > 128).collect())
        .collect();

    Ok(bitmap)
}

pub fn write_bitmap_to_file(bitmap: &Vec<Vec<bool>>, filename: &str) -> Result<()> {
    let width = bitmap[0].len();
    let height = bitmap.len();
    let mut img = GrayImage::new(width as u32, height as u32);

    for (y, row) in bitmap.iter().enumerate() {
        for (x, &pixel) in row.iter().enumerate() {
            img.put_pixel(
                x as u32,
                y as u32,
                image::Luma([if pixel { 0 } else { 255 }]),
            );
        }
    }

    img.save(filename)?;
    info!("Bitmap saved to {}", filename);
    Ok(())
}

pub fn option_or_env(options: &OptionMap, key: &str, env_key: &str) -> String {
    let option = options.get(key);
    if option.is_some() {
        option.unwrap().to_string()
    } else {
        std::env::var(env_key.to_string()).unwrap().to_string()
    }
}

pub fn option_or_env_fallback(
    options: &OptionMap,
    key: &str,
    env_key: &str,
    fallback: &str,
) -> String {
    let option = options.get(key);
    if option.is_some() {
        option.unwrap().to_string()
    } else {
        std::env::var(env_key.to_string())
            .unwrap_or(fallback.to_string())
            .to_string()
    }
}
