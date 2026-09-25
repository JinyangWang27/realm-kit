use crate::*;
use std::collections::BTreeSet;

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

fn issue(out: &mut Vec<Diagnostic>, entity: &str, code: &str, message: impl Into<String>) {
    out.push(Diagnostic {
        severity: Severity::Error,
        entity_id: Some(entity.into()),
        code: code.into(),
        message: message.into(),
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

pub fn diagnostics(w: &WorldSpec) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let owner = &w.world.id;
    ids(&mut out, "world", std::iter::once(owner.as_str()));
    if w.world.format_version != FORMAT_VERSION {
        issue(
            &mut out,
            owner,
            "unsupported_version",
            format!(
                "expected format version {FORMAT_VERSION}, got {}",
                w.world.format_version
            ),
        );
    }
    if w.world.name.trim().is_empty() || w.world.player_name.trim().is_empty() {
        issue(
            &mut out,
            owner,
            "empty_name",
            "world and player names must not be empty",
        );
    }
    if w.world.language.trim().is_empty() {
        issue(
            &mut out,
            owner,
            "missing_language",
            "declare the language of the world's player-facing content",
        );
    }
    ids(
        &mut out,
        "location",
        w.locations.iter().map(|v| v.id.as_str()),
    );
    ids(&mut out, "NPC", w.npcs.iter().map(|v| v.id.as_str()));
    ids(
        &mut out,
        "monster",
        w.monsters.iter().map(|v| v.id.as_str()),
    );
    ids(&mut out, "item", w.items.iter().map(|v| v.id.as_str()));
    ids(&mut out, "quest", w.quests.iter().map(|v| v.id.as_str()));
    ids(
        &mut out,
        "dialogue",
        w.dialogues.iter().map(|v| v.id.as_str()),
    );
    ids(&mut out, "flag", w.world.flags.iter().map(String::as_str));
    reference(
        &mut out,
        owner,
        "starting location",
        &w.world.start,
        w.location(&w.world.start).is_some(),
    );
    if w.world.levels.first().map(|l| l.xp) != Some(0)
        || w.world.levels.iter().any(|l| l.hp == 0 || l.attack == 0)
        || w.world
            .levels
            .windows(2)
            .any(|l| l[0].xp >= l[1].xp || l[0].hp > l[1].hp || l[0].attack > l[1].attack)
    {
        issue(&mut out, owner, "invalid_levels", "levels must start at 0 XP, have positive HP/attack, strictly increasing XP and nondecreasing stats");
    }
    let mut placed_monsters = BTreeSet::new();
    for l in &w.locations {
        for exit in l.exits.values() {
            reference(
                &mut out,
                &l.id,
                "exit destination",
                &exit.destination,
                w.location(&exit.destination).is_some(),
            );
            conditions(&mut out, w, &l.id, &exit.requires);
        }
        ids(&mut out, "NPC placement", l.npcs.iter().map(String::as_str));
        for npc in &l.npcs {
            reference(&mut out, &l.id, "NPC", npc, w.npc(npc).is_some());
        }
        for monster in &l.monsters {
            reference(
                &mut out,
                &l.id,
                "monster",
                monster,
                w.monster(monster).is_some(),
            );
            if !placed_monsters.insert(monster.as_str()) {
                issue(
                    &mut out,
                    &l.id,
                    "duplicate_placement",
                    format!("monster {monster} is placed more than once; each ID is one instance"),
                );
            }
        }
    }
    for npc in &w.npcs {
        reference(
            &mut out,
            &npc.id,
            "dialogue",
            &npc.dialogue,
            w.dialogue(&npc.dialogue).is_some(),
        );
        conditions(&mut out, w, &npc.id, &npc.requires);
    }
    for monster in &w.monsters {
        if monster.hp == 0 {
            issue(
                &mut out,
                &monster.id,
                "invalid_stats",
                "monster HP must be positive",
            );
        }
        items(&mut out, w, &monster.id, &monster.loot);
    }
    for quest in &w.quests {
        reference(
            &mut out,
            &quest.id,
            "quest giver",
            &quest.giver,
            w.npc(&quest.giver).is_some(),
        );
        let QuestObjective::Defeat { monster } = &quest.objective;
        reference(
            &mut out,
            &quest.id,
            "quest target",
            monster,
            w.monster(monster).is_some(),
        );
        if w.monster(monster).is_some() && !placed_monsters.contains(monster.as_str()) {
            issue(
                &mut out,
                &quest.id,
                "unplaced_target",
                format!("quest target {monster} has no location"),
            );
        }
        items(&mut out, w, &quest.id, &quest.reward_items);
        for flag in &quest.completion_flags {
            reference(
                &mut out,
                &quest.id,
                "flag",
                flag,
                w.world.flags.contains(flag),
            );
        }
    }
    for dialogue in &w.dialogues {
        ids(
            &mut out,
            "dialogue node",
            dialogue.nodes.iter().map(|n| n.id.as_str()),
        );
        reference(
            &mut out,
            &dialogue.id,
            "start node",
            &dialogue.start,
            dialogue.nodes.iter().any(|n| n.id == dialogue.start),
        );
        for node in &dialogue.nodes {
            for choice in &node.choices {
                if let Some(next) = &choice.next {
                    reference(
                        &mut out,
                        &dialogue.id,
                        "next node",
                        next,
                        dialogue.nodes.iter().any(|n| &n.id == next),
                    );
                }
                conditions(&mut out, w, &dialogue.id, &choice.requires);
                match &choice.effect {
                    Some(
                        DialogueEffect::AcceptQuest { quest }
                        | DialogueEffect::CompleteQuest { quest },
                    ) => reference(
                        &mut out,
                        &dialogue.id,
                        "quest",
                        quest,
                        w.quest(quest).is_some(),
                    ),
                    Some(DialogueEffect::SetFlag { flag }) => reference(
                        &mut out,
                        &dialogue.id,
                        "flag",
                        flag,
                        w.world.flags.contains(flag),
                    ),
                    None => {}
                }
            }
        }
    }
    for (key, variants) in [("attack", &w.narrative.attack), ("hurt", &w.narrative.hurt)] {
        if variants.is_empty() {
            issue(
                &mut out,
                key,
                "empty_variants",
                "at least one narrative variant is required",
            );
        }
        for text in variants {
            template(&mut out, key, &text.0, &["attacker", "target", "damage"]);
        }
    }
    template(&mut out, "victory", &w.narrative.victory.0, &["target"]);
    if w.narrative.death.trim().is_empty() {
        issue(
            &mut out,
            "death",
            "empty_narrative",
            "death text is required",
        );
    }
    out
}
