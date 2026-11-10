use ab_glyph::{Font, FontArc};
use std::sync::OnceLock;

pub fn font_regular() -> Option<&'static FontArc> {
    static F: OnceLock<Option<FontArc>> = OnceLock::new();
    F.get_or_init(|| load_font(&["segoeui.ttf", "arial.ttf"])).as_ref()
}

pub fn font_bold() -> Option<&'static FontArc> {
    static F: OnceLock<Option<FontArc>> = OnceLock::new();
    F.get_or_init(|| load_font(&["segoeuib.ttf", "arialbd.ttf", "segoeui.ttf"])).as_ref()
}

/// Fonts consulted, in order, for characters the primary font lacks
/// (symbols, emoji outlines, CJK, Korean).
fn fallback_fonts() -> &'static [FontArc] {
    static F: OnceLock<Vec<FontArc>> = OnceLock::new();
    F.get_or_init(|| {
        ["seguisym.ttf", "seguiemj.ttf", "msyh.ttc", "YuGothM.ttc", "malgun.ttf", "Nirmala.ttc"]
            .iter()
            .filter_map(|n| load_font(&[n]))
            .collect()
    })
}

/// The font that should draw `ch`: the primary one if it has the glyph.
pub(super) fn font_for(primary: &'static FontArc, ch: char) -> &'static FontArc {
    if ch.is_whitespace() || primary.glyph_id(ch).0 != 0 {
        return primary;
    }
    fallback_fonts().iter().find(|f| f.glyph_id(ch).0 != 0).unwrap_or(primary)
}

fn load_font(names: &[&str]) -> Option<FontArc> {
    let dir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into());
    names.iter().find_map(|n| {
        let data = std::fs::read(format!("{dir}\\Fonts\\{n}")).ok()?;
        if n.ends_with(".ttc") {
            ab_glyph::FontVec::try_from_vec_and_index(data, 0).ok().map(FontArc::new)
        } else {
            FontArc::try_from_vec(data).ok()
        }
    })
}
