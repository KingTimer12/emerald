//! Interface text in the user's language. Each language is a TOML file under
//! `locales/`, embedded at build time (see `build.rs`), whose nested tables
//! flatten to dotted keys (`friends.add`). Code asks for text with [`t!`]; a
//! key missing from the current language falls back to English, then to the
//! key itself. `tools/discord_locales.py` fills other languages from the
//! Discord web client's own translations.
//!
//! Locale codes are Discord's (`en-US`, `pt-BR`, …), so the same code goes
//! to the API and server-side messages come back in the same language.

use std::collections::HashMap;
use std::fmt::Display;
use std::ops::Range;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Every bundled language as (code, TOML), English first: it is the fallback.
const SOURCES: &[(&str, &str)] = include!(concat!(env!("OUT_DIR"), "/locales.rs"));

pub const FALLBACK: &str = "en-US";

struct Catalog {
    code: &'static str,
    texts: HashMap<String, &'static str>,
}

static CATALOGS: OnceLock<Vec<Catalog>> = OnceLock::new();
static CURRENT: AtomicUsize = AtomicUsize::new(0);

fn catalogs() -> &'static [Catalog] {
    CATALOGS.get_or_init(|| {
        SOURCES
            .iter()
            .map(|(code, source)| Catalog {
                code,
                texts: parse(source).unwrap_or_else(|e| panic!("locales/{code}.toml: {e}")),
            })
            .collect()
    })
}

/// Flattens a TOML document into dotted keys. The text lives as long as the
/// program, so lookups hand out `&'static str` like a literal would.
fn parse(source: &str) -> Result<HashMap<String, &'static str>, String> {
    fn walk(prefix: &str, table: toml::Table, out: &mut HashMap<String, &'static str>) -> Result<(), String> {
        for (key, value) in table {
            let key = if prefix.is_empty() { key } else { format!("{prefix}.{key}") };
            match value {
                toml::Value::String(text) => {
                    out.insert(key, Box::leak(text.into_boxed_str()));
                }
                toml::Value::Table(table) => walk(&key, table, out)?,
                other => return Err(format!("{key}: expected text, found {}", other.type_str())),
            }
        }
        Ok(())
    }
    let table: toml::Table = source.parse().map_err(|e: toml::de::Error| e.to_string())?;
    let mut out = HashMap::new();
    walk("", table, &mut out)?;
    Ok(out)
}

/// Switches the interface language; unknown codes fall back to English.
/// Also accepts a bare language (`pt`) or a system locale (`pt_BR`).
pub fn set(code: &str) {
    CURRENT.store(resolve(code).unwrap_or(0), Ordering::Relaxed);
}

fn resolve(code: &str) -> Option<usize> {
    let code = code.replace('_', "-");
    let list = catalogs();
    list.iter()
        .position(|c| c.code.eq_ignore_ascii_case(&code))
        .or_else(|| {
            let language = code.split('-').next()?;
            list.iter()
                .position(|c| c.code.split('-').next().is_some_and(|l| l.eq_ignore_ascii_case(language)))
        })
}

/// The bundled language closest to the system's, else English.
pub fn system() -> &'static str {
    let list = catalogs();
    sys_locale::get_locales()
        .find_map(|locale| resolve(&locale))
        .map_or(FALLBACK, |ix| list[ix].code)
}

/// The language in use, as a Discord locale code.
pub fn current() -> &'static str {
    catalogs()[CURRENT.load(Ordering::Relaxed)].code
}

/// A bundled language as the language picker shows it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Language {
    /// Discord locale code.
    pub code: &'static str,
    /// Its name in itself: "Deutsch", "Português (Brasil)".
    pub name: &'static str,
    /// Flag emoji, as Discord's picker shows one per language.
    pub flag: &'static str,
}

/// Every bundled language.
pub fn available() -> impl Iterator<Item = Language> {
    catalogs().iter().map(|c| Language {
        code: c.code,
        name: c.texts.get("language.name").copied().unwrap_or(c.code),
        flag: c.texts.get("language.flag").copied().unwrap_or(""),
    })
}

