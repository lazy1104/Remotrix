use std::sync::OnceLock;
use std::time::Duration;

pub use iced::animation::{Animation, Easing};
pub use iced::time::Instant;

pub const EASE_OUT_QUAD: Easing = Easing::EaseOutQuad;
#[allow(dead_code)]
pub const EASE_OUT_CUBIC: Easing = Easing::EaseOutCubic;
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

pub const CARD_ENTER_MS: u64 = 280;
pub const CARD_EXIT_MS: u64 = 200;

pub struct CardAnim {
    anim: Animation<f32>,
}

impl CardAnim {
    pub fn entering(now: Instant) -> Self {
        Self {
            anim: Animation::new(0.0)
                .easing(EASE_OUT_CUBIC)
                .duration(Duration::from_millis(CARD_ENTER_MS))
                .go(1.0, now),
        }
    }

    pub fn exiting(now: Instant) -> Self {
        Self {
            anim: Animation::new(1.0)
                .easing(EASE_OUT_QUAD)
                .duration(Duration::from_millis(CARD_EXIT_MS))
                .go(0.0, now),
        }
    }

    pub fn begin_exit(&mut self, now: Instant) {
        self.anim = self
            .anim
            .clone()
            .easing(EASE_OUT_QUAD)
            .duration(Duration::from_millis(CARD_EXIT_MS))
            .go(0.0, now);
    }

    pub fn value(&self, now: Instant) -> f32 {
        self.anim.interpolate_with(|v| v, now).clamp(0.0, 1.0)
    }

    pub fn is_animating(&self, now: Instant) -> bool {
        self.anim.is_animating(now)
    }
}

#[derive(Default)]
pub struct DialogAnim {
    anim: Option<CardAnim>,
    dismissing: bool,
}

impl DialogAnim {
    pub fn open(&mut self, now: Instant) {
        self.anim = Some(CardAnim::entering(now));
        self.dismissing = false;
    }

    pub fn begin_exit(&mut self, now: Instant) {
        match &mut self.anim {
            Some(a) => a.begin_exit(now),
            None => self.anim = Some(CardAnim::exiting(now)),
        }
        self.dismissing = true;
    }

    pub fn value(&self, now: Instant) -> f32 {
        self.anim.as_ref().map(|a| a.value(now)).unwrap_or(1.0)
    }

    pub fn is_dismissing(&self) -> bool {
        self.dismissing
    }

    pub fn needs_tick(&self, now: Instant) -> bool {
        self.dismissing || self.anim.as_ref().is_some_and(|a| a.is_animating(now))
    }

    pub fn tick(&mut self, now: Instant) -> bool {
        if self.dismissing && self.anim.as_ref().is_some_and(|a| !a.is_animating(now)) {
            self.anim = None;
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
