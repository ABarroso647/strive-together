// Image generation modules - one per tracker type
pub mod gym;

use resvg::tiny_skia::{Pixmap, Transform};
use resvg::usvg::{Options, Tree};

/// Render an SVG string to PNG bytes (shared utility)
pub fn render_svg_to_png(svg: &str, width: u32, height: u32) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
    let mut options = Options::default();
    options.fontdb_mut().load_system_fonts();
    let tree = Tree::from_str(svg, &options)?;

    // Create pixmap
    let mut pixmap = Pixmap::new(width, height)
        .ok_or("Failed to create pixmap")?;

    // Fill with background color (dark theme)
    pixmap.fill(resvg::tiny_skia::Color::from_rgba8(47, 49, 54, 255));

    // Render
    resvg::render(&tree, Transform::default(), &mut pixmap.as_mut());

    // Encode to PNG
    let png_data = pixmap.encode_png()?;
    Ok(png_data)
}

/// Escape text for SVG (handle special characters) - shared utility
pub fn escape_svg(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
