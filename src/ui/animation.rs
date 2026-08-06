use std::sync::OnceLock;
use std::time::Duration;

pub use iced::animation::{Animation, Easing};
pub use iced::time::Instant;

pub const EASE_OUT_QUAD: Easing = Easing::EaseOutQuad;
#[allow(dead_code)]
pub const EASE_OUT_CUBIC: Easing = Easing::EaseOutCubic;
#[allow(dead_code)]
pub const EASE_IN_OUT_QUAD: Easing = Easing::EaseInOutQuad;
#[allow(dead_code)]
pub const EASE_OUT_BACK: Easing = Easing::EaseOutBack;
#[allow(dead_code)]
pub const EASE_OUT_ELASTIC: Easing = Easing::EaseOutElastic;
pub const EASE_PROGRESS: Easing = EASE_OUT_QUAD;

pub const PROGRESS_MS_MIN: f32 = 100.0;
pub const PROGRESS_MS_MAX: f32 = 400.0;

pub fn progress_duration(delta_pct: f32) -> Duration {
    let ms = (100.0 + delta_pct * 12.0).clamp(PROGRESS_MS_MIN, PROGRESS_MS_MAX);
    Duration::from_millis(ms as u64)
}

pub struct ProgressTween {
    anim: Animation<f32>,
    last_target: f32,
}

impl ProgressTween {
    pub fn new(v: f32) -> Self {
        Self {
            anim: Animation::new(v),
            last_target: v,
        }
    }

    pub fn towards(&mut self, target: f32, now: Instant) {
        let delta = (target - self.last_target).abs();
        self.last_target = target;
        self.anim = self
            .anim
            .clone()
            .easing(EASE_PROGRESS)
            .duration(progress_duration(delta))
            .go(target, now);
    }

    pub fn value(&self, now: Instant) -> f32 {
        self.anim.interpolate_with(|v| v, now)
    }

    pub fn is_animating(&self, now: Instant) -> bool {
        self.anim.is_animating(now)
    }
}

#[allow(dead_code)]
static ANIM_EPOCH: OnceLock<Instant> = OnceLock::new();
#[allow(dead_code)]
fn epoch() -> Instant {
    *ANIM_EPOCH.get_or_init(Instant::now)
}

#[allow(dead_code)]
pub fn cycle(now: Instant, period: Duration) -> f32 {
    let p = period.as_secs_f32();
    let t = now.duration_since(epoch()).as_secs_f32() % p;
    t / p
}

#[allow(dead_code)]
pub fn spin(now: Instant, period: Duration) -> f32 {
    cycle(now, period) * 360.0
}
