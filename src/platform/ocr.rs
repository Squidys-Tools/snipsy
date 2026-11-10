use anyhow::{anyhow, Result};
use image::RgbaImage;

/// Must be called off the UI thread; blocks until done.
pub fn recognize_text(img: &RgbaImage) -> Result<String> {
    use windows::Graphics::Imaging::{BitmapAlphaMode, BitmapPixelFormat, SoftwareBitmap};
    use windows::Media::Ocr::OcrEngine;
    use windows::Storage::Streams::DataWriter;
    use windows::Win32::System::WinRT::{RoInitialize, RO_INIT_MULTITHREADED};

    unsafe {
        let _ = RoInitialize(RO_INIT_MULTITHREADED);
    }
    // Upscale small captures: the engine is much more accurate on larger glyphs.
    let max_dim = OcrEngine::MaxImageDimension().unwrap_or(10000);
    let mut work = img.clone();
    if work.height() < 400 && work.width() * 3 < max_dim && work.height() * 3 < max_dim {
        work = image::imageops::resize(
            &work,
            work.width() * 3,
            work.height() * 3,
            image::imageops::FilterType::CatmullRom,
        );
    } else if work.height() < 900 && work.width() * 2 < max_dim && work.height() * 2 < max_dim {
        work = image::imageops::resize(
            &work,
            work.width() * 2,
            work.height() * 2,
            image::imageops::FilterType::CatmullRom,
        );
    }
    if work.width() > max_dim || work.height() > max_dim {
        let s = max_dim as f32 / work.width().max(work.height()) as f32;
        work = image::imageops::resize(
            &work,
            (work.width() as f32 * s) as u32,
            (work.height() as f32 * s) as u32,
            image::imageops::FilterType::Triangle,
        );
    }
    let (ww, wh) = (work.width() as i32, work.height() as i32);
    let mut bgra = work.into_raw();
    for p in bgra.chunks_exact_mut(4) {
        p.swap(0, 2);
    }
    let writer = DataWriter::new()?;
    writer.WriteBytes(&bgra)?;
    let buffer = writer.DetachBuffer()?;
    let bitmap = SoftwareBitmap::CreateCopyWithAlphaFromBuffer(
        &buffer,
        BitmapPixelFormat::Bgra8,
        ww,
        wh,
        BitmapAlphaMode::Premultiplied,
    )?;
    let engine = OcrEngine::TryCreateFromUserProfileLanguages()
        .map_err(|e| anyhow!("No OCR language pack installed: {e}"))?;
    let result = engine.RecognizeAsync(&bitmap)?.get()?;
    let mut out = String::new();
    for line in result.Lines()? {
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(&line.Text()?.to_string_lossy());
    }
    Ok(out)
}
