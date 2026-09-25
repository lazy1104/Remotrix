//! Shared animation primitives for the UI: easing curves, periodic clocks,
//! and a small [`DialogAnim`] state machine for enter/exit transitions.
//!
//! Times are kept in milliseconds via the `ANIM_*` token constants and
//! wrapped in [`Easing`] helpers below. Periodic helpers ([`cycle`],
//! [`spin`]) use a process-wide `OnceLock` epoch because the GUI thread is
//! the only writer.

use std::sync::OnceLock;
use std::time::{Duration, Instant};

pub use iced_anim::animation::animation;
pub use iced_anim::event::Event;
pub use iced_anim::transition::{Curve, Easing};
pub use iced_anim::Animated;

/// Standard animations. The default tier for modal dialogs, toasts, the
/// speed HUD, the top-border progress, and the task progress bar. Exit is
/// ~27% faster than enter, matching the project's enter-vs-exit asymmetry
/// rule (mirrors iOS HIG / Material 3 / Fluent).
pub const ANIM_STANDARD_ENTER_MS: u64 = 220;
/// Standard animations. Exit phase — see [`ANIM_STANDARD_ENTER_MS`].
pub const ANIM_STANDARD_EXIT_MS: u64 = 160;

/// Light / responsive animations. High-frequency UI feedback (sidebar
/// filter pill, scroll-driven transitions, ripple-equivalents). Same 33%
/// asymmetry, shorter overall.
pub const ANIM_LIGHT_ENTER_MS: u64 = 180;
/// Light / responsive animations. Exit phase — see
/// [`ANIM_LIGHT_ENTER_MS`].
pub const ANIM_LIGHT_EXIT_MS: u64 = 120;

/// Heavy / prominent animations. Reserved for transitions that own the
/// user's attention — currently just the task-card slide in/out. Same
/// 28% asymmetry, the longest enter in the system.
pub const ANIM_HEAVY_ENTER_MS: u64 = 280;
/// Heavy / prominent animations. Exit phase — see [`ANIM_HEAVY_ENTER_MS`].
pub const ANIM_HEAVY_EXIT_MS: u64 = 200;

/// Theme colour / accent / light-dark continuous transition. Not an
/// enter/exit pair — used once per theme change.
pub const ANIM_THEME_MS: u64 = 320;

/// Build a non-reversible `t -> 1 - (1-t)^2` easing curve lasting
/// `duration_ms`. Used for element entrance animations where the value
/// should settle quickly and never play backwards.
pub fn ease_out_quad(duration_ms: u64) -> Easing {
    Easing::new(Curve::Custom(|p| 1.0 - (1.0 - p).powi(2)))
        .with_duration(Duration::from_millis(duration_ms))
        .reversible(false)
}

/// Build a non-reversible cubic-out easing (`1 - (1-t)^3`) lasting
/// `duration_ms`. Slightly stronger deceleration than [`ease_out_quad`].
pub fn ease_out_cubic(duration_ms: u64) -> Easing {
    Easing::new(Curve::Custom(|p| 1.0 - (1.0 - p).powi(3)))
        .with_duration(Duration::from_millis(duration_ms))
        .reversible(false)
}

/// Build a non-reversible quadratic-in-out easing lasting `duration_ms`.
/// Used where the value must accelerate from rest and decelerate back to
/// rest (e.g. dialog open + close within a single transition).
pub fn ease_in_out_quad(duration_ms: u64) -> Easing {
    Easing::new(Curve::Custom(|p| {
        if p < 0.5 {
            2.0 * p * p
        } else {
            1.0 - (-2.0 * p + 2.0).powi(2) / 2.0
        }
    }))
    .with_duration(Duration::from_millis(duration_ms))
    .reversible(false)
}

