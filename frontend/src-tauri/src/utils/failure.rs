//! Failures the interface can render in its own language.
//!
//! Most backend failures never reach a screen and stay plain prose. The ones
//! that do need to arrive in the interface language, and the backend has no
//! locale of its own - so it sends the prose plus the catalog key for it, and
//! the frontend picks. That is the same bargain `services::evolution::msg`
//! strikes for progress messages; this is the failure-shaped half of it.
//!
//! The envelope is a JSON object encoded as a string, because the error channel
//! is `Result<_, String>` throughout (`api_request` funnels every local-service
//! call into one). Reshaping ~35 command signatures to carry a struct would be
//! a far larger change for no additional safety: the frontend is untyped
//! anyway, so the guarantee that matters - that a code is understood before it
//! is trusted - lives in the frontend's `describeError`, which falls back to
//! the prose and then to a generic translated message.

use serde_json::{json, Value};

/// A failure whose prose and catalog key travel together.
///
/// `code` is the frontend's catalog key, not a domain identifier: a key is an
/// identifier rather than locale knowledge, so the backend still holds no
/// language of its own, and one name for one thing cannot drift the way a
/// lookup table can.
pub(crate) fn failed(code: &str, text: impl Into<String>) -> String {
    envelope_text(code, json!({}), text)
}

/// [`failed`], for prose with placeholders to fill.
pub(crate) fn failed_with(code: &str, params: Value, text: impl Into<String>) -> String {
    envelope_text(code, params, text)
}

/// The wire shape: the prose, the catalog key for it, and the values that
/// entry interpolates.
///
/// Shared with [`services::community::utils::field`], which puts the same
/// envelope in a successful response instead of an error - the interface
/// renders both the same way, so they must not be built from two copies of
/// this object literal.
pub(crate) fn envelope(code: &str, params: Value, text: impl Into<String>) -> Value {
    json!({ "code": code, "params": params, "message": text.into() })
}

