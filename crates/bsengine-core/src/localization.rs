//! Localization: translated strings by key, from CSV string tables, with a
//! current locale and a fallback.

use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

use bevy_ecs::prelude::Resource;

/// Every translated string the game has, and the locale it is showing.
///
/// The model all three reference engines share: a string is looked up by a
/// *key*, in tables that map each key to its text in each locale -- Unity's
/// String Tables, Unreal's String Tables, Godot's `TranslationServer`. The
/// tables come from CSV, the one import format all three take, laid out as
/// Godot lays it out: a header row naming the locale of each column after the
/// first, then one row per key.
///
/// ```text
/// keys,en,ko
/// GREETING,Hello,안녕하세요
/// SCORE,"Score: {points}","점수: {points}"
/// ```
///
/// Lookup walks a fallback chain, as Godot's and Unity's do: the current
/// locale exactly (`pt-BR`), then its language alone (`pt`), then the
/// project's default locale; a key in none of them comes back as itself (what
/// Godot shows), so a missing translation is visible on screen rather than a
/// blank. Placeholders are named, `{points}`, and filled by [`Self::format`].
///
/// The tables sit behind an `Arc`: they are read every frame by the scripting
/// snapshot and change only when the tables are reloaded.
#[derive(Resource, Debug, Clone, Default)]
pub struct Localization {
    tables: Arc<HashMap<String, HashMap<String, String>>>,
    locale: String,
    default_locale: String,
}

impl Localization {
    /// Tables from each `(source, csv text)`, later sources overriding earlier
    /// ones key by key; the locale starts at `default_locale`.
    ///
    /// # Errors
    ///
    /// The first table that does not parse, naming its source.
    pub fn from_csv_tables<'a>(
        tables: impl IntoIterator<Item = (&'a str, &'a str)>,
        default_locale: &str,
    ) -> Result<Self, String> {
        let mut merged: HashMap<String, HashMap<String, String>> = HashMap::new();
        for (source, text) in tables {
            for (locale, entries) in parse_csv_table(text).map_err(|e| format!("{source}: {e}"))? {
                merged.entry(locale).or_default().extend(entries);
            }
        }
        let default_locale = normalize_locale(default_locale);
        Ok(Self {
            tables: Arc::new(merged),
            locale: default_locale.clone(),
            default_locale,
        })
    }

    /// The locale being shown, normalized (`ko`, `pt-BR`).
    pub fn locale(&self) -> &str {
        &self.locale
    }

    /// The locale lookups fall back to.
    pub fn default_locale(&self) -> &str {
        &self.default_locale
    }

    /// Switches the locale. Any string is accepted -- a locale with no table
    /// falls back like any missing translation -- and normalized.
    pub fn set_locale(&mut self, locale: &str) {
        self.locale = normalize_locale(locale);
    }

    /// Every locale some table has a column for, sorted.
    pub fn locales(&self) -> Vec<String> {
        let set: BTreeSet<&String> = self.tables.keys().collect();
        set.into_iter().cloned().collect()
    }

    /// The best available locale for `wanted` among the tables' own: the
    /// exact locale, else one with the same language, else `None`. What a
    /// game uses to pick its starting locale from the system's.
    pub fn best_match(&self, wanted: &str) -> Option<String> {
        let wanted = normalize_locale(wanted);
        if self.tables.contains_key(&wanted) {
            return Some(wanted);
        }
        let language = language_of(&wanted);
        let mut same_language: Vec<&String> = self
            .tables
            .keys()
            .filter(|l| language_of(l) == language)
            .collect();
        same_language.sort();
        same_language.first().map(|l| (*l).clone())
    }

    /// The text for `key` in the current locale, down the fallback chain;
    /// `None` when no locale in the chain has it.
    pub fn lookup(&self, key: &str) -> Option<&str> {
        self.lookup_with_locale(key).map(|(text, _)| text)
    }

    /// [`Self::lookup`], with the locale the text was found in -- which is
    /// not the current one when the chain fell back.
    fn lookup_with_locale(&self, key: &str) -> Option<(&str, &str)> {
        let language = language_of(&self.locale);
        [self.locale.as_str(), language, self.default_locale.as_str()]
            .into_iter()
            .find_map(|locale| {
                self.tables
                    .get_key_value(locale)
                    .and_then(|(locale, t)| t.get(key).map(|text| (text, locale)))
                    .filter(|(text, _)| !text.is_empty())
            })
            .map(|(text, locale)| (text.as_str(), locale.as_str()))
    }

    /// [`Self::lookup`], or the key itself when nothing has it.
    pub fn tr(&self, key: &str) -> String {
        self.lookup(key).unwrap_or(key).to_string()
    }

    /// [`Self::tr`] with its placeholders filled from `args` -- `{name}`,
    /// and the `plural` and `select` forms [`format_message`] describes --
    /// choosing plural forms by the rules of the language the text was
    /// *found* in. That is the current locale's unless the chain fell back:
    /// a Polish game missing a key shows the English text, and English text
    /// carries English forms (`one`/`other`), which Polish rules would never
    /// pick (`few`, `many`). ICU and Unreal resolve a message's plurals in
    /// the message's own locale for the same reason.
    pub fn tr_args(&self, key: &str, args: &HashMap<String, String>) -> String {
        match self.lookup_with_locale(key) {
            Some((text, locale)) => format_message(locale, text, args),
            None => format_message(&self.locale, key, args),
        }
    }

    /// Fills `{name}` placeholders in `text` from `args`; a placeholder with
    /// no argument is left as it is, so the gap shows. `{{` and `}}` are
    /// literal braces. Plural forms in `text` are chosen by English rules --
    /// [`Self::tr_args`] uses the text's own language.
    pub fn format(text: &str, args: &HashMap<String, String>) -> String {
        format_message("en", text, args)
    }
}

