use gpui::prelude::*;
use gpui::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::time::{Duration, Instant};

pub const FAST: Duration = Duration::from_millis(140);
pub const ENTER: Duration = Duration::from_millis(240);

/// Fade in while sliding into place from `(dx, dy)`. The element must not be absolutely positioned.
pub fn enter<E: Styled + IntoElement + 'static>(id: impl Into<ElementId>, el: E, dx: f32, dy: f32) -> AnimationElement<E> {
    el.with_animation(id, Animation::new(ENTER).with_easing(ease_out_quint()), move |el, t| {
        el.relative().left(px(dx * (1.0 - t))).top(px(dy * (1.0 - t))).opacity(t)
    })
}

pub fn fade_in<E: Styled + IntoElement + 'static>(id: impl Into<ElementId>, el: E) -> AnimationElement<E> {
    el.with_animation(id, Animation::new(FAST), |el, t| el.opacity(t))
}

pub fn fade_out<E: Styled + IntoElement + 'static>(id: impl Into<ElementId>, el: E) -> AnimationElement<E> {
    el.with_animation(id, Animation::new(FAST), |el, t| el.opacity(1.0 - t))
}

thread_local! {
    static LAST: RefCell<HashMap<ElementId, (bool, Instant)>> = RefCell::new(HashMap::new());
}

/// Whether `value` flipped within the last `FAST`, so a stateless control animates
/// a change but not its first paint.
pub fn just_changed(id: &ElementId, value: bool) -> bool {
    LAST.with_borrow_mut(|m| match m.get_mut(id) {
        Some((v, at)) => {
            if *v != value {
                *v = value;
                *at = Instant::now();
            }
            at.elapsed() < FAST
        }
        None => {
            m.insert(id.clone(), (value, Instant::now() - FAST));
            false
        }
    })
}