fn envelope_text(code: &str, params: Value, text: impl Into<String>) -> String {
    envelope(code, params, text).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use proc_macro2::{Delimiter, Group, TokenStream, TokenTree};
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::{Path, PathBuf};

    const LOCALES: &[&str] = &["en", "zh-CN"];

    fn src_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
    }

    fn locales_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../src/i18n/locales")
    }

    fn rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                rs_files(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(path);
            }
        }
    }

    /// Every string literal in a source file, by way of the real tokenizer.
    ///
    /// A hand-written scanner was tried first and lost codes silently: a char
    /// literal holding a quote (`trim_matches('"')`, in `team_service.rs`)
    /// desynchronised the walk, and every key after it in that file went
    /// unexamined while the test still reported success. The tokenizer knows
    /// the language's lexical rules - comments, escapes, char literals, raw
    /// strings - so it cannot make that mistake.
    fn literals(source: &str, path: &Path) -> Vec<String> {
        let stream = source
            .parse::<TokenStream>()
            .unwrap_or_else(|e| panic!("cannot tokenize {}: {e}", path.display()));
        let mut out = Vec::new();
        collect(stream, &mut out);
        out
    }

    /// Whether a group is a doc comment.
    ///
    /// Comments never reach the tokenizer as trivia; a doc comment arrives as
    /// `#[doc = ".."]`, and its prose is free to quote a key that the code does
    /// not send. Those groups are dropped before their literals are read.
    fn is_doc_attribute(group: &Group) -> bool {
        group.delimiter() == Delimiter::Bracket
            && matches!(
                group.stream().into_iter().next(),
                Some(TokenTree::Ident(ident)) if ident == "doc"
            )
    }

    fn collect(stream: TokenStream, out: &mut Vec<String>) {
        for token in stream {
            match token {
                TokenTree::Group(group) => {
                    if !is_doc_attribute(&group) {
                        collect(group.stream(), out);
                    }
                }
                TokenTree::Literal(literal) => {
                    let text = literal.to_string();
                    if let Some(inner) = text.strip_prefix('"').and_then(|t| t.strip_suffix('"')) {
                        out.push(inner.to_string());
                    }
                }
                TokenTree::Ident(_) | TokenTree::Punct(_) => {}
            }
        }
    }

    /// Whether a literal has the shape of a catalog key: `a.b` or deeper, each
    /// segment starting lowercase. It is a filter, not a definition - the
    /// catalogs remain the authority - so it only has to be loose enough not to
    /// drop a real key and tight enough to drop a file name.
    fn is_catalog_key(candidate: &str) -> bool {
        let mut parts = candidate.split('.');
        let Some(head) = parts.next() else {
            return false;
        };
        let tail: Vec<&str> = parts.collect();
        let named = |part: &str| {
            part.chars().next().is_some_and(|c| c.is_ascii_lowercase())
                && part.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        };
        head.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
            && !head.is_empty()
            && !tail.is_empty()
            && tail.iter().all(|part| named(part))
    }

    /// Literals in the backend that look like a catalog key but are not one.
    ///
    /// The scan takes *every* dotted string literal rather than the arguments
    /// of the calls that send one. The key is written in three shapes - a
    /// `failed(..)` argument, a `"code"` field, and the first element of a
    /// `("key", json!(..), text)` tuple - and the third cannot be told from a
    /// plain tuple by any rule that is not itself a guess. A guess silently
    /// under-covers, which is the one failure mode this test exists to prevent.
    ///
    /// So the exceptions are named instead. A new one fails the test and gets
    /// looked at, which is the outcome to want.
    const NOT_KEYS: &[&str] = &[
        "agents.json",
        "download.zip",
        "explorer.exe",
        "msg.rs",
        "os.system",
        "pickle.load",
        "settings.json",
        "shutil.rmtree",
        "skill.md",
        "skill.zip",
        "skills.csv",
        "skills.json",
        "test.db",
        "uploaded.zip",
        "yaml.load",
    ];

    /// Every key the backend hands to the frontend, in any of the shapes it
    /// writes one in.
    fn sent_codes() -> BTreeSet<String> {
        let mut files = Vec::new();
        rs_files(&src_root(), &mut files);
        let mut codes = BTreeSet::new();
        for path in files {
            let Ok(raw) = fs::read_to_string(&path) else {
                continue;
            };
            for literal in literals(&raw, &path) {
                if is_catalog_key(&literal) && !NOT_KEYS.contains(&literal.as_str()) {
                    codes.insert(literal);
                }
            }
        }
        codes
    }

    fn catalogs() -> Vec<(&'static str, Value)> {
        LOCALES
            .iter()
            .map(|locale| {
                let dir = locales_root().join(locale);
                let mut merged = serde_json::Map::new();
                let mut names: Vec<_> = fs::read_dir(&dir)
                    .unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()))
                    .flatten()
                    .map(|e| e.path())
                    .filter(|p| p.extension().is_some_and(|x| x == "json"))
                    .collect();
                names.sort();
                for path in names {
                    let raw = fs::read_to_string(&path)
                        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
                    let parsed: Value = serde_json::from_str(&raw)
                        .unwrap_or_else(|e| panic!("cannot parse {}: {e}", path.display()));
                    if let Value::Object(map) = parsed {
                        merged.extend(map);
                    }
                }
                (*locale, Value::Object(merged))
            })
            .collect()
    }

    fn lookup<'a>(catalog: &'a Value, code: &str) -> Option<&'a Value> {
        let mut node = catalog;
        for part in code.split('.') {
            node = node.get(part)?;
        }
        Some(node)
    }

    /// A code with no catalog entry renders as the untranslated prose, which on
    /// an English screen means Chinese. The frontend cannot tell the difference
    /// at runtime - it falls back by design - so the typo has to be caught here
    /// or not at all.
    #[test]
    fn sent_codes_have_catalog_entries() {
        let codes = sent_codes();
        assert!(
            codes.len() > 100,
            "the source scan found only {} codes - it has probably stopped matching",
            codes.len()
        );
        for (locale, catalog) in catalogs() {
            for code in &codes {
                let entry = lookup(&catalog, code);
                assert!(
                    entry.is_some_and(|v| v.is_string()),
                    "{locale} has no text for `{code}`"
                );
            }
        }
    }
}
