//! Static image encoding; callers own destinations and publication.

use crate::render::{FRAME_HEIGHT_PIXELS, FRAME_WIDTH_PIXELS, KoiFrame};

pub const STATIC_EXPORT_SCALE: u32 = 2;
pub const STATIC_WIDTH_PIXELS: u32 = FRAME_WIDTH_PIXELS * STATIC_EXPORT_SCALE;
pub const STATIC_HEIGHT_PIXELS: u32 = FRAME_HEIGHT_PIXELS * STATIC_EXPORT_SCALE;

/// Encode a static PNG by repeating each native pixel in both dimensions.
/// Preserve its color and alpha without interpolation, timestamps, I/O, or mutation.
/// Encoding errors propagate before the caller publishes any resulting bytes.
pub fn encode_png(koi_frame: &KoiFrame) -> Result<Vec<u8>, png::EncodingError> {
    let native_pixels = koi_frame.as_rgba_bytes().as_chunks::<4>().0;
    let mut export_pixels = vec![[0; 4]; (STATIC_WIDTH_PIXELS * STATIC_HEIGHT_PIXELS) as usize];
    for (pixel_index, pixel_channels) in export_pixels.iter_mut().enumerate() {
        let native_x = pixel_index as u32 % STATIC_WIDTH_PIXELS / STATIC_EXPORT_SCALE;
        let native_y = pixel_index as u32 / STATIC_WIDTH_PIXELS / STATIC_EXPORT_SCALE;
        *pixel_channels = native_pixels[(native_y * FRAME_WIDTH_PIXELS + native_x) as usize];
    }
    let mut png_bytes = Vec::new();
    let mut png_encoder =
        png::Encoder::new(&mut png_bytes, STATIC_WIDTH_PIXELS, STATIC_HEIGHT_PIXELS);
    png_encoder.set_color(png::ColorType::Rgba);
    png_encoder.set_depth(png::BitDepth::Eight);
    let mut png_writer = png_encoder.write_header()?;
    png_writer.write_image_data(export_pixels.as_flattened())?;
    png_writer.finish()?;
    Ok(png_bytes)
}

/// Encode a self-contained SVG at the static export size with native pixel coordinates.
/// Equal neighboring pixels share a rectangle within their row. Transparent pixels
/// are omitted; visible pixels retain their color and alpha. Identical frames produce
/// identical documents without I/O, timestamps, external assets, or input mutation.
pub fn encode_svg(koi_frame: &KoiFrame) -> String {
    let mut svg_document = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{STATIC_WIDTH_PIXELS}\" height=\"{STATIC_HEIGHT_PIXELS}\" viewBox=\"0 0 {FRAME_WIDTH_PIXELS} {FRAME_HEIGHT_PIXELS}\" shape-rendering=\"crispEdges\">\n"
    );
    let native_pixels = koi_frame.as_rgba_bytes().as_chunks::<4>().0;
    let mut pixel_index = 0;
    while pixel_index < native_pixels.len() {
        let pixel_channels = native_pixels[pixel_index];
        let pixel_x = pixel_index % FRAME_WIDTH_PIXELS as usize;
        let pixel_y = pixel_index / FRAME_WIDTH_PIXELS as usize;
        let row_end_index = (pixel_y + 1) * FRAME_WIDTH_PIXELS as usize;
        let run_length_pixels = native_pixels[pixel_index..row_end_index]
            .iter()
            .take_while(|next_pixel| **next_pixel == pixel_channels)
            .count();
        if pixel_channels[3] != 0 {
            let [red_channel, green_channel, blue_channel, alpha_channel] = pixel_channels;
            let fill_opacity = f64::from(alpha_channel) / 255.0;
            svg_document.push_str(&format!(
                "  <rect x=\"{pixel_x}\" y=\"{pixel_y}\" width=\"{run_length_pixels}\" height=\"1\" fill=\"#{red_channel:02x}{green_channel:02x}{blue_channel:02x}\" fill-opacity=\"{fill_opacity}\"/>\n"
            ));
        }
        pixel_index += run_length_pixels;
    }
    svg_document.push_str("</svg>\n");
    svg_document
}
