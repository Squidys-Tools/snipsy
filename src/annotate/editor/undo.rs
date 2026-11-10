use super::Editor;

impl Editor {
    pub(super) fn push_undo(&mut self) {
        self.tentative = None;
        self.undo.push(self.doc.clone());
        if self.undo.len() > 100 {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.dirty = true;
    }

    /// Snapshot before an operation that may turn out to change nothing
    /// (a selection click, starting a text edit). `drop_noop_undo` undoes it.
    pub(super) fn push_undo_tentative(&mut self) {
        let saved = (std::mem::take(&mut self.redo), self.dirty);
        self.push_undo();
        self.tentative = Some(saved);
    }

    /// Snapshot before restyling the selection, unless it is the text being
    /// edited under a pending tentative snapshot, which already covers it (so
    /// an abandoned text still leaves no undo step).
    pub(super) fn push_undo_restyle(&mut self) {
        if self.tentative.is_some() && self.editing_text.is_some() && self.editing_text == self.selected {
            return;
        }
        self.push_undo();
    }

    /// Drop the last undo snapshot if it equals the current doc, restoring the
    /// redo stack / dirty flag a tentative snapshot cleared.
    pub(super) fn drop_noop_undo(&mut self) {
        if self.undo.last().is_some_and(|d| d.same_content(&self.doc)) {
            self.undo.pop();
            if let Some((r, d)) = self.tentative.take() {
                self.redo = r;
                self.dirty = d;
            }
        }
        self.tentative = None;
    }

    pub(super) fn do_undo(&mut self) {
        self.commit_text();
        if let Some(d) = self.undo.pop() {
            self.redo.push(std::mem::replace(&mut self.doc, d));
            self.selected = None;
            self.dirty = true;
        }
        self.reset_crop();
    }

    pub(super) fn do_redo(&mut self) {
        self.commit_text();
        if let Some(d) = self.redo.pop() {
            self.undo.push(std::mem::replace(&mut self.doc, d));
            self.selected = None;
            self.dirty = true;
        }
        self.reset_crop();
    }
}
