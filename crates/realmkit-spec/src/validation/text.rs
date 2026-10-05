//! Language overlays: complete, exact, and well formed in their language.

use super::*;

/// A language tag: ASCII letters, digits and `-`, as in `zh-Hans`. It names
/// a file, so nothing else may appear.
pub fn language_tag(tag: &str) -> bool {
    !tag.is_empty() && tag.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-')
}

/// Every listed overlay is another language, holds exactly the base prose
/// keys, and reads as valid content with its text in place.
pub(super) fn translations(out: &mut Vec<Diagnostic>, w: &WorldSpec) {
    let tags = &w.world.translations;
    if tags.is_empty() {
        return;
    }
    let base_texts = w.texts();
    let mut base = w.clone();
    base.world.translations.clear();
    let found = diagnostics(&base);
    let mut seen = BTreeSet::new();
    for tag in tags {
        if !language_tag(tag) || *tag == w.world.language || !seen.insert(tag) {
            issue(
                out,
                tag,
                "invalid_translation",
                "a translation is another language than the package's, listed once, by a tag of ASCII letters, digits and '-'",
            );
            continue;
        }
        let Some(overlay) = w.translations.get(tag) else {
            issue(
                out,
                tag,
                "missing_translation",
                format!("no text/{tag}.json was loaded"),
            );
            continue;
        };
        for key in base_texts.keys().filter(|k| !overlay.contains_key(*k)) {
            issue(
                out,
                tag,
                "missing_translation",
                format!("no text for {key}"),
            );
        }
        for key in overlay.keys().filter(|k| !base_texts.contains_key(*k)) {
            issue(
                out,
                tag,
                "unused_translation",
                format!("{key} names no text in this package"),
            );
        }
        // Templates and names are checked again in this language; what the
        // base already reports is not repeated.
        for mut d in diagnostics(&w.translated(tag, overlay)) {
            if !found.contains(&d) {
                d.message = format!("in {tag}: {}", d.message);
                out.push(d);
            }
        }
    }
}