/// Fills a message's placeholders from `args`, ICU MessageFormat's way --
/// the syntax Unreal's text formatting and most translation tools share:
///
/// - `{name}` -- the argument's text.
/// - `{count, plural, =0 {none} one {# apple} other {# apples}}` -- the case
///   for the number `count` holds: an exact `=N` first, then its CLDR plural
///   category in `locale`'s language (`zero`, `one`, `two`, `few`, `many`,
///   `other`; see [`plural_category`]), then `other`. In the chosen case `#`
///   is the number, as written in the argument.
/// - `{gender, select, female {her} male {his} other {their}}` -- the case
///   whose name is the argument's text, else `other`.
///
/// Cases are messages themselves and may hold placeholders, nested plurals
/// included. `{{` and `}}` are literal braces outside a placeholder. Anything
/// that cannot be resolved -- a missing argument, a plural over something
/// that is not a number, no case to fall back to, a malformed placeholder --
/// is left in the output as written, so the gap shows on screen rather than
/// vanishing, as a missing key does.
pub fn format_message(locale: &str, text: &str, args: &HashMap<String, String>) -> String {
    let mut out = String::with_capacity(text.len());
    format_into(&mut out, language_of(locale), text, args, None);
    out
}

fn format_into(
    out: &mut String,
    language: &str,
    text: &str,
    args: &HashMap<String, String>,
    number: Option<&str>,
) {
    let mut rest = text;
    while let Some(i) = rest.find(['{', '}', '#']) {
        out.push_str(&rest[..i]);
        let tail = &rest[i..];
        if tail.starts_with("{{") || tail.starts_with("}}") {
            out.push_str(&tail[..1]);
            rest = &tail[2..];
        } else if let Some(after) = tail.strip_prefix('#') {
            // The number of the plural this text is a case of; a `#`
            // anywhere else is just a `#`.
            out.push_str(number.unwrap_or("#"));
            rest = after;
        } else if let Some(end) = tail
            .starts_with('{')
            .then(|| matching_brace(tail))
            .flatten()
        {
            let whole = &tail[..=end];
            if !format_placeholder(out, language, &tail[1..end], args, number) {
                out.push_str(whole);
            }
            rest = &tail[end + 1..];
        } else {
            out.push_str(&tail[..1]);
            rest = &tail[1..];
        }
    }
    out.push_str(rest);
}

