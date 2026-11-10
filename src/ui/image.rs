use gpui::{rgb, Hsla, RenderImage, Rgba};

pub fn hsla_of(c: [u8; 4]) -> Hsla {
    Rgba { r: c[0] as f32 / 255.0, g: c[1] as f32 / 255.0, b: c[2] as f32 / 255.0, a: c[3] as f32 / 255.0 }.into()
}

pub fn hex_hsla(c: u32) -> Hsla {
    rgb(c).into()
}

/// Convert an RGBA image into a GPUI render image (BGRA).
pub fn to_render_image(img: &image::RgbaImage) -> std::sync::Arc<RenderImage> {
    let mut data = img.as_raw().clone();
    for p in data.chunks_exact_mut(4) {
        p.swap(0, 2);
    }
    let buf = image::RgbaImage::from_raw(img.width(), img.height(), data).unwrap();
    std::sync::Arc::new(RenderImage::new(smallvec::smallvec![image::Frame::new(buf)]))
}
