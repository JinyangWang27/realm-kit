//! Quests and dialogue.

use super::*;

pub(super) fn quests(out: &mut Vec<Diagnostic>, w: &WorldSpec, placed: &BTreeSet<&str>) {
    for quest in &w.quests {
        reference(
            out,
            &quest.id,
            "quest giver",
            &quest.giver,
            w.character(&quest.giver).is_some(),
        );
        if w.character(&quest.giver)
            .is_some_and(|c| c.dialogue.is_none())
        {
            issue(
                out,
                &quest.id,
                "invalid_giver",
                format!("quest giver {} has no dialogue", quest.giver),
            );
        }
        match &quest.objective {
            QuestObjective::Defeat { character } => {
                defeat_target(out, w, &quest.id, character, placed)
            }
            QuestObjective::Flag { flag } => {
                reference(out, &quest.id, "flag", flag, w.world.flags.contains(flag))
            }
        }
        if quest.reward_xp > 0 && w.combat().is_none() {
            issue(
                out,
                &quest.id,
                "combat_disabled",
                "this world has no combat block, so there is no XP to reward",
            );
        }
        items(out, w, &quest.id, &quest.reward_items);
        for grant in &quest.reward_techniques {
            progression::technique_grant(out, w, &quest.id, grant);
        }
        for flag in &quest.completion_flags {
            reference(out, &quest.id, "flag", flag, w.world.flags.contains(flag));
        }
    }
}

/// A defeat objective names a placed fighter that can actually die.
fn defeat_target(
    out: &mut Vec<Diagnostic>,
    w: &WorldSpec,
    quest: &str,
    character: &str,
    placed: &BTreeSet<&str>,
) {
    let target = w.character(character);
    reference(out, quest, "quest target", character, target.is_some());
    // Yielders stop at 1 HP, so a yielding group's members never die.
    let yields = target
        .and_then(|c| c.combat.as_ref()?.group.as_deref())
        .and_then(|g| w.group(g))
        .is_some_and(|g| g.yield_share.is_some());
    if yields {
        issue(
            out,
            quest,
            "invalid_target",
            format!("quest target {character} yields instead of being defeated"),
        );
    }
    if target.is_some_and(|c| c.combat.is_none()) {
        issue(
            out,
            quest,
            "invalid_target",
            format!("quest target {character} has no combat profile"),
        );
    }
    if target.is_some() && !placed.contains(character) {
        issue(
            out,
            quest,
            "unplaced_target",
            format!("quest target {character} has no location"),
        );
    }
    if w.combat().is_none() {
        issue(
            out,
            quest,
            "combat_disabled",
            "this world has no combat block, so no quest can require a defeat",
        );
    }
}

pub(super) fn dialogues(out: &mut Vec<Diagnostic>, w: &WorldSpec) {
    for dialogue in &w.dialogues {
        ids(
            out,
            "dialogue node",
            dialogue.nodes.iter().map(|n| n.id.as_str()),
        );
        reference(
            out,
            &dialogue.id,
            "start node",
            &dialogue.start,
            dialogue.nodes.iter().any(|n| n.id == dialogue.start),
        );
        for choice in dialogue.nodes.iter().flat_map(|n| &n.choices) {
            if let Some(next) = &choice.next {
                reference(
                    out,
                    &dialogue.id,
                    "next node",
                    next,
                    dialogue.nodes.iter().any(|n| &n.id == next),
                );
            }
            condition(out, w, &dialogue.id, choice.requires.as_ref());
            for effect in &choice.effects {
                self::effect(out, w, &dialogue.id, effect);
            }
        }
    }
}

pub(super) fn effect(out: &mut Vec<Diagnostic>, w: &WorldSpec, owner: &str, effect: &Effect) {
    match effect {
        Effect::AcceptQuest { quest } | Effect::CompleteQuest { quest } => {
            reference(out, owner, "quest", quest, w.quest(quest).is_some())
        }
        Effect::SetFlag { flag } => {
            reference(out, owner, "flag", flag, w.world.flags.contains(flag))
        }
        Effect::GrantItems { items: stacks } => items(out, w, owner, stacks),
        Effect::TakeItems { items } => {
            for stack in items {
                counted(out, w, owner, &stack.item, stack.quantity);
            }
        }
        Effect::GrantTechnique(grant) => {
            progression::technique_grant(out, w, owner, grant);
            // A choice can be taken again; teaching a rank is idempotent,
            // XP would not be. One-time XP comes from quest rewards.
            if grant.xp > 0 {
                issue(
                    out,
                    owner,
                    "repeatable_reward",
                    "a dialogue grant may teach a technique or rank, but not XP; reward XP through a quest",
                );
            }
        }
    }
}