/// The index of the `}` that closes the `{` `text` starts with.
fn matching_brace(text: &str) -> Option<usize> {
    let mut depth = 0usize;
    for (i, c) in text.char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

/// Writes one placeholder's value; `false` when it cannot be resolved and
/// should be left as written.
fn format_placeholder(
    out: &mut String,
    language: &str,
    body: &str,
    args: &HashMap<String, String>,
    number: Option<&str>,
) -> bool {
    let mut parts = body.splitn(3, ',');
    let name = parts.next().unwrap_or("").trim();
    let (Some(kind), Some(cases)) = (parts.next(), parts.next()) else {
        // `{name}`: a plain argument.
        return match args.get(name) {
            Some(value) => {
                out.push_str(value);
                true
            }
            None => false,
        };
    };
    let Some(cases) = parse_cases(cases) else {
        return false;
    };
    let Some(value) = args.get(name) else {
        return false;
    };
    let case_for = |selector: &str| cases.iter().find(|(s, _)| *s == selector).map(|(_, m)| *m);
    match kind.trim() {
        "plural" => {
            let Ok(n) = value.trim().parse::<f64>() else {
                return false;
            };
            let exact = cases.iter().find_map(|(s, m)| {
                s.strip_prefix('=')
                    .and_then(|v| v.parse::<f64>().ok())
                    .filter(|v| *v == n)
                    .map(|_| *m)
            });
            let Some(message) = exact
                .or_else(|| case_for(plural_category(language, n)))
                .or_else(|| case_for("other"))
            else {
                return false;
            };
            format_into(out, language, message, args, Some(value.trim()));
            true
        }
        "select" => {
            let Some(message) = case_for(value.as_str()).or_else(|| case_for("other")) else {
                return false;
            };
            // A select inside a plural keeps that plural's `#`.
            format_into(out, language, message, args, number);
            true
        }
        _ => false,
    }
}

/// `one {# apple} other {# apples}` -> `[("one", "# apple"), ("other",
/// "# apples")]`; `None` when it does not read that way.
fn parse_cases(text: &str) -> Option<Vec<(&str, &str)>> {
    let mut cases = Vec::new();
    let mut rest = text.trim_start();
    while !rest.is_empty() {
        let open = rest.find('{')?;
        let selector = rest[..open].trim();
        if selector.is_empty() || selector.contains(char::is_whitespace) {
            return None;
        }
        let close = matching_brace(&rest[open..])? + open;
        cases.push((selector, &rest[open + 1..close]));
        rest = rest[close + 1..].trim_start();
    }
    (!cases.is_empty()).then_some(cases)
}

/// The CLDR cardinal plural category of `n` in `language` (`en`, `ru`, ...):
/// which of a message's `zero`/`one`/`two`/`few`/`many`/`other` cases it
/// takes.
///
/// The rules of CLDR's plural table for the languages a game commonly ships
/// in, by family; a language not listed takes English's (`one` for exactly 1,
/// else `other`), which every message has to cover anyway. Fractions follow
/// CLDR where it is simple to (`1.5` is `other` in English, `one` in French)
/// and are `other` elsewhere.
pub fn plural_category(language: &str, n: f64) -> &'static str {
    let integer = n.fract() == 0.0 && n.is_finite();
    let i = n.abs().trunc() as u64;
    let (m10, m100) = (i % 10, i % 100);
    match language {
        // No plural forms.
        "ja" | "ko" | "zh" | "th" | "vi" | "id" | "ms" | "lo" | "my" | "km" | "yue" => "other",
        // 0 and 1 (and 0.x, 1.x) are one.
        "fr" | "pt" | "hi" | "bn" | "fa" | "zu" | "am" | "gu" | "kn" | "mr" => {
            if i <= 1 {
                "one"
            } else {
                "other"
            }
        }
        "ru" | "uk" | "be" => {
            if !integer {
                "other"
            } else if m10 == 1 && m100 != 11 {
                "one"
            } else if (2..=4).contains(&m10) && !(12..=14).contains(&m100) {
                "few"
            } else {
                "many"
            }
        }
        "pl" => {
            if !integer {
                "other"
            } else if i == 1 {
                "one"
            } else if (2..=4).contains(&m10) && !(12..=14).contains(&m100) {
                "few"
            } else {
                "many"
            }
        }
        "cs" | "sk" => {
            if !integer {
                "many"
            } else if i == 1 {
                "one"
            } else if (2..=4).contains(&i) {
                "few"
            } else {
                "other"
            }
        }
        "ar" => {
            if !integer {
                "other"
            } else if i == 0 {
                "zero"
            } else if i == 1 {
                "one"
            } else if i == 2 {
                "two"
            } else if (3..=10).contains(&m100) {
                "few"
            } else if (11..=99).contains(&m100) {
                "many"
            } else {
                "other"
            }
        }
        "ro" => {
            if integer && i == 1 {
                "one"
            } else if !integer || i == 0 || (2..=19).contains(&m100) {
                "few"
            } else {
                "other"
            }
        }
        // English and the many languages that count like it.
        _ => {
            if integer && i == 1 {
                "one"
            } else {
                "other"
            }
        }
    }
}

