//! Fade animation registry for [`slim_scrollable`] scrollbars.
//!
//! Each slim scrollable owns a stable [`iced::widget::Id`]. When the user
//! moves the cursor over the rail (or drags the thumb) we want the
//! scrollbar thumb to fade in; when they leave we want it to fade out.
//! Scrolling (wheel, keyboard, drag-thumb, programmatic) keeps the
//! scrollbar visible for an extra [`SC_SCROLL_BOOST_MS`] window after the
//! last scroll event — but only after an initial [`GRACE_MS`] grace
//! period so that programmatic viewport changes during widget creation
//! (e.g. `scroll_to` after navigation) don't pop the scrollbar up on
//! first render.
//!
//! iced 0.14's built-in `Scrollable` does not expose per-widget animation
//! state to its style function, so we keep a small process-wide registry
//! keyed by `Id` and let the style closure read the current alpha every
//! frame.
//!
//! The interpolation uses a **fixed per-frame step** rather than a
//! delta-time accumulator. That way each `ScrollAnimTick` advances the
//! animation by exactly one 60 fps frame worth of progress, regardless
//! of how long the entry sat at rest between transitions. Without this,
//! dt would pile up while the entry was idle (subscription off) and the
//! next transition would jump the alpha to its target in a single step.
//!
//! The 60 Hz tick is emitted from `app::subscription()` while at least
//! one entry is mid-transition OR still inside its scroll-boost window
//! (`is_any_active()`); the handler is `Task::none()` and the only side
//! effect is forcing iced to rebuild the view, which in turn calls the
//! style function and updates the alpha.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use iced::widget::Id;

/// Fade-in duration in milliseconds.
pub const SC_IN_MS: u64 = 150;
/// Fade-out duration in milliseconds.
pub const SC_OUT_MS: u64 = 220;
/// How long the scrollbar stays fully visible after the last scroll
/// event before the fade-out is allowed to start.
pub const SC_SCROLL_BOOST_MS: u64 = 1500;
/// After the registry first sees an id, ignore subsequent scroll events
/// for this many milliseconds. Covers the layout/programmatic-scroll
/// churn at widget creation so the scrollbar stays hidden on first open.
pub const GRACE_MS: u64 = 500;
/// Per-frame budget that the 60 Hz tick approximates. Used to size the
/// fixed fade step.
const FRAME_MS: u64 = 16;

const GC_AFTER_IDLE: Duration = Duration::from_secs(30);

struct Entry {
    target: f32,
    current: f32,
    last_scroll: Option<Instant>,
    created_at: Instant,
}

static REGISTRY: OnceLock<Mutex<HashMap<Id, Entry>>> = OnceLock::new();

fn registry() -> &'static Mutex<HashMap<Id, Entry>> {
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Advance the entry for `id` by one frame and return its current alpha
/// in `[0.0, 1.0]`. `hover` reflects the raw `Status::Hovered` / `Dragged`
/// signal from the style function. The effective target also rises to
/// `1.0` while the entry is inside its scroll-boost window.
///
/// The fade progresses by exactly one 60 fps frame's worth of progress
/// per call, regardless of how long the entry sat at rest. Pass
/// `Instant::now()` for production use; the parameter exists so tests
/// can drive deterministic instant sequences.
pub fn tick(id: Id, hover: bool, now: Instant) -> f32 {
    let mut map = registry().lock().expect("scroll_anim registry poisoned");
    let entry = map.entry(id).or_insert(Entry {
        target: if hover { 1.0 } else { 0.0 },
        current: 0.0,
        last_scroll: None,
        created_at: now,
    });
    let boost = entry
        .last_scroll
        .map(|t| now.duration_since(t) < Duration::from_millis(SC_SCROLL_BOOST_MS))
        .unwrap_or(false);
    entry.target = if hover || boost { 1.0 } else { 0.0 };
    if (entry.current - entry.target).abs() > f32::EPSILON {
        let step = if entry.target > entry.current {
            FRAME_MS as f32 / SC_IN_MS as f32
        } else {
            FRAME_MS as f32 / SC_OUT_MS as f32
        };
        entry.current = if entry.target > entry.current {
            (entry.current + step).min(entry.target)
        } else {
            (entry.current - step).max(entry.target)
        };
    }
    entry.current
}

