//! Player-facing prose by stable key: what the rules revision leaves out.

use crate::*;
use serde_json::Value;

/// The fields that hold prose, wherever they appear. Every other string in a
/// package is an ID, a tag or a reference, so it belongs to the rules.
const PROSE: [&str; 16] = [
    "name",
    "description",
    "text",
    "blocked_text",
    "special_name",
    "introduction",
    "progress",
    "completion",
    "source",
    "facts",
    "attack",
    "hurt",
    "victory",
    "death",
    "format",
    "clock",
];

/// Calls `f` with the key and text of every prose string, depth first. A key
/// is the JSON path joined by `.`: an array element with an `id` is named by
/// it, any other by its index, so `dialogues.pell.nodes.greeting.choices.0.text`.
// ponytail: an element ID made of digits could share a key with an index;
// validate keys as unique if authors ever do that.
fn visit(
    value: &mut Value,
    key: &mut Vec<String>,
    prose: bool,
    f: &mut impl FnMut(String, &mut String),
) {
    match value {
        Value::String(text) if prose => f(key.join("."), text),
        Value::Array(items) => {
            for (index, item) in items.iter_mut().enumerate() {
                let name = item.get("id").and_then(Value::as_str);
                key.push(name.map_or_else(|| index.to_string(), str::to_owned));
                visit(item, key, prose, f);
                key.pop();
            }
        }
        Value::Object(fields) => {
            for (name, field) in fields {
                key.push(name.clone());
                visit(field, key, PROSE.contains(&name.as_str()), f);
                key.pop();
            }
        }
        _ => {}
    }
}

/// Every prose string in `value`, emptied. An emptied optional field is
/// dropped, so adding one is a prose edit too; `blocked_text` stays, since
/// whether a choice has one decides whether it is listed. Lists keep their
/// length: how many facts, readings or variants there are is a rule.
fn strip(value: &mut Value) {
    visit(value, &mut Vec::new(), false, &mut |_, text| text.clear());
    fn drop_empty(value: &mut Value) {
        match value {
            Value::Array(items) => items.iter_mut().for_each(drop_empty),
            Value::Object(fields) => {
                fields.retain(|name, field| {
                    name == "blocked_text" || !PROSE.contains(&name.as_str()) || *field != ""
                });
                fields.values_mut().for_each(drop_empty);
            }
            _ => {}
        }
    }
    drop_empty(value);
}

impl WorldSpec {
    /// Identifies this package's rules: everything but its prose and its
    /// language. Saves bind to it, so a typo fix keeps them loadable, while
    /// any edit to IDs, numbers, conditions or effects makes them incompatible.
    // ponytail: 64-bit FNV-1a over canonical JSON detects edits, not tampering;
    // switch to SHA-256 if revisions must be adversarially unique.
    pub fn revision(&self) -> String {
        let mut value = serde_json::to_value(self).expect("world specs always serialize");
        strip(&mut value);
        value["world"].as_object_mut().unwrap().remove("language");
        let bytes = serde_json::to_vec(&value).expect("values always serialize");
        let hash = bytes.iter().fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3)
        });
        format!("fnv1a64:{hash:016x}")
    }

    /// Every player-facing text by its stable key, as a language overlay
    /// lists them.
    pub fn texts(&self) -> BTreeMap<String, String> {
        let mut value = serde_json::to_value(self).expect("world specs always serialize");
        let mut texts = BTreeMap::new();
        visit(&mut value, &mut Vec::new(), false, &mut |key, text| {
            texts.insert(key, text.clone());
        });
        texts
    }
}