/// `ko_KR.UTF-8`, `KO-kr`, `ko-KR` -> `ko-KR`: language lowercase, region
/// uppercase, `-` between, encoding and modifiers dropped. Both the POSIX
/// spelling (what `LANG` holds) and the BCP 47 one (what Windows and the
/// tables use) come out the same.
pub fn normalize_locale(locale: &str) -> String {
    let base = locale.split(['.', '@']).next().unwrap_or("").trim();
    let mut parts = base.split(['_', '-']).filter(|p| !p.is_empty());
    let Some(language) = parts.next() else {
        return String::new();
    };
    let mut out = language.to_ascii_lowercase();
    for part in parts {
        out.push('-');
        // A two-letter region is uppercase (KR); a four-letter script is
        // title case (Hans), as BCP 47 writes them.
        if part.len() == 4 {
            let (first, rest) = part.split_at(1);
            out.push_str(&first.to_ascii_uppercase());
            out.push_str(&rest.to_ascii_lowercase());
        } else {
            out.push_str(&part.to_ascii_uppercase());
        }
    }
    out
}

/// The language of a normalized locale: `pt` of `pt-BR`.
fn language_of(locale: &str) -> &str {
    locale.split('-').next().unwrap_or(locale)
}

/// The operating system's locale, normalized -- `None` when it reports none.
pub fn system_locale() -> Option<String> {
    sys_locale::get_locale()
        .map(|l| normalize_locale(&l))
        .filter(|l| !l.is_empty())
}

/// One CSV table: locale -> key -> text.
///
/// The first row is the header: its first cell names the key column (Godot
/// writes `keys`; any name is accepted), every other cell a locale. A column
/// whose header starts with `_` is a comment column and ignored, as Godot
/// ignores it -- room for a translator's notes. Empty rows are skipped; an
/// empty cell means "not translated" and falls back like a missing key.
///
/// # Errors
///
/// An empty table, a header without a locale, an unterminated quote, a row
/// with more cells than the header, or a key given twice.
pub fn parse_csv_table(text: &str) -> Result<HashMap<String, HashMap<String, String>>, String> {
    let rows = parse_csv(text.strip_prefix('\u{feff}').unwrap_or(text))?;
    let mut rows = rows
        .into_iter()
        .enumerate()
        .filter(|(_, r)| r.iter().any(|c| !c.is_empty()));
    let Some((_, header)) = rows.next() else {
        return Err("the table is empty".to_string());
    };
    let columns: Vec<Option<String>> = header
        .iter()
        .skip(1)
        .map(|h| {
            let h = h.trim();
            (!h.is_empty() && !h.starts_with('_')).then(|| normalize_locale(h))
        })
        .collect();
    if columns.iter().all(Option::is_none) {
        return Err("the header names no locale after the key column".to_string());
    }
    let mut tables: HashMap<String, HashMap<String, String>> = HashMap::new();
    let mut seen = std::collections::HashSet::new();
    for (index, row) in rows {
        let line = index + 1;
        if row.len() > header.len() {
            return Err(format!(
                "row {line} has {} cells but the header has {}",
                row.len(),
                header.len()
            ));
        }
        let key = row[0].trim();
        if key.is_empty() {
            continue;
        }
        if !seen.insert(key.to_string()) {
            return Err(format!("row {line}: the key {key:?} is given twice"));
        }
        for (cell, locale) in row.iter().skip(1).zip(&columns) {
            if let Some(locale) = locale {
                tables
                    .entry(locale.clone())
                    .or_default()
                    .insert(key.to_string(), cell.clone());
            }
        }
    }
    Ok(tables)
}

