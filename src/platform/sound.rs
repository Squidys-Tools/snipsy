use windows::core::PCWSTR;

static SHUTTER: std::sync::OnceLock<Vec<u8>> = std::sync::OnceLock::new();

/// Synthesised camera-shutter click (two short filtered noise bursts).
fn shutter_wav() -> &'static [u8] {
    SHUTTER.get_or_init(|| {
        let rate = 44100u32;
        let n = (rate as f32 * 0.16) as usize;
        let mut seed = 0x1234_5678u32;
        let mut samples = Vec::with_capacity(n);
        let mut lp = 0.0f32;
        for i in 0..n {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            let noise = (seed as f32 / u32::MAX as f32) * 2.0 - 1.0;
            let t = i as f32 / rate as f32;
            let env1 = if t < 0.05 { (-t * 90.0).exp() } else { 0.0 };
            let t2 = t - 0.07;
            let env2 = if t2 > 0.0 { (-t2 * 70.0).exp() * 0.8 } else { 0.0 };
            lp += (noise - lp) * 0.35;
            samples.push(((lp * (env1 + env2)) * 0.6 * i16::MAX as f32) as i16);
        }
        let data_len = (samples.len() * 2) as u32;
        let mut w = Vec::with_capacity(44 + data_len as usize);
        w.extend_from_slice(b"RIFF");
        w.extend_from_slice(&(36 + data_len).to_le_bytes());
        w.extend_from_slice(b"WAVEfmt ");
        w.extend_from_slice(&16u32.to_le_bytes());
        w.extend_from_slice(&1u16.to_le_bytes());
        w.extend_from_slice(&1u16.to_le_bytes());
        w.extend_from_slice(&rate.to_le_bytes());
        w.extend_from_slice(&(rate * 2).to_le_bytes());
        w.extend_from_slice(&2u16.to_le_bytes());
        w.extend_from_slice(&16u16.to_le_bytes());
        w.extend_from_slice(b"data");
        w.extend_from_slice(&data_len.to_le_bytes());
        for s in samples {
            w.extend_from_slice(&s.to_le_bytes());
        }
        w
    })
}

pub fn play_shutter() {
    use windows::Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_MEMORY, SND_NODEFAULT};
    let wav = shutter_wav();
    unsafe {
        let _ = PlaySoundW(
            PCWSTR(wav.as_ptr() as *const u16),
            None,
            SND_MEMORY | SND_ASYNC | SND_NODEFAULT,
        );
    }
}
