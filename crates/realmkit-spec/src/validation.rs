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
    #[error("unsupported package format {}: this RealmKit reads Format {FORMAT_VERSION} only; Format 1 packages are no longer supported, so convert the package to Format {FORMAT_VERSION}", found.map_or("(missing)".into(), |v| v.to_string()))]
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
    if w.world.name.trim().is_empty() || w.characters.iter().any(|c| c.name.trim().is_empty()) {
        issue(
            &mut out,
            owner,
            "empty_name",
            "world and character names must not be empty",
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
    ids(
        &mut out,
        "character",
        w.characters.iter().map(|v| v.id.as_str()),
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
    let mut placed = BTreeSet::new();
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
        ids(
            &mut out,
            "character placement",
            l.characters.iter().map(String::as_str),
        );
        for id in &l.characters {
            let character = w.character(id);
            reference(&mut out, &l.id, "character", id, character.is_some());
            // A fighter's defeat is permanent, so it can be in one place only.
            if !placed.insert(id.as_str()) && character.is_some_and(|c| c.combat.is_some()) {
                issue(
                    &mut out,
                    &l.id,
                    "duplicate_placement",
                    format!("character {id} can fight, so it is one instance and may be placed only once"),
                );
            }
        }
    }
    // The player's numbers come from the level table, not from a profile.
    match w.character(&w.world.player) {
        None => reference(&mut out, owner, "player character", &w.world.player, false),
        Some(player) => {
            if player.combat.is_some()
                || player.dialogue.is_some()
                || placed.contains(player.id.as_str())
            {
                issue(
                    &mut out,
                    &player.id,
                    "invalid_player",
                    "the player character must have no combat profile or dialogue and be placed at no location",
                );
            }
        }
    }
    for character in &w.characters {
        if let Some(dialogue) = &character.dialogue {
            reference(
                &mut out,
                &character.id,
                "dialogue",
                dialogue,
                w.dialogue(dialogue).is_some(),
            );
        }
        conditions(&mut out, w, &character.id, &character.requires);
        if let Some(profile) = &character.combat {
            if profile.hp == 0 {
                issue(
                    &mut out,
                    &character.id,
                    "invalid_stats",
                    "combat HP must be positive",
                );
            }
            items(&mut out, w, &character.id, &profile.loot);
            if w.combat().is_none() {
                issue(
                    &mut out,
                    &character.id,
                    "combat_disabled",
                    "this world has no combat block, so no character can fight",
                );
            }
        }
    }
    for quest in &w.quests {
        reference(
            &mut out,
            &quest.id,
            "quest giver",
            &quest.giver,
            w.character(&quest.giver).is_some(),
        );
        if w.character(&quest.giver)
            .is_some_and(|c| c.dialogue.is_none())
        {
            issue(
                &mut out,
                &quest.id,
                "invalid_giver",
                format!("quest giver {} has no dialogue", quest.giver),
            );
        }
        match &quest.objective {
            QuestObjective::Defeat { character } => {
                let target = w.character(character);
                reference(
                    &mut out,
                    &quest.id,
                    "quest target",
                    character,
                    target.is_some(),
                );
                if target.is_some_and(|c| c.combat.is_none()) {
                    issue(
                        &mut out,
                        &quest.id,
                        "invalid_target",
                        format!("quest target {character} has no combat profile"),
                    );
                }
                if target.is_some() && !placed.contains(character.as_str()) {
                    issue(
                        &mut out,
                        &quest.id,
                        "unplaced_target",
                        format!("quest target {character} has no location"),
                    );
                }
                if w.combat().is_none() {
                    issue(
                        &mut out,
                        &quest.id,
                        "combat_disabled",
                        "this world has no combat block, so no quest can require a defeat",
                    );
                }
            }
            QuestObjective::Flag { flag } => reference(
                &mut out,
                &quest.id,
                "flag",
                flag,
                w.world.flags.contains(flag),
            ),
        }
        if quest.reward_xp > 0 && w.combat().is_none() {
            issue(
                &mut out,
                &quest.id,
                "combat_disabled",
                "this world has no combat block, so there is no XP to reward",
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
    if let Some(combat) = w.combat() {
        combat_rules(&mut out, owner, combat);
    }
    out
}

fn combat_rules(out: &mut Vec<Diagnostic>, owner: &str, combat: &Combat) {
    let levels = &combat.levels;
    if levels.first().map(|l| l.xp) != Some(0)
        || levels.iter().any(|l| l.hp == 0 || l.attack == 0)
        || levels
            .windows(2)
            .any(|l| l[0].xp >= l[1].xp || l[0].hp > l[1].hp || l[0].attack > l[1].attack)
    {
        issue(out, owner, "invalid_levels", "levels must start at 0 XP, have positive HP/attack, strictly increasing XP and nondecreasing stats");
    }
    let narrative = &combat.narrative;
    for (key, variants) in [("attack", &narrative.attack), ("hurt", &narrative.hurt)] {
        if variants.is_empty() {
            issue(
                out,
                key,
                "empty_variants",
                "at least one narrative variant is required",
            );
        }
        for text in variants {
            template(out, key, &text.0, &["attacker", "target", "damage"]);
        }
    }
    template(out, "victory", &narrative.victory.0, &["target"]);
    if narrative.death.trim().is_empty() {
        issue(out, "death", "empty_narrative", "death text is required");
    }
}