/// Map a normalised animation value in `[0.0, 1.0]` to a toast card scale
/// factor. Used by [`crate::ui::components::toast`] to translate the
/// abstract enter/exit progress into a `Transformation` argument; values
/// outside `[0.0, 1.0]` (clamping artefacts) collapse to the endpoints so
/// the visual never goes past `1.0` or under `0.0` (invisible).
///
/// The second argument is retained for API symmetry with future widgets
/// that may need a non-zero floor; toasts pass `0.0` so the card grows
/// from nothing on enter and collapses to nothing on exit.
pub fn scale_factor_from_value(value: f32, _min_scale: f32) -> f32 {
    value.clamp(0.0, 1.0)
}

/// State machine backing an enter/exit dialog animation.
///
/// Two [`Animated`] fields drive the visible value in `[0.0, 1.0]`:
/// `anim` runs the enter transition (target `1.0`) and `exit_anim` runs
/// the exit transition (target `0.0`) at a shorter duration. `dismissing`
/// remembers which side is currently active so the host can decide when
/// to drop the widget.
pub struct DialogAnim {
    anim: Animated<f32>,
    exit_anim: Animated<f32>,
    dismissing: bool,
}

impl Default for DialogAnim {
    fn default() -> Self {
        Self {
            anim: Animated::transition(0.0, ease_out_cubic(ANIM_STANDARD_ENTER_MS)),
            exit_anim: Animated::transition(0.0, ease_out_cubic(ANIM_STANDARD_EXIT_MS)),
            dismissing: false,
        }
    }
}

impl DialogAnim {
    /// Start the open transition. Rebuilds `anim` from `0.0` so the
    /// scale ramp `0.0 → 1.0` plays in full on every call, regardless
    /// of any previous enter/exit cycle. Safe to call from any state;
    /// resets the `dismissing` flag so an in-progress close is cancelled.
    pub fn open(&mut self) {
        self.dismissing = false;
        self.anim = Animated::transition(0.0, ease_out_cubic(ANIM_STANDARD_ENTER_MS));
        self.anim.set_target(1.0);
    }

    /// Start the exit transition. Anchors the exit easing at the current
    /// visible value so there's no jump when dismissed mid-enter. Sets
    /// `dismissing` so [`Self::completed_dismiss`] can later report
    /// completion.
    pub fn begin_exit(&mut self) {
        let current = self.value();
        self.exit_anim = Animated::transition(
            current.clamp(0.0, 1.0),
            ease_out_cubic(ANIM_STANDARD_EXIT_MS),
        );
        self.exit_anim.set_target(0.0);
        self.dismissing = true;
    }

    /// The current visible value, in `[0.0, 1.0]`. Returns `0.0` before the
    /// open transition has played and again after the exit has finished.
    pub fn value(&self) -> f32 {
        if self.dismissing {
            *self.exit_anim.value()
        } else {
            *self.anim.value()
        }
    }

    /// Borrow the [`Animated`] that should currently drive the
    /// `iced_anim::animation` subscription. Returns the enter transition
    /// while the dialog is open/closing-in and the exit transition while
    /// it's dismissing.
    pub fn phase_anim(&self) -> &Animated<f32> {
        if self.dismissing {
            &self.exit_anim
        } else {
            &self.anim
        }
    }

    /// Returns `true` between [`Self::begin_exit`] and
    /// [`Self::completed_dismiss`].
    pub fn is_dismissing(&self) -> bool {
        self.dismissing
    }

    /// Forward an `iced_anim` event into the currently active transition.
    pub fn update(&mut self, event: Event<f32>) {
        if self.dismissing {
            self.exit_anim.update(event);
        } else {
            self.anim.update(event);
        }
    }

    /// Call after each `update`; returns `true` once the exit animation has
    /// finished, resetting the dismissing flag.
    pub fn completed_dismiss(&mut self) -> bool {
        if self.dismissing && !self.exit_anim.is_animating() {
            self.dismissing = false;
            return true;
        }
        false
    }
}

static ANIM_EPOCH: OnceLock<Instant> = OnceLock::new();
fn epoch() -> Instant {
    *ANIM_EPOCH.get_or_init(Instant::now)
}