/// The text for `key` in the current language.
pub fn text(key: &'static str) -> &'static str {
    let list = catalogs();
    list[CURRENT.load(Ordering::Relaxed)]
        .texts
        .get(key)
        .or_else(|| list[0].texts.get(key))
        .copied()
        .unwrap_or(key)
}

/// The text for `key` with each `{name}` replaced by its argument.
pub fn format(key: &'static str, args: &[(&str, &dyn Display)]) -> String {
    fill(text(key), args).0
}

/// Like [`format`], also returning where each argument landed, so callers
/// can style the names inside a sentence whatever the word order.
pub fn format_spans(key: &'static str, args: &[(&str, &dyn Display)]) -> (String, Vec<Range<usize>>) {
    fill(text(key), args)
}

fn fill(template: &str, args: &[(&str, &dyn Display)]) -> (String, Vec<Range<usize>>) {
    use std::fmt::Write;
    let mut out = String::with_capacity(template.len() + 16);
    let mut spans = Vec::new();
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        let arg = after
            .find('}')
            .and_then(|close| Some((close, args.iter().find(|(name, _)| *name == &after[..close])?)));
        match arg {
            Some((close, (_, value))) => {
                let start = out.len();
                write!(out, "{value}").ok();
                spans.push(start..out.len());
                rest = &after[close + 1..];
            }
            None => {
                out.push('{');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    (out, spans)
}

/// `t!("key")` is the text for `key`, a `&'static str`; with arguments,
/// `t!("key", name = value)` fills `{name}` and returns a `String`.
#[macro_export]
macro_rules! t {
    ($key:literal) => {
        $crate::i18n::text($key)
    };
    ($key:literal, $($name:ident = $value:expr),+ $(,)?) => {
        $crate::i18n::format($key, &[$((stringify!($name), &$value as &dyn ::std::fmt::Display)),+])
    };
}

/// `t_spans!("key", name = value)`: [`format_spans`] with `t!`'s arguments.
#[macro_export]
macro_rules! t_spans {
    ($key:literal, $($name:ident = $value:expr),+ $(,)?) => {
        $crate::i18n::format_spans($key, &[$((stringify!($name), &$value as &dyn ::std::fmt::Display)),+])
    };
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    fn placeholders(text: &str) -> BTreeSet<&str> {
        text.split('{').skip(1).filter_map(|part| part.split_once('}').map(|(name, _)| name)).collect()
    }

    #[test]
    fn fills_named_arguments() {
        let n = 3;
        assert_eq!(fill("{a} e {b}", &[("a", &"x"), ("b", &n)]), ("x e 3".into(), vec![0..1, 4..5]));
        assert_eq!(fill("sem {nada} {", &[]).0, "sem {nada} {");
    }

    #[test]
    fn resolves_system_locales() {
        assert_eq!(resolve("pt_BR").map(|ix| catalogs()[ix].code), Some("pt-BR"));
        assert_eq!(resolve("pt-PT").map(|ix| catalogs()[ix].code), Some("pt-BR"));
        assert_eq!(resolve("en-AU").map(|ix| catalogs()[ix].code), Some("en-US"));
        assert_eq!(resolve("xx"), None);
    }

    /// No language has keys English lacks, or uses an argument English does
    /// not (it may leave some out, like a 24-hour clock's AM/PM). Missing
    /// keys are allowed: they fall back to English.
    #[test]
    fn languages_match_english() {
        let list = catalogs();
        let english = &list[0];
        for catalog in &list[1..] {
            let mut extra: Vec<_> = catalog.texts.keys().filter(|k| !english.texts.contains_key(*k)).collect();
            extra.sort();
            assert!(extra.is_empty(), "{} has unknown {extra:?}", catalog.code);
            for (key, text) in &catalog.texts {
                let unknown: Vec<_> = placeholders(text)
                    .difference(&placeholders(english.texts[key]))
                    .copied()
                    .collect();
                assert!(unknown.is_empty(), "{}: {key} uses unknown {unknown:?}", catalog.code);
            }
        }
    }

    /// pt-BR is written by hand alongside English and must stay complete.
    #[test]
    fn portuguese_is_complete() {
        let list = catalogs();
        let portuguese = list.iter().find(|c| c.code == "pt-BR").unwrap();
        let mut missing: Vec<_> = list[0].texts.keys().filter(|k| !portuguese.texts.contains_key(*k)).collect();
        missing.sort();
        assert!(missing.is_empty(), "pt-BR lacks {missing:?}");
    }

    /// Every `t!("…")` in the source names a key English has.
    #[test]
    fn every_used_key_exists() {
        fn visit(dir: &std::path::Path, out: &mut Vec<(String, String)>) {
            for entry in std::fs::read_dir(dir).unwrap().flatten() {
                let path = entry.path();
                if path.is_dir() {
                    visit(&path, out);
                } else if path.extension().is_some_and(|e| e == "rs") {
                    let source = std::fs::read_to_string(&path).unwrap();
                    for macro_call in ["t!(\"", "t_spans!(\""] {
                        for (at, _) in source.match_indices(macro_call) {
                            // Not the tail of another macro, like `format!(`.
                            let before = source[..at].chars().next_back();
                            if before.is_some_and(|c| c.is_alphanumeric() || c == '_') {
                                continue;
                            }
                            let rest = &source[at + macro_call.len()..];
                            if let Some((key, _)) = rest.split_once('"') {
                                out.push((path.display().to_string(), key.to_owned()));
                            }
                        }
                    }
                }
            }
        }
        // Every `emerald-*` crate beside this one uses `t!`.
        let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let mut used = Vec::new();
        for entry in std::fs::read_dir(workspace).unwrap().flatten() {
            let src = entry.path().join("src");
            if entry.file_name().to_string_lossy().starts_with("emerald-") && src.is_dir() {
                visit(&src, &mut used);
            }
        }
        assert!(used.len() > 100, "found only {} t! calls; is the workspace layout right?", used.len());
        let english = &catalogs()[0].texts;
        let unknown: Vec<_> = used
            .iter()
            // Skips the examples in doc comments.
            .filter(|(_, key)| key.contains('.') && !english.contains_key(key))
            .collect();
        assert!(unknown.is_empty(), "unknown keys: {unknown:?}");
    }
}