/// Record that a scroll event just occurred for `id`. Boosts the
/// entry's target to `1.0` for [`SC_SCROLL_BOOST_MS`] so the user can
/// see the scrollbar without having to keep the cursor on the rail.
///
/// Scroll events fired during the initial [`GRACE_MS`] after the
/// entry is first observed are dropped on the floor so programmatic
/// viewport changes during widget creation do not pop the scrollbar up
/// on first render.
pub fn note_scroll(id: Id, now: Instant) {
    let mut map = registry().lock().expect("scroll_anim registry poisoned");
    let entry = map.entry(id).or_insert(Entry {
        target: 1.0,
        current: 1.0,
        last_scroll: None,
        created_at: now,
    });
    if now.duration_since(entry.created_at) < Duration::from_millis(GRACE_MS) {
        return;
    }
    entry.last_scroll = Some(now);
}

/// Returns `true` while at least one entry is mid-transition **or** still
/// inside its scroll-boost window. Used by `app::subscription` to mount
/// or dismiss the 60 Hz tick.
pub fn is_any_active() -> bool {
    let map = registry().lock().expect("scroll_anim registry poisoned");
    let now = Instant::now();
    map.values().any(|e| {
        let transitioning = (e.current - e.target).abs() > f32::EPSILON;
        let boosting = e
            .last_scroll
            .map(|t| now.duration_since(t) < Duration::from_millis(SC_SCROLL_BOOST_MS))
            .unwrap_or(false);
        transitioning || boosting
    })
}

