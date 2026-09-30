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
        let language = language_of(&self.locale);
        [self.locale.as_str(), language, self.default_locale.as_str()]
            .into_iter()
            .find_map(|locale| {
                self.tables
                    .get(locale)
                    .and_then(|t| t.get(key))
                    .filter(|text| !text.is_empty())
            })
            .map(String::as_str)
    }

    /// [`Self::lookup`], or the key itself when nothing has it.
    pub fn tr(&self, key: &str) -> String {
        self.lookup(key).unwrap_or(key).to_string()
    }

    /// Fills `{name}` placeholders in `text` from `args`; a placeholder with
    /// no argument is left as it is, so the gap shows. `{{` and `}}` are
    /// literal braces.
    pub fn format(text: &str, args: &HashMap<String, String>) -> String {
        let mut out = String::with_capacity(text.len());
        let mut rest = text;
        while let Some(i) = rest.find(['{', '}']) {
            out.push_str(&rest[..i]);
            let tail = &rest[i..];
            if tail.starts_with("{{") || tail.starts_with("}}") {
                out.push_str(&tail[..1]);
                rest = &tail[2..];
            } else if let Some(end) = tail.strip_prefix('{').and_then(|t| t.find('}')) {
                let name = &tail[1..1 + end];
                match args.get(name) {
                    Some(value) => out.push_str(value),
                    None => out.push_str(&tail[..end + 2]),
                }
                rest = &tail[end + 2..];
            } else {
                out.push_str(&tail[..1]);
                rest = &tail[1..];
            }
        }
        out.push_str(rest);
        out
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
}
