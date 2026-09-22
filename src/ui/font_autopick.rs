use std::sync::OnceLock;

use system_fonts::{FontStyle, FoundFont};

static SANS_CACHE: OnceLock<Vec<FoundFont>> = OnceLock::new();

fn sans_for_locale() -> &'static [FoundFont] {
    SANS_CACHE
        .get_or_init(|| system_fonts::find_for_system_locale(FontStyle::Sans).2)
        .as_slice()
}

pub fn pick_default_family() -> Option<String> {
    sans_for_locale()
        .first()
        .map(|f| f.family.clone())
        .filter(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pick_default_family_is_idempotent() {
        let a = pick_default_family();
        let b = pick_default_family();
        assert_eq!(a, b);
    }
}
