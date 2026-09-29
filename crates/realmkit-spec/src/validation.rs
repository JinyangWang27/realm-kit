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
        if l.safe && w.combat().is_none() {
            issue(
                &mut out,
                &l.id,
                "combat_disabled",
                "this world has no combat block, so there is nothing to rest from",
            );
        }
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
            stats(&mut out, &character.id, &profile.stats);
            crit(&mut out, &character.id, profile.basic_crit);
            items(&mut out, w, &character.id, &profile.loot);
            let usable = profile
                .skills
                .iter()
                .filter_map(|id| w.skill(id))
                .filter(|s| s.level <= profile.level);
            if profile.level == 0 {
                issue(
                    &mut out,
                    &character.id,
                    "invalid_level",
                    "a combat profile's level is 1 or more",
                );
            }
            if let Some(group) = &profile.group {
                reference(
                    &mut out,
                    &character.id,
                    "group",
                    group,
                    w.group(group).is_some(),
                );
            }
            if let Some(combat) = w.combat() {
                ids(
                    &mut out,
                    "skill reference",
                    profile.skills.iter().map(String::as_str),
                );
                for id in &profile.skills {
                    reference(&mut out, &character.id, "skill", id, w.skill(id).is_some());
                }
                usable_skills(
                    &mut out,
                    combat,
                    &character.id,
                    profile.basic_channel,
                    usable.map(|s| (s, &profile.stats)),
                    &profile.stats,
                );
            }
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
                // Yielders stop at 1 HP, so a yielding group's members never die.
                let yields = target
                    .and_then(|c| c.combat.as_ref()?.group.as_deref())
                    .and_then(|g| w.group(g))
                    .is_some_and(|g| g.yield_share.is_some());
                if yields {
                    issue(
                        &mut out,
                        &quest.id,
                        "invalid_target",
                        format!("quest target {character} yields instead of being defeated"),
                    );
                }
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
        combat_rules(&mut out, w, owner, combat);
    }
    out
}

fn stats(out: &mut Vec<Diagnostic>, owner: &str, stats: &Stats) {
    let values = [
        stats.hp,
        stats.mp,
        stats.patk,
        stats.pdef,
        stats.satk,
        stats.sdef,
        stats.speed,
    ];
    if stats.hp == 0 || stats.speed == 0 || values.iter().any(|v| *v > STAT_BOUND) {
        issue(
            out,
            owner,
            "invalid_stats",
            format!("stats must be at most {STAT_BOUND}, with HP and speed at least 1"),
        );
    }
}

fn crit(out: &mut Vec<Diagnostic>, owner: &str, crit: Option<Crit>) {
    if crit.is_some_and(|c| {
        !(1..=100).contains(&c.chance_percent) || !(101..=1_000).contains(&c.multiplier_percent)
    }) {
        issue(
            out,
            owner,
            "invalid_crit",
            "a crit needs a chance from 1 to 100 percent and a multiplier from 101 to 1,000 percent",
        );
    }
}

fn share(out: &mut Vec<Diagnostic>, owner: &str, value: u32) {
    if value > 100 {
        issue(
            out,
            owner,
            "invalid_share",
            "a cross share is a percentage from 0 to 100",
        );
    }
}

/// Every skill a character can use, and its basic attack, needs attack in its
/// channel and MP enough to use it once at the stats it unlocks with.
fn usable_skills<'a>(
    out: &mut Vec<Diagnostic>,
    combat: &Combat,
    owner: &str,
    basic: Channel,
    skills: impl Iterator<Item = (&'a Skill, &'a Stats)>,
    first: &Stats,
) {
    if first.combined(basic, combat.cross_share, false) == 0 {
        issue(
            out,
            owner,
            "no_attack",
            "the basic attack needs attack in its channel",
        );
    }
    for (skill, stats) in skills {
        let share = skill.cross_share.unwrap_or(combat.cross_share);
        if stats.combined(skill.channel, share, false) == 0 {
            issue(
                out,
                owner,
                "no_attack",
                format!("{} needs attack in its channel", skill.id),
            );
        }
        // Rage accumulates during a fight, so only MP has a ceiling to check.
        if skill.resource == Resource::Mp && skill.cost > stats.mp {
            issue(
                out,
                owner,
                "unaffordable_skill",
                format!(
                    "{} costs more MP than the character has when it can use it",
                    skill.id
                ),
            );
        }
    }
}

