//! Content checks, one file per domain, and the helpers they share.

use crate::*;
use std::collections::BTreeSet;

mod combat;
mod equipment;
mod progression;
mod story;
mod world;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: Severity,
    pub entity_id: Option<Id>,
    pub code: String,
    pub message: String,
}

#[derive(Debug, thiserror::Error)]
pub enum SpecError {
    #[error("unsupported package format {}: this RealmKit reads Format {FORMAT_VERSION} only; older packages are not migrated, so convert the package to Format {FORMAT_VERSION} (see docs/world-format.md)", found.map_or("(missing)".into(), |v| v.to_string()))]
    UnsupportedFormat { found: Option<u64> },
    #[error("world validation failed: {0:?}")]
    Validation(Vec<Diagnostic>),
    #[error("{path}: {source}")]
    Io {
        path: std::path::PathBuf,
        source: std::io::Error,
    },
    #[error("{path}: {source}")]
    Json {
        path: std::path::PathBuf,
        source: serde_json::Error,
    },
}

pub fn diagnostics(w: &WorldSpec) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    world::header(&mut out, w);
    let placed = world::locations(&mut out, w);
    world::player(&mut out, w, &placed);
    world::characters(&mut out, w);
    story::quests(&mut out, w, &placed);
    story::dialogues(&mut out, w);
    if let Some(combat) = w.combat() {
        combat::rules(&mut out, w, combat);
    }
    out
}

fn issue(out: &mut Vec<Diagnostic>, entity: &str, code: &str, message: impl Into<String>) {
    push(out, Severity::Error, entity, code, message.into());
}

fn warn(out: &mut Vec<Diagnostic>, entity: &str, code: &str, message: impl Into<String>) {
    push(out, Severity::Warning, entity, code, message.into());
}

fn push(out: &mut Vec<Diagnostic>, severity: Severity, entity: &str, code: &str, message: String) {
    out.push(Diagnostic {
        severity,
        entity_id: Some(entity.into()),
        code: code.into(),
        message,
    });
}

fn ids<'a>(out: &mut Vec<Diagnostic>, kind: &str, values: impl Iterator<Item = &'a str>) {
    let mut seen = BTreeSet::new();
    for id in values {
        if id.is_empty()
            || !id
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
        {
            issue(
                out,
                id,
                "invalid_id",
                format!("{kind} ID must contain only ASCII letters, digits, '_' or '-': {id:?}"),
            );
        }
        if !seen.insert(id) {
            issue(
                out,
                id,
                "duplicate_id",
                format!("duplicate {kind} ID: {id}"),
            );
        }
    }
}

fn reference(out: &mut Vec<Diagnostic>, owner: &str, kind: &str, target: &str, exists: bool) {
    if !exists {
        issue(
            out,
            owner,
            "missing_reference",
            format!("{kind} does not exist: {target}"),
        );
    }
}

fn conditions(out: &mut Vec<Diagnostic>, w: &WorldSpec, owner: &str, values: &[Condition]) {
    for condition in values {
        match condition {
            Condition::Flag { flag } => {
                reference(out, owner, "flag", flag, w.world.flags.contains(flag))
            }
            Condition::Quest { quest, .. } => {
                reference(out, owner, "quest", quest, w.quest(quest).is_some())
            }
            Condition::Technique { technique, rank } => {
                let known = w.technique(technique);
                reference(out, owner, "technique", technique, known.is_some());
                if known.is_some_and(|t| *rank == 0 || *rank > t.ranks.len()) {
                    issue(
                        out,
                        owner,
                        "invalid_rank",
                        format!("{technique} has no rank {rank}"),
                    );
                }
            }
        }
    }
}

fn items(out: &mut Vec<Diagnostic>, w: &WorldSpec, owner: &str, stacks: &[ItemStack]) {
    for stack in stacks {
        reference(
            out,
            owner,
            "item",
            &stack.item,
            w.item(&stack.item).is_some(),
        );
        if stack.quantity == 0 {
            issue(
                out,
                owner,
                "invalid_quantity",
                "item quantity must be positive",
            );
        }
        // Each wearable one becomes its own piece, so keep grants small.
        let wearable = w.item(&stack.item).is_some_and(|i| i.equipment.is_some());
        if wearable && stack.quantity > GEAR_STACK_BOUND {
            issue(
                out,
                owner,
                "invalid_quantity",
                format!("at most {GEAR_STACK_BOUND} pieces of equipment are granted at once"),
            );
        }
    }
}

fn template(out: &mut Vec<Diagnostic>, owner: &str, text: &str, allowed: &[&str]) {
    let mut rest = text;
    let valid = loop {
        match rest.find('{') {
            Some(open) => {
                if rest[..open].contains('}') {
                    break false;
                }
                let tail = &rest[open + 1..];
                let Some(close) = tail.find('}') else {
                    break false;
                };
                if !allowed.contains(&&tail[..close]) {
                    break false;
                }
                rest = &tail[close + 1..];
            }
            None => break !rest.contains('}') && !text.trim().is_empty(),
        }
    };
    if !valid {
        issue(
            out,
            owner,
            "invalid_template",
            format!(
                "malformed template; allowed placeholders: {}",
                allowed.join(", ")
            ),
        );
    }
}
