use crate::annotate::doc::*;
use gpui::prelude::*;
use gpui::*;
use super::Editor;

impl Editor {
    fn edited_text_mut(&mut self) -> Option<&mut String> {
        let id = self.editing_text?;
        match self.doc.annotations.iter_mut().find(|a| a.id == id) {
            Some(Annotation { shape: Shape::Text { text, .. }, .. }) => Some(text),
            _ => None,
        }
    }

    fn insert_text(&mut self, s: &str, mark: bool, cx: &mut Context<Self>) {
        let marked = std::mem::take(&mut self.ime_marked);
        let clean: String = s.chars().filter(|c| !c.is_control() || *c == '\n').collect();
        if let Some(text) = self.edited_text_mut() {
            let mut keep = text.len().saturating_sub(marked);
            while !text.is_char_boundary(keep) {
                keep -= 1;
            }
            text.truncate(keep);
            text.push_str(&clean);
            if mark {
                self.ime_marked = clean.len();
            }
            self.dirty = true;
            cx.notify();
        }
    }
}

impl EntityInputHandler for Editor {
    fn text_for_range(&mut self, range: std::ops::Range<usize>, adjusted: &mut Option<std::ops::Range<usize>>, _: &mut Window, _: &mut Context<Self>) -> Option<String> {
        let text = self.edited_text_mut()?.clone();
        let utf16: Vec<u16> = text.encode_utf16().collect();
        let r = range.start.min(utf16.len())..range.end.min(utf16.len());
        *adjusted = Some(r.clone());
        Some(String::from_utf16_lossy(&utf16[r]))
    }

    fn selected_text_range(&mut self, _: bool, _: &mut Window, _: &mut Context<Self>) -> Option<UTF16Selection> {
        let n = self.edited_text_mut()?.encode_utf16().count();
        Some(UTF16Selection { range: n..n, reversed: false })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<std::ops::Range<usize>> {
        if self.ime_marked == 0 {
            return None;
        }
        let id = self.editing_text?;
        let text = match self.doc.annotations.iter().find(|a| a.id == id) {
            Some(Annotation { shape: Shape::Text { text, .. }, .. }) => text,
            _ => return None,
        };
        let mut start = text.len().saturating_sub(self.ime_marked);
        while !text.is_char_boundary(start) {
            start -= 1;
        }
        let s16 = text[..start].encode_utf16().count();
        let e16 = text.encode_utf16().count();
        Some(s16..e16)
    }

    fn unmark_text(&mut self, _: &mut Window, _: &mut Context<Self>) {
        self.ime_marked = 0;
    }

    fn replace_text_in_range(&mut self, _: Option<std::ops::Range<usize>>, text: &str, _: &mut Window, cx: &mut Context<Self>) {
        self.insert_text(text, false, cx);
    }

    fn replace_and_mark_text_in_range(&mut self, _: Option<std::ops::Range<usize>>, text: &str, _: Option<std::ops::Range<usize>>, _: &mut Window, cx: &mut Context<Self>) {
        self.insert_text(text, true, cx);
    }

    fn bounds_for_range(&mut self, _: std::ops::Range<usize>, element_bounds: Bounds<Pixels>, _: &mut Window, _: &mut Context<Self>) -> Option<Bounds<Pixels>> {
        let (origin, z) = self.view.get();
        let id = self.editing_text?;
        let a = self.doc.annotations.iter().find(|a| a.id == id)?;
        let r = a.bounds();
        let _ = element_bounds;
        Some(Bounds::new(point(origin.x + px(r.x1 * z), origin.y + px(r.y0 * z)), size(px(2.), px((r.y1 - r.y0) * z))))
    }

    fn character_index_for_point(&mut self, _: Point<Pixels>, _: &mut Window, _: &mut Context<Self>) -> Option<usize> {
        None
    }
}