fn combat_rules(out: &mut Vec<Diagnostic>, w: &WorldSpec, owner: &str, combat: &Combat) {
    if combat.special_name.trim().is_empty() {
        issue(
            out,
            owner,
            "empty_name",
            "name the special damage channel, for example magic or 内力",
        );
    }
    share(out, owner, combat.cross_share);
    crit(out, owner, combat.player_basic_crit);
    let timeline = combat.timeline;
    if !(1..=ACTION_COST_BOUND).contains(&timeline.action_cost)
        || timeline.speed_cap == 0
        || timeline.speed_cap > STAT_BOUND
    {
        issue(
            out,
            owner,
            "invalid_timeline",
            format!("action cost must be from 1 to {ACTION_COST_BOUND} and the speed cap from 1 to {STAT_BOUND}"),
        );
    }
    // Unused constants are harmless, so they only warn.
    let uses = |resource| {
        combat
            .skills
            .iter()
            .any(|s| s.resource == resource && s.cost > 0)
    };
    let r = combat.resources;
    let rage = r.rage_per_action > 0 || r.rage_per_max_hp > 0;
    let mut warn = |entity: &str, code: &str, message: String| {
        out.push(Diagnostic {
            severity: Severity::Warning,
            entity_id: Some(entity.into()),
            code: code.into(),
            message,
        })
    };
    for (set, used, what) in [
        (
            r.mp_regen_percent > 0,
            uses(Resource::Mp),
            "MP regeneration",
        ),
        (rage, uses(Resource::Rage), "rage"),
    ] {
        if set && !used {
            warn(
                owner,
                "unused_resource",
                format!("{what} is configured but no skill spends it"),
            );
        }
    }
    // Rage starts at 0 in every fight, so without a source a rage cost is never met.
    if !rage {
        for skill in combat
            .skills
            .iter()
            .filter(|s| s.resource == Resource::Rage && s.cost > 0)
        {
            let message = "costs rage, but this world has no rage_per_action or rage_per_max_hp";
            warn(&skill.id, "unusable_skill", message.into());
        }
    }
    let levels = &combat.levels;
    for level in levels {
        stats(out, owner, &level.stats);
    }
    let fields = |s: &Stats| [s.hp, s.mp, s.patk, s.pdef, s.satk, s.sdef, s.speed];
    if levels.first().map(|l| l.xp) != Some(0)
        || levels.windows(2).any(|l| {
            l[0].xp >= l[1].xp
                || fields(&l[0].stats)
                    .iter()
                    .zip(fields(&l[1].stats))
                    .any(|(a, b)| *a > b)
        })
    {
        issue(
            out,
            owner,
            "invalid_levels",
            "levels must start at 0 XP, with strictly increasing XP and stats that never fall",
        );
    }
    ids(out, "group", combat.groups.iter().map(|g| g.id.as_str()));
    for group in &combat.groups {
        if group.yield_share.is_some_and(|s| !(1..=100).contains(&s)) {
            issue(
                out,
                &group.id,
                "invalid_share",
                "a yield share is a percentage from 1 to 100",
            );
        }
        for flag in group.victory_flags.iter().chain(&group.defeat_flags) {
            reference(out, &group.id, "flag", flag, w.world.flags.contains(flag));
        }
    }
    ids(out, "skill", combat.skills.iter().map(|s| s.id.as_str()));
    for skill in &combat.skills {
        if !(POWER_BOUNDS.0..=POWER_BOUNDS.1).contains(&skill.power) {
            issue(
                out,
                &skill.id,
                "invalid_power",
                format!(
                    "skill power must be from {} to {}",
                    POWER_BOUNDS.0, POWER_BOUNDS.1
                ),
            );
        }
        if skill.level == 0 {
            issue(
                out,
                &skill.id,
                "invalid_level",
                "skills unlock at level 1 or later",
            );
        }
        if !(TIME_BOUNDS.0..=TIME_BOUNDS.1).contains(&skill.time) {
            issue(
                out,
                &skill.id,
                "invalid_time",
                format!(
                    "action time must be from {} to {} percent",
                    TIME_BOUNDS.0, TIME_BOUNDS.1
                ),
            );
        }
        if let Some(value) = skill.cross_share {
            share(out, &skill.id, value);
        }
        crit(out, &skill.id, skill.crit);
        if skill.name.trim().is_empty() {
            issue(out, &skill.id, "empty_name", "skills need a name");
        }
        template(
            out,
            &skill.id,
            &skill.text.0,
            &["attacker", "target", "damage"],
        );
    }
    ids(
        out,
        "skill reference",
        combat.player_skills.iter().map(String::as_str),
    );
    for id in &combat.player_skills {
        reference(out, owner, "skill", id, w.skill(id).is_some());
        if w.skill(id).is_some_and(|s| s.level > levels.len()) {
            issue(
                out,
                id,
                "invalid_level",
                "the player never reaches this skill's level",
            );
        }
    }
    if let Some(first) = levels.first() {
        let unlocked = combat
            .player_skills
            .iter()
            .filter_map(|id| w.skill(id))
            .filter_map(|s| Some((s, &levels.get(s.level.checked_sub(1)?)?.stats)));
        usable_skills(
            out,
            combat,
            &w.world.player,
            combat.player_basic_channel,
            unlocked,
            &first.stats,
        );
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