/// RFC 4180 CSV: cells separated by commas, rows by `\n` or `\r\n`, a cell in
/// double quotes may hold commas, newlines and `""` for a quote.
fn parse_csv(text: &str) -> Result<Vec<Vec<String>>, String> {
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut cell = String::new();
    let mut chars = text.chars().peekable();
    let mut in_quotes = false;
    let mut line = 1;
    while let Some(c) = chars.next() {
        if in_quotes {
            match c {
                '"' if chars.peek() == Some(&'"') => {
                    chars.next();
                    cell.push('"');
                }
                '"' => in_quotes = false,
                '\n' => {
                    line += 1;
                    cell.push('\n');
                }
                c => cell.push(c),
            }
            continue;
        }
        match c {
            '"' if cell.is_empty() => in_quotes = true,
            ',' => row.push(std::mem::take(&mut cell)),
            '\r' if chars.peek() == Some(&'\n') => {}
            '\n' => {
                line += 1;
                row.push(std::mem::take(&mut cell));
                rows.push(std::mem::take(&mut row));
            }
            c => cell.push(c),
        }
    }
    if in_quotes {
        return Err(format!("a quote opened before line {line} is never closed"));
    }
    if !cell.is_empty() || !row.is_empty() {
        row.push(cell);
        rows.push(row);
    }
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TABLE: &str = "keys,en,ko,pt,pt-BR,_notes\n\
                         GREETING,Hello,안녕하세요,Olá,Oi,casual\n\
                         SCORE,\"Score: {points}\",\"점수: {points}\",,,\n\
                         ONLY_EN,English only,,,,\n\
                         QUOTED,\"say \"\"hi\"\", then, wave\",,,,\n";

    fn loc(locale: &str) -> Localization {
        let mut l = Localization::from_csv_tables([("strings.csv", TABLE)], "en").unwrap();
        l.set_locale(locale);
        l
    }

    #[test]
    fn lookup_walks_locale_then_language_then_default_then_the_key() {
        assert_eq!(loc("ko").tr("GREETING"), "안녕하세요");
        assert_eq!(loc("pt-BR").tr("GREETING"), "Oi", "the exact locale first");
        assert_eq!(loc("pt-PT").tr("GREETING"), "Olá", "then its language");
        assert_eq!(loc("ko").tr("ONLY_EN"), "English only", "then the default");
        assert_eq!(
            loc("ko").tr("NO_SUCH_KEY"),
            "NO_SUCH_KEY",
            "then the key itself"
        );
        assert_eq!(
            loc("pt-BR").tr("SCORE"),
            "Score: {points}",
            "an empty cell is untranslated: pt-BR and pt are both empty, so en"
        );
        assert_eq!(loc("fr").tr("GREETING"), "Hello", "a locale with no column");
    }

    #[test]
    fn csv_quotes_commas_and_comment_columns() {
        assert_eq!(loc("en").tr("QUOTED"), "say \"hi\", then, wave");
        assert!(
            !loc("en").locales().iter().any(|l| l.contains("notes")),
            "a `_` column is a comment, not a locale: {:?}",
            loc("en").locales()
        );
        assert_eq!(loc("en").locales(), vec!["en", "ko", "pt", "pt-BR"]);
        let multiline = "keys,en\r\nPOEM,\"line one\r\nline two\"\r\n";
        let t = parse_csv_table(multiline).unwrap();
        assert_eq!(t["en"]["POEM"], "line one\r\nline two");
    }

    #[test]
    fn bad_tables_are_refused_with_a_reason() {
        for (text, expected) in [
            ("", "empty"),
            ("keys\nA\n", "no locale"),
            ("keys,en\nA,\"open\n", "never closed"),
            ("keys,en\nA,a,b\n", "cells"),
            ("keys,en\nA,a\nA,b\n", "twice"),
        ] {
            let err = parse_csv_table(text).unwrap_err();
            assert!(err.contains(expected), "{text:?}: {err}");
        }
        let err = Localization::from_csv_tables([("ui.csv", "keys\n")], "en").unwrap_err();
        assert!(err.starts_with("ui.csv: "), "the source is named: {err}");
    }

    #[test]
    fn later_tables_override_earlier_ones_key_by_key() {
        let l = Localization::from_csv_tables(
            [
                ("base.csv", "keys,en\nA,base a\nB,base b\n"),
                ("patch.csv", "keys,en\nB,patched b\n"),
            ],
            "en",
        )
        .unwrap();
        assert_eq!(l.tr("A"), "base a");
        assert_eq!(l.tr("B"), "patched b");
    }

    #[test]
    fn locales_are_normalized_whichever_way_they_are_spelled() {
        assert_eq!(normalize_locale("ko_KR.UTF-8"), "ko-KR");
        assert_eq!(normalize_locale("KO-kr"), "ko-KR");
        assert_eq!(normalize_locale("zh_hans_cn"), "zh-Hans-CN");
        assert_eq!(normalize_locale("de_DE@euro"), "de-DE");
        assert_eq!(normalize_locale(""), "");
        let l = loc("KO_kr");
        assert_eq!(l.locale(), "ko-KR");
        assert_eq!(l.tr("GREETING"), "안녕하세요", "and ko-KR falls back to ko");
    }

    #[test]
    fn best_match_prefers_exact_then_same_language() {
        let l = loc("en");
        assert_eq!(l.best_match("pt_BR.UTF-8").as_deref(), Some("pt-BR"));
        assert_eq!(l.best_match("ko-KR").as_deref(), Some("ko"));
        assert_eq!(l.best_match("pt-PT").as_deref(), Some("pt"));
        assert_eq!(l.best_match("fr-FR"), None);
    }

    #[test]
    fn placeholders_are_filled_by_name() {
        let args: HashMap<String, String> = [("points".to_string(), "42".to_string())]
            .into_iter()
            .collect();
        assert_eq!(Localization::format("점수: {points}", &args), "점수: 42");
        assert_eq!(
            Localization::format("{points}/{max}", &args),
            "42/{max}",
            "an unfilled placeholder shows"
        );
        assert_eq!(Localization::format("{{points}}", &args), "{points}");
        assert_eq!(Localization::format("a } b {", &args), "a } b {");
    }

    fn args(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    /// CLDR's cardinal categories, at the numbers where each family's rules
    /// differ -- the teens of Russian and Polish, French's zero, Arabic's
    /// hundreds -- rather than only at 1 and 2, where most agree.
    #[test]
    fn plural_categories_follow_cldr() {
        for (language, n, expected) in [
            ("en", 0.0, "other"),
            ("en", 1.0, "one"),
            ("en", 2.0, "other"),
            ("en", 1.5, "other"),
            ("fr", 0.0, "one"),
            ("fr", 1.5, "one"),
            ("fr", 2.0, "other"),
            ("ru", 1.0, "one"),
            ("ru", 21.0, "one"),
            ("ru", 11.0, "many"),
            ("ru", 2.0, "few"),
            ("ru", 22.0, "few"),
            ("ru", 24.0, "few"),
            ("ru", 14.0, "many"),
            ("ru", 12.0, "many"),
            ("ru", 5.0, "many"),
            ("ru", 1.5, "other"),
            ("pl", 1.0, "one"),
            ("pl", 21.0, "many"),
            ("pl", 22.0, "few"),
            ("pl", 12.0, "many"),
            ("cs", 1.0, "one"),
            ("cs", 3.0, "few"),
            ("cs", 5.0, "other"),
            ("cs", 1.5, "many"),
            ("ar", 0.0, "zero"),
            ("ar", 1.0, "one"),
            ("ar", 2.0, "two"),
            ("ar", 3.0, "few"),
            ("ar", 103.0, "few"),
            ("ar", 11.0, "many"),
            ("ar", 100.0, "other"),
            ("ro", 0.0, "few"),
            ("ro", 1.0, "one"),
            ("ro", 102.0, "few"),
            ("ro", 101.0, "other"),
            ("ro", 20.0, "other"),
            ("ja", 1.0, "other"),
            ("ko", 1.0, "other"),
            // An unlisted language counts as English does.
            ("xx", 1.0, "one"),
            ("xx", 3.0, "other"),
        ] {
            assert_eq!(plural_category(language, n), expected, "{language} {n}");
        }
    }

    /// ICU's `plural` and `select`: an exact `=N` before the category, the
    /// category before `other`, `#` as the number inside a plural and only
    /// there, placeholders and nested forms inside cases -- and anything that
    /// cannot be resolved left as written.
    #[test]
    fn plural_and_select_messages() {
        let apples =
            "{n, plural, =0 {no apples} =1 {just one apple} one {# apple} other {# apples}}";
        assert_eq!(
            format_message("en", apples, &args(&[("n", "0")])),
            "no apples"
        );
        assert_eq!(
            format_message("en", apples, &args(&[("n", "1")])),
            "just one apple",
            "an exact match before the category"
        );
        assert_eq!(
            format_message("en", apples, &args(&[("n", "7")])),
            "7 apples"
        );

        let ru = "{n, plural, one {# яблоко} few {# яблока} many {# яблок} other {# яблока}}";
        assert_eq!(format_message("ru", ru, &args(&[("n", "21")])), "21 яблоко");
        assert_eq!(format_message("ru", ru, &args(&[("n", "3")])), "3 яблока");
        assert_eq!(format_message("ru", ru, &args(&[("n", "11")])), "11 яблок");
        assert_eq!(
            format_message("ru-RU", ru, &args(&[("n", "11")])),
            "11 яблок",
            "a regional locale uses its language's rules"
        );
        assert_eq!(
            format_message(
                "ru",
                "{n, plural, one {# яблоко} other {# яблок}}",
                &args(&[("n", "3")])
            ),
            "3 яблок",
            "a category the message lacks falls to other"
        );

        let nested = "{who} found {n, plural, one {# coin} other {# coins, {g, select, female {her #-coin} male {his} other {their}} best haul}}#";
        assert_eq!(
            format_message(
                "en",
                nested,
                &args(&[("who", "Ann"), ("n", "3"), ("g", "female")])
            ),
            "Ann found 3 coins, her 3-coin best haul#",
            "placeholders and a select inside a case; a # outside a plural is a #"
        );
        assert_eq!(
            format_message(
                "en",
                nested,
                &args(&[("who", "Bo"), ("n", "1"), ("g", "x")])
            ),
            "Bo found 1 coin#"
        );
        assert_eq!(
            format_message(
                "en",
                "{g, select, female {her} other {their}} {n, plural, one {#} other {#s}}",
                &args(&[("g", "?"), ("n", "2")])
            ),
            "their 2s",
            "an unknown select value takes other"
        );

        for (text, why) in [
            (
                "{missing, plural, one {# apple} other {# apples}}",
                "no argument",
            ),
            ("{word, plural, one {#} other {#}}", "not a number"),
            ("{n, plural, one {# apple}}", "no case and no other"),
            ("{n, plural, one # apple}", "malformed cases"),
            ("{n, ordinal, one {#}}", "an unknown form"),
        ] {
            assert_eq!(
                format_message("en", text, &args(&[("n", "5"), ("word", "five")])),
                text,
                "{why}: left as written"
            );
        }
    }

    /// The rules come from the language the text was *found* in. A Russian
    /// player missing a key reads the English fallback, and the English text
    /// only has `one`/`other`: Russian rules would call 21 `one` -- "21 item"
    /// -- where English says "21 items".
    #[test]
    fn plurals_follow_the_language_the_text_came_from() {
        let table = "keys,en,pl\n\
                     ITEMS,\"{n, plural, one {# item} other {# items}}\",\n\
                     APPLES,\"{n, plural, one {# apple} other {# apples}}\",\
                     \"{n, plural, one {# jabłko} few {# jabłka} many {# jabłek} other {# jabłka}}\"\n";
        let mut l = Localization::from_csv_tables([("t.csv", table)], "en").unwrap();
        l.set_locale("ru");
        assert_eq!(
            plural_category("ru", 21.0),
            "one",
            "premise: Russian and English disagree about 21"
        );
        assert_eq!(l.tr_args("ITEMS", &args(&[("n", "21")])), "21 items");
        l.set_locale("pl");
        assert_eq!(
            l.tr_args("APPLES", &args(&[("n", "5")])),
            "5 jabłek",
            "and the current locale's own text by its own rules"
        );
        assert_eq!(
            l.tr_args("NO_SUCH_KEY {n}", &args(&[("n", "5")])),
            "NO_SUCH_KEY 5",
            "a missing key is formatted as itself"
        );
    }
}