/// Drop entries that have been at rest (and outside the boost window)
/// for longer than [`GC_AFTER_IDLE`], so a long session with many
/// `Id::unique()` calls does not leak memory.
pub fn gc(now: Instant) {
    let mut map = registry().lock().expect("scroll_anim registry poisoned");
    map.retain(|_, e| {
        let transitioning = (e.current - e.target).abs() > f32::EPSILON;
        let boosting = e
            .last_scroll
            .map(|t| now.duration_since(t) < Duration::from_millis(SC_SCROLL_BOOST_MS))
            .unwrap_or(false);
        transitioning || boosting || now.duration_since(e.created_at) < GC_AFTER_IDLE
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    fn unique() -> Id {
        Id::unique()
    }

    fn isolated() -> &'static Mutex<HashMap<Id, Entry>> {
        Box::leak(Box::new(Mutex::new(HashMap::new())))
    }

    fn tick_local(
        map: &'static Mutex<HashMap<Id, Entry>>,
        id: Id,
        hover: bool,
        now: Instant,
    ) -> f32 {
        let mut guard = map.lock().unwrap();
        let entry = guard.entry(id).or_insert(Entry {
            target: if hover { 1.0 } else { 0.0 },
            current: 0.0,
            last_scroll: None,
            created_at: now,
        });
        let boost = entry
            .last_scroll
            .map(|t| now.duration_since(t) < Duration::from_millis(SC_SCROLL_BOOST_MS))
            .unwrap_or(false);
        entry.target = if hover || boost { 1.0 } else { 0.0 };
        if (entry.current - entry.target).abs() > f32::EPSILON {
            let step = if entry.target > entry.current {
                FRAME_MS as f32 / SC_IN_MS as f32
            } else {
                FRAME_MS as f32 / SC_OUT_MS as f32
            };
            entry.current = if entry.target > entry.current {
                (entry.current + step).min(entry.target)
            } else {
                (entry.current - step).max(entry.target)
            };
        }
        entry.current
    }

    fn note_scroll_local(map: &'static Mutex<HashMap<Id, Entry>>, id: Id, now: Instant) {
        let mut guard = map.lock().unwrap();
        let entry = guard.entry(id).or_insert(Entry {
            target: 1.0,
            current: 1.0,
            last_scroll: None,
            created_at: now,
        });
        if now.duration_since(entry.created_at) < Duration::from_millis(GRACE_MS) {
            return;
        }
        entry.last_scroll = Some(now);
    }

    fn any_active_local(map: &Mutex<HashMap<Id, Entry>>, now: Instant) -> bool {
        let guard = map.lock().unwrap();
        guard.values().any(|e| {
            let transitioning = (e.current - e.target).abs() > f32::EPSILON;
            let boosting = e
                .last_scroll
                .map(|t| now.duration_since(t) < Duration::from_millis(SC_SCROLL_BOOST_MS))
                .unwrap_or(false);
            transitioning || boosting
        })
    }

    /// Advance `n` frames of 16 ms each at `t0`, calling `tick_local`
    /// with `hover` for each frame. Returns the alpha after the last
    /// frame. `n` frames advance by `n * FRAME_MS` simulated time.
    fn advance(
        map: &'static Mutex<HashMap<Id, Entry>>,
        id: Id,
        hover: bool,
        t0: Instant,
        n: usize,
    ) -> (f32, Instant) {
        let mut now = t0;
        let mut alpha = 0.0;
        for i in 0..n {
            now = t0 + Duration::from_millis(FRAME_MS * (i as u64 + 1));
            alpha = tick_local(map, id.clone(), hover, now);
        }
        (alpha, now)
    }

    #[test]
    fn starts_invisible_then_fades_in() {
        let map = isolated();
        let id = unique();
        let t0 = Instant::now();
        let (a0, _) = advance(map, id.clone(), false, t0, 1);
        assert_eq!(a0, 0.0);

        let n_in = (SC_IN_MS / FRAME_MS + 1) as usize;
        let (a_mid, _) = advance(
            map,
            id.clone(),
            true,
            t0 + Duration::from_millis(FRAME_MS),
            n_in / 2,
        );
        assert!(a_mid > 0.0 && a_mid < 1.0, "mid fade alpha = {a_mid}");

        let (a_end, _) = advance(
            map,
            id.clone(),
            true,
            t0 + Duration::from_millis(FRAME_MS * n_in as u64 / 2 + FRAME_MS),
            n_in / 2,
        );
        assert!((a_end - 1.0).abs() < 1e-3, "final alpha = {a_end}");
    }

    #[test]
    fn fades_out_when_unhovered() {
        let map = isolated();
        let id = unique();
        let t0 = Instant::now();
        let n_in = (SC_IN_MS / FRAME_MS + 1) as usize;
        let (a_in, after_in) = advance(map, id.clone(), true, t0, n_in);
        assert!((a_in - 1.0).abs() < 1e-3, "pre-fade alpha = {a_in}");

        let n_out = (SC_OUT_MS / FRAME_MS + 1) as usize;
        let (a_mid, after_mid) = advance(
            map,
            id.clone(),
            false,
            after_in + Duration::from_millis(FRAME_MS),
            n_out / 2,
        );
        assert!(a_mid < 1.0 && a_mid > 0.0, "mid fade-out alpha = {a_mid}");

        let (a_end, _) = advance(
            map,
            id.clone(),
            false,
            after_mid + Duration::from_millis(FRAME_MS),
            n_out / 2,
        );
        assert!(a_end.abs() < 1e-3, "final alpha after fade-out = {a_end}");
    }

    #[test]
    fn registry_reports_activity_during_transitions() {
        let map = isolated();
        let id = unique();
        let t0 = Instant::now();
        assert!(!any_active_local(map, t0));

        let (_, mid) = advance(
            map,
            id.clone(),
            true,
            t0,
            (SC_IN_MS / FRAME_MS / 2) as usize,
        );
        assert!(any_active_local(map, mid));

        let (_, settled) = advance(map, id.clone(), true, mid, (SC_IN_MS / FRAME_MS) as usize);
        assert!(!any_active_local(map, settled));
    }

    #[test]
    fn fixed_step_advances_after_long_rest() {
        let map = isolated();
        let id = unique();
        let t0 = Instant::now();
        let _ = advance(map, id.clone(), false, t0, 1);

        let long_rest = t0 + Duration::from_secs(60);
        let (a_after_rest, _) = advance(map, id.clone(), true, long_rest, 1);
        let expected_step = FRAME_MS as f32 / SC_IN_MS as f32;
        assert!(
            (a_after_rest - expected_step).abs() < 1e-3,
            "after 60s rest the fade-in should advance by exactly one frame, got {a_after_rest}"
        );

        let n_in = (SC_IN_MS / FRAME_MS + 1) as usize;
        let (a_full, _) = advance(
            map,
            id.clone(),
            true,
            long_rest + Duration::from_millis(FRAME_MS),
            n_in - 1,
        );
        assert!((a_full - 1.0).abs() < 1e-3, "fade-in completion = {a_full}");
    }

    #[test]
    fn note_scroll_during_grace_is_ignored() {
        let map = isolated();
        let id = unique();
        let t0 = Instant::now();
        let _ = tick_local(map, id.clone(), false, t0);
        let mid_grace = t0 + Duration::from_millis(GRACE_MS / 2);
        note_scroll_local(map, id.clone(), mid_grace);

        let after_grace_call = mid_grace + Duration::from_millis(FRAME_MS);
        let alpha = tick_local(map, id.clone(), false, after_grace_call);
        assert_eq!(
            alpha, 0.0,
            "scroll boost should be ignored during grace period"
        );
    }

    #[test]
    fn note_scroll_after_grace_drives_fade_in() {
        let map = isolated();
        let id = unique();
        let t0 = Instant::now();
        let _ = tick_local(map, id.clone(), false, t0);
        let after_grace = t0 + Duration::from_millis(GRACE_MS + 50);
        note_scroll_local(map, id.clone(), after_grace);

        let n_in = (SC_IN_MS / FRAME_MS + 1) as usize;
        let (alpha, _) = advance(
            map,
            id.clone(),
            false,
            after_grace + Duration::from_millis(FRAME_MS),
            n_in,
        );
        assert!(
            (alpha - 1.0).abs() < 1e-3,
            "scroll boost after grace should fade-in to 1, got {alpha}"
        );
    }

    #[test]
    fn scroll_boost_keeps_alpha_high_after_unhover() {
        let map = isolated();
        let id = unique();
        let t0 = Instant::now();
        let _ = tick_local(map, id.clone(), false, t0);
        note_scroll_local(map, id.clone(), t0 + Duration::from_millis(GRACE_MS + 50));

        let n_in = (SC_IN_MS / FRAME_MS + 1) as usize;
        let (alpha, _) = advance(
            map,
            id.clone(),
            false,
            t0 + Duration::from_millis(GRACE_MS + 50 + FRAME_MS),
            n_in,
        );
        assert!(
            (alpha - 1.0).abs() < 1e-3,
            "alpha during boost window without hover = {alpha}"
        );
    }

    #[test]
    fn scroll_boost_expires_and_lets_fade_out() {
        let map = isolated();
        let id = unique();
        let t0 = Instant::now();
        let _ = tick_local(map, id.clone(), false, t0);
        note_scroll_local(map, id.clone(), t0 + Duration::from_millis(GRACE_MS + 50));

        let t_after = t0 + Duration::from_millis(GRACE_MS + SC_SCROLL_BOOST_MS + 50);
        let n_out = (SC_OUT_MS / FRAME_MS + 1) as usize;
        let (alpha, _) = advance(map, id.clone(), false, t_after, n_out);
        assert!(alpha.abs() < 1e-3, "alpha after boost + fade-out = {alpha}");
    }

    #[test]
    fn registry_active_through_boost_window() {
        let map = isolated();
        let id = unique();
        let t0 = Instant::now();
        let _ = tick_local(map, id.clone(), false, t0);
        note_scroll_local(map, id.clone(), t0 + Duration::from_millis(GRACE_MS + 50));
        let mid = t0 + Duration::from_millis(GRACE_MS + SC_SCROLL_BOOST_MS / 2);
        assert!(
            any_active_local(map, mid),
            "boost should keep registry active mid-window"
        );
        let after =
            t0 + Duration::from_millis(GRACE_MS + SC_SCROLL_BOOST_MS + SC_OUT_MS + FRAME_MS * 2);
        assert!(
            !any_active_local(map, after),
            "after boost and fade-out the registry should be idle"
        );
    }
}
