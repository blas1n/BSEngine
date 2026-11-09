use std::collections::BTreeMap;

use bevy_ecs::prelude::Resource;

use crate::localization::{language_of, normalize_locale};

/// The project's UI fonts, from `project.toml`'s `[ui] fonts` and
/// `[ui.locale_fonts]`, read at start.
///
/// The renderer puts them ahead of egui's built-in fonts, in order: the
/// first is the font text is drawn in, and a character it lacks is looked
/// for in the next, then the next, and last in the built-in ones -- Godot's
/// custom font with its fallbacks, TextMeshPro's font asset with its
/// fallback list. Without one, text in a script egui does not ship (Hangul,
/// kana, Han) draws as a row of identical boxes, so a translated table is no
/// use for those languages.
///
/// Absent in the editor, which keeps its own fonts.
#[derive(Resource, Default, Clone)]
pub struct UiFonts {
    /// `[ui] fonts`: every locale's, behind any locale's own.
    pub fonts: Vec<UiFont>,
    /// `[ui.locale_fonts]`: a locale's own fonts, keyed by normalized locale
    /// (`ja`, `zh-CN`), which go in front of [`Self::fonts`] while it is the
    /// current one. See [`Self::for_locale`].
    pub by_locale: BTreeMap<String, Vec<UiFont>>,
}

/// One font file, by the path the manifest names it with.
#[derive(Clone)]
pub struct UiFont {
    /// As written in `[ui] fonts`; names the font in logs.
    pub path: String,
    /// The file's contents: a `.ttf` or `.otf`.
    pub bytes: Vec<u8>,
}

impl UiFonts {
    /// Only `[ui] fonts`, no locale's own.
    pub fn new(fonts: Vec<UiFont>) -> Self {
        Self {
            fonts,
            by_locale: BTreeMap::new(),
        }
    }

    /// Which `[ui.locale_fonts]` entry `locale` uses: the locale exactly
    /// (`zh-TW`), else its language alone (`zh`) -- the first two steps of
    /// the string lookup's chain, so a locale's text and its fonts are
    /// chosen alike. Not the default locale, the chain's third step: its
    /// fonts would then stand in front for every language that has none,
    /// which is what `[ui] fonts` is for. `None` when neither has an entry.
    pub fn locale_key(&self, locale: &str) -> Option<&str> {
        let locale = normalize_locale(locale);
        let key = [locale.as_str(), language_of(&locale)]
            .into_iter()
            .find_map(|l| self.by_locale.get_key_value(l))
            .map(|(key, _)| key.as_str());
        key
    }

    /// The fonts UI text is drawn in while `locale` is current, first first:
    /// that locale's own ([`Self::locale_key`]) and then `[ui] fonts`. The
    /// locale's go in front because what they are for is the characters the
    /// others also have but draw wrong for it -- one Han code point is
    /// written differently in Japanese and in Chinese, so a Japanese game
    /// puts a Japanese font first and a Chinese one behind it would never be
    /// asked. Unreal's composite font does the same with a sub-font's
    /// cultures. A font in both lists is used once, where it comes first.
    pub fn for_locale(&self, locale: &str) -> Vec<&UiFont> {
        let own = self
            .locale_key(locale)
            .and_then(|key| self.by_locale.get(key))
            .into_iter()
            .flatten();
        let mut chosen: Vec<&UiFont> = Vec::new();
        for font in own.chain(&self.fonts) {
            if !chosen.iter().any(|f| f.path == font.path) {
                chosen.push(font);
            }
        }
        chosen
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn font(path: &str) -> UiFont {
        UiFont {
            path: path.to_string(),
            bytes: Vec::new(),
        }
    }

    fn paths(fonts: &[&UiFont]) -> Vec<String> {
        fonts.iter().map(|f| f.path.clone()).collect()
    }

    fn fonts() -> UiFonts {
        UiFonts {
            fonts: vec![font("title.ttf"), font("kr.otf")],
            by_locale: BTreeMap::from([
                ("ja".to_string(), vec![font("jp.otf")]),
                ("zh".to_string(), vec![font("sc.otf")]),
                ("zh-TW".to_string(), vec![font("tc.otf"), font("title.ttf")]),
            ]),
        }
    }

    /// A locale's own fonts come first, `[ui] fonts` after them in their own
    /// order -- not appended, which would leave a Han character in a
    /// Japanese line drawn by whichever general font has it.
    #[test]
    fn a_locales_fonts_go_in_front_of_the_shared_ones() {
        assert_eq!(
            paths(&fonts().for_locale("ja")),
            ["jp.otf", "title.ttf", "kr.otf"]
        );
    }

    /// The exact locale before its language, as the string lookup does; and
    /// the language when the exact one has no entry.
    #[test]
    fn the_exact_locale_wins_and_its_language_stands_in() {
        let f = fonts();
        assert_eq!(f.locale_key("zh-TW"), Some("zh-TW"));
        assert_eq!(f.locale_key("zh-CN"), Some("zh"));
        assert_eq!(f.locale_key("ja_JP.UTF-8"), Some("ja"), "normalized");
        assert_eq!(
            paths(&f.for_locale("zh-CN")),
            ["sc.otf", "title.ttf", "kr.otf"]
        );
    }

    /// A font listed for the locale and in `[ui] fonts` is used once, in the
    /// locale's place.
    #[test]
    fn a_font_in_both_lists_is_used_once_where_it_comes_first() {
        assert_eq!(
            paths(&fonts().for_locale("zh-TW")),
            ["tc.otf", "title.ttf", "kr.otf"]
        );
    }

    /// A locale without an entry -- or only a more specific one: `zh` does
    /// not take `zh-TW`'s -- draws in `[ui] fonts` alone.
    #[test]
    fn a_locale_without_an_entry_has_the_shared_fonts_alone() {
        let mut f = fonts();
        assert_eq!(f.locale_key("ko"), None);
        assert_eq!(paths(&f.for_locale("ko")), ["title.ttf", "kr.otf"]);
        f.by_locale.remove("zh");
        assert_eq!(f.locale_key("zh"), None);
    }
}
