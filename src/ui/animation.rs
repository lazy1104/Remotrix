use std::sync::OnceLock;
use std::time::{Duration, Instant};

pub use iced_anim::animation::animation;
pub use iced_anim::event::Event;
pub use iced_anim::transition::{Curve, Easing};
pub use iced_anim::Animated;

pub const CARD_ENTER_MS: u64 = 280;
pub const CARD_EXIT_MS: u64 = 200;
pub const HUD_ANIM_MS: u64 = 220;
pub const PROGRESS_MS: u64 = 250;
pub const PILL_MS: u64 = 200;

pub fn ease_out_quad(duration_ms: u64) -> Easing {
    Easing::new(Curve::Custom(|p| 1.0 - (1.0 - p).powi(2)))
        .with_duration(Duration::from_millis(duration_ms))
        .reversible(false)
}

pub fn ease_out_cubic(duration_ms: u64) -> Easing {
    Easing::new(Curve::Custom(|p| 1.0 - (1.0 - p).powi(3)))
        .with_duration(Duration::from_millis(duration_ms))
        .reversible(false)
}

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

#[derive(Default)]
pub struct DialogAnim {
    anim: Animated<f32>,
    dismissing: bool,
}

impl DialogAnim {
    pub fn open(&mut self) {
        self.anim.set_target(1.0);
        self.dismissing = false;
    }

    pub fn begin_exit(&mut self) {
        self.anim.set_target(0.0);
        self.dismissing = true;
    }

    pub fn value(&self) -> f32 {
        *self.anim.value()
    }

    pub fn anim(&self) -> &Animated<f32> {
        &self.anim
    }

    pub fn is_dismissing(&self) -> bool {
        self.dismissing
    }

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

pub fn cycle(now: Instant, period: Duration) -> f32 {
    let p = period.as_secs_f32();
    let t = now.duration_since(epoch()).as_secs_f32() % p;
    t / p
}

pub fn spin(now: Instant, period: Duration) -> f32 {
    cycle(now, period) * 360.0
}
