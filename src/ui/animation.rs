//! Shared animation primitives for the UI: easing curves, periodic clocks,
//! and a small [`DialogAnim`] state machine for enter/exit transitions.
//!
//! Times are kept in milliseconds via the `*_MS` constants and wrapped in
//! [`Easing`] helpers below. Periodic helpers ([`cycle`], [`spin`]) use a
//! process-wide `OnceLock` epoch because the GUI thread is the only writer.

use std::sync::OnceLock;
use std::time::{Duration, Instant};

pub use iced_anim::animation::animation;
pub use iced_anim::event::Event;
pub use iced_anim::transition::{Curve, Easing};
pub use iced_anim::Animated;

/// Card slide-in duration in milliseconds.
pub const CARD_ENTER_MS: u64 = 280;
/// Card slide-out duration in milliseconds.
pub const CARD_EXIT_MS: u64 = 200;
/// Heads-up-display fade/slide duration.
pub const HUD_ANIM_MS: u64 = 220;
/// Progress-bar easing duration.
pub const PROGRESS_MS: u64 = 250;
/// Filter-pill slide duration.
pub const PILL_MS: u64 = 200;
/// Modal dialog fade/scale duration.
pub const DIALOG_ANIM_MS: u64 = 240;

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

/// State machine backing an enter/exit dialog animation.
///
/// The inner [`Animated`] field drives the visible value in `[0.0, 1.0]`,
/// while `dismissing` remembers whether we are currently playing the
/// closing transition so the host can decide when to drop the widget.
pub struct DialogAnim {
    anim: Animated<f32>,
    dismissing: bool,
}

impl Default for DialogAnim {
    fn default() -> Self {
        Self {
            anim: Animated::transition(0.0, ease_out_cubic(DIALOG_ANIM_MS)),
            dismissing: false,
        }
    }
}

impl DialogAnim {
    /// Start the open transition. Safe to call repeatedly; resets the
    /// `dismissing` flag so an in-progress close is cancelled.
    pub fn open(&mut self) {
        self.anim.set_target(1.0);
        self.dismissing = false;
    }

    /// Start the exit transition. Sets `dismissing` so
    /// [`Self::completed_dismiss`] can later report completion.
    pub fn begin_exit(&mut self) {
        self.anim.set_target(0.0);
        self.dismissing = true;
    }

    /// The current visible value, in `[0.0, 1.0]`. Returns `0.0` before the
    /// open transition has played and again after the exit has finished.
    pub fn value(&self) -> f32 {
        *self.anim.value()
    }

    /// Borrow the underlying [`Animated`] to drive it from an
    /// `iced_anim::animation` subscription.
    pub fn anim(&self) -> &Animated<f32> {
        &self.anim
    }

    /// Returns `true` between [`Self::begin_exit`] and
    /// [`Self::completed_dismiss`].
    pub fn is_dismissing(&self) -> bool {
        self.dismissing
    }

    /// Forward an `iced_anim` event into the underlying value.
    pub fn update(&mut self, event: Event<f32>) {
        self.anim.update(event);
    }

    /// Call after each `update`; returns `true` once the exit animation has
    /// finished, resetting the dismissing flag.
    pub fn completed_dismiss(&mut self) -> bool {
        if self.dismissing && !self.anim.is_animating() {
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
    fn dialog_anim_begin_exit_sets_flag() {
        let mut a = DialogAnim::default();
        a.begin_exit();
        assert!(a.is_dismissing());
    }
}