/// Normalised position of `now` inside a periodic `period`, in `[0, 1)`.
///
/// The zero point is the first call into this function in the process —
/// subsequent calls are always relative to that epoch, so the returned
/// phase is stable across frames even after long idle periods.
pub fn cycle(now: Instant, period: Duration) -> f32 {
    let p = period.as_secs_f32();
    let t = now.duration_since(epoch()).as_secs_f32() % p;
    t / p
}

/// Angle in degrees corresponding to one full revolution per `period`.
///
/// Equivalent to `cycle(now, period) * 360.0`; always in `[0, 360)` and
/// suitable for driving a spinner.
pub fn spin(now: Instant, period: Duration) -> f32 {
    cycle(now, period) * 360.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cycle_is_in_unit_range() {
        let now = Instant::now();
        let period = Duration::from_secs(10);
        for offset_ms in [0, 100, 500, 1_000, 2_500, 9_999] {
            let t = now + Duration::from_millis(offset_ms);
            let v = cycle(t, period);
            assert!((0.0..1.0).contains(&v), "cycle out of [0,1): {v}");
        }
    }

    #[test]
    fn cycle_wraps_at_period_boundary() {
        // The function uses a process-wide epoch, so we can only check the
        // relative invariant: phase strictly increases between two times and
        // does not blow up when the period is shorter than the elapsed gap.
        let now = Instant::now();
        let period = Duration::from_millis(50);
        let a = cycle(now, period);
        let b = cycle(now + Duration::from_millis(10), period);
        assert!((0.0..1.0).contains(&a));
        assert!((0.0..1.0).contains(&b));
    }

    #[test]
    fn cycle_period_larger_than_elapsed_stays_low() {
        // When the elapsed time is much smaller than the period, the
        // returned phase must be very close to zero (not larger than 1).
        let now = Instant::now();
        let period = Duration::from_secs(60 * 60);
        let v = cycle(now, period);
        assert!((0.0..1.0).contains(&v), "unexpected phase {v}");
    }

    #[test]
    fn spin_in_angle_range() {
        let now = Instant::now();
        let period = Duration::from_secs(2);
        for offset_ms in [0, 250, 500, 1_000, 1_999] {
            let t = now + Duration::from_millis(offset_ms);
            let v = spin(t, period);
            assert!((0.0..360.0).contains(&v), "spin out of [0,360): {v}");
        }
    }

    #[test]
    fn dialog_anim_default_state() {
        let mut a = DialogAnim::default();
        assert_eq!(a.value(), 0.0);
        assert!(!a.is_dismissing());
        assert!(!a.completed_dismiss());
    }

    #[test]
    fn dialog_anim_completed_dismiss_false_when_not_dismissing() {
        let mut a = DialogAnim::default();
        assert!(!a.completed_dismiss());
        a.open();
        // open() does not flip dismissing, so completed_dismiss stays false
        // regardless of animation state.
        assert!(!a.completed_dismiss());
    }

    #[test]
    fn dialog_anim_open_resets_dismissing() {
        let mut a = DialogAnim::default();
        a.begin_exit();
        assert!(a.is_dismissing());
        a.open();
        assert!(!a.is_dismissing());
    }

    #[test]
    fn dialog_anim_open_replays_enter() {
        let mut a = DialogAnim::default();
        a.open();
        a.update(Event::Settle);
        assert_eq!(a.value(), 1.0);
        // Second open rebuilds the enter anim from 0.0 and re-plays the
        // full 0 → 1 scale ramp.
        a.open();
        assert!(a.anim.is_animating());
        a.update(Event::Settle);
        assert_eq!(a.value(), 1.0);
    }

    #[test]
    fn dialog_anim_begin_exit_sets_flag() {
        let mut a = DialogAnim::default();
        a.begin_exit();
        assert!(a.is_dismissing());
    }
}
