use iced::Color;
use palette::cam16::{Cam16, Cam16Jch, Parameters, StaticWp};
use palette::convert::FromColorUnclamped;
use palette::hues::Cam16Hue;
use palette::white_point::D65;
use palette::{IntoColor, Srgb, Xyz};

type Params = palette::cam16::BakedParameters<StaticWp<D65>, f64>;

fn params() -> Params {
    Parameters::default_static_wp(40.0).bake()
}

fn rgb_to_xyz(c: Color) -> Xyz<D65, f64> {
    Srgb::new(f64::from(c.r), f64::from(c.g), f64::from(c.b))
        .into_linear()
        .into_color()
}

fn l_star_from_y(y: f64) -> f64 {
    if y > 0.008_856 {
        116.0 * y.cbrt() - 16.0
    } else {
        903.3 * y
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Hct {
    pub hue: f64,
    pub chroma: f64,
    pub tone: f64,
}

impl Hct {
    pub fn from_rgb(color: Color) -> Self {
        let xyz = rgb_to_xyz(color);
        let cam = Cam16::from_xyz(xyz, params());
        Self {
            hue: cam.hue.into_positive_degrees(),
            chroma: cam.chroma,
            tone: l_star_from_y(xyz.y),
        }
    }

    pub fn to_rgb(self) -> Color {
        let (r, g, b) = self.to_rgb_parts();
        Color::from_rgb(
            r.clamp(0.0, 1.0) as f32,
            g.clamp(0.0, 1.0) as f32,
            b.clamp(0.0, 1.0) as f32,
        )
    }

    pub fn to_rgb_parts(self) -> (f64, f64, f64) {
        let mut chroma = self.chroma;
        loop {
            let (r, g, b) = self.color_at_chroma(chroma);
            if (0.0..=1.0).contains(&r) && (0.0..=1.0).contains(&g) && (0.0..=1.0).contains(&b) {
                return (r, g, b);
            }
            chroma *= 0.85;
            if chroma < 0.05 {
                return (r.clamp(0.0, 1.0), g.clamp(0.0, 1.0), b.clamp(0.0, 1.0));
            }
        }
    }

    fn color_at_chroma(&self, chroma: f64) -> (f64, f64, f64) {
        let mut lo = 0.0f64;
        let mut hi = 100.0f64;
        for _ in 0..15 {
            let mid = (lo + hi) / 2.0;
            let l = l_star_from_y(self.y_for_lightness(chroma, mid));
            if l < self.tone {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        let xyz =
            Cam16Jch::new((lo + hi) / 2.0, chroma, Cam16Hue::new(self.hue)).into_xyz(params());
        let srgb: Srgb<f64> = FromColorUnclamped::from_color_unclamped(xyz);
        (srgb.red, srgb.green, srgb.blue)
    }

    fn y_for_lightness(&self, chroma: f64, lightness: f64) -> f64 {
        Cam16Jch::new(lightness, chroma, Cam16Hue::new(self.hue))
            .into_xyz(params())
            .y
    }
}

pub fn ramp(hue: f64, chroma: f64, tone: f64) -> Color {
    Hct { hue, chroma, tone }.to_rgb()
}

pub fn hue_distance(a: f64, b: f64) -> f64 {
    let d = (a - b).abs() % 360.0;
    d.min(360.0 - d)
}

pub fn push_hue_away(hue: f64, seed: f64, min_sep: f64) -> f64 {
    let d = hue_distance(hue, seed);
    if d >= min_sep {
        return hue;
    }
    let delta = min_sep - d;
    let signed = (hue - seed).rem_euclid(360.0);
    let pushed = if signed > 180.0 {
        hue - delta
    } else {
        hue + delta
    };
    pushed.rem_euclid(360.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::theme::CANDIDATE_COLORS;

    fn assert_tone(c: Color, expected: f64) {
        let h = Hct::from_rgb(c);
        assert!(
            (h.tone - expected).abs() <= 1.0,
            "tone mismatch: got {} expected {}",
            h.tone,
            expected
        );
    }

    #[test]
    fn to_rgb_from_rgb_round_trip_tone_in_gamut() {
        for (seed, _) in CANDIDATE_COLORS {
            let h = Hct::from_rgb(*seed);
            for tone in [10.0, 40.0, 80.0, 90.0, 98.0] {
                let hct = Hct {
                    hue: h.hue,
                    chroma: h.chroma,
                    tone,
                };
                let (r, g, b) = hct.to_rgb_parts();
                let in_gamut = (0.0..=1.0).contains(&r)
                    && (0.0..=1.0).contains(&g)
                    && (0.0..=1.0).contains(&b);
                if in_gamut {
                    assert_tone(hct.to_rgb(), tone);
                }
            }
        }
    }

    #[test]
    fn neutral_ramp_round_trip_tone() {
        for (seed, _) in CANDIDATE_COLORS {
            let h = Hct::from_rgb(*seed);
            for tone in [10.0, 90.0, 98.0] {
                let hct = Hct {
                    hue: h.hue,
                    chroma: h.chroma * 0.10,
                    tone,
                };
                let (r, g, b) = hct.to_rgb_parts();
                let in_gamut = (0.0..=1.0).contains(&r)
                    && (0.0..=1.0).contains(&g)
                    && (0.0..=1.0).contains(&b);
                if in_gamut {
                    assert_tone(hct.to_rgb(), tone);
                }
            }
        }
    }

    #[test]
    fn palette_tones_are_faithful() {
        use crate::ui::theme;
        for (seed, _) in CANDIDATE_COLORS {
            for (dark, tone) in [(true, 80.0), (false, 40.0)] {
                let t = theme::build_iced(*seed, dark);
                let p = t.extended_palette();
                let primary = Hct::from_rgb(p.primary.base.color).tone;
                let danger = Hct::from_rgb(p.danger.base.color).tone;
                assert!(
                    (primary - tone).abs() <= 2.0,
                    "seed {:?} dark {} primary tone {} expected {}",
                    seed,
                    dark,
                    primary,
                    tone
                );
                assert!(
                    (danger - tone).abs() <= 2.0,
                    "seed {:?} dark {} danger tone {} expected {}",
                    seed,
                    dark,
                    danger,
                    tone
                );
            }
        }
    }

    #[test]
    fn semantic_colors_keep_min_separation() {
        for (seed, _) in CANDIDATE_COLORS {
            let h = Hct::from_rgb(*seed);
            for canonical in [25.0, 140.0, 60.0] {
                let pushed = push_hue_away(canonical, h.hue, 25.0);
                assert!(
                    hue_distance(pushed, h.hue) + 1e-6 >= 25.0,
                    "seed hue {} canonical {} pushed {} too close",
                    h.hue,
                    canonical,
                    pushed
                );
            }
        }
    }

    #[test]
    fn hue_distance_wraps() {
        assert!((hue_distance(350.0, 10.0) - 20.0).abs() < 1e-6);
        assert!((hue_distance(0.0, 0.0)).abs() < 1e-6);
        assert!((hue_distance(180.0, 0.0) - 180.0).abs() < 1e-6);
    }

    #[test]
    fn push_hue_away_wraps_around() {
        let pushed = push_hue_away(10.0, 0.0, 25.0);
        assert!(hue_distance(pushed, 0.0) >= 25.0);
        let pushed = push_hue_away(350.0, 0.0, 25.0);
        assert!(hue_distance(pushed, 0.0) >= 25.0);
    }
}
