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
        Effect::GrantCurrency { amount } | Effect::PayCurrency { amount } => {
            economy::amount(out, w, owner, *amount)
        }
        Effect::BuyWorkshop { workshop } | Effect::SellWorkshop { workshop } => {
            economy::workshop(out, w, owner, workshop)
        }
        Effect::RaiseProficiency { proficiency, ranks } => {
            economy::proficiency(out, w, owner, *proficiency, *ranks)
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

/// Every question has options with unique IDs, and an option only shapes
/// the start: no quest giver or market is at hand before the first turn.
pub(super) fn start_questions(out: &mut Vec<Diagnostic>, w: &WorldSpec) {
    let questions = &w.world.start_questions;
    ids(
        out,
        "start question",
        questions.iter().map(|q| q.id.as_str()),
    );
    for question in questions {
        if question.options.is_empty() {
            issue(
                out,
                &question.id,
                "empty_start_question",
                "a start question needs at least one option",
            );
        }
        ids(
            out,
            "start option",
            question.options.iter().map(|o| o.id.as_str()),
        );
        for effect in question.options.iter().flat_map(|o| &o.effects) {
            match effect {
                Effect::SetFlag { .. }
                | Effect::GrantItems { .. }
                | Effect::GrantCurrency { .. }
                | Effect::RaiseProficiency { .. } => self::effect(out, w, &question.id, effect),
                // Answered once, so a grant may carry XP like a quest reward.
                Effect::GrantTechnique(grant) => {
                    progression::technique_grant(out, w, &question.id, grant)
                }
                Effect::AcceptQuest { .. }
                | Effect::CompleteQuest { .. }
                | Effect::TakeItems { .. }
                | Effect::PayCurrency { .. }
                | Effect::BuyWorkshop { .. }
                | Effect::SellWorkshop { .. } => issue(
                    out,
                    &question.id,
                    "invalid_effect",
                    "a start option may set flags, grant items or currency, teach techniques and raise proficiencies",
                ),
            }
        }
    }
    // The richest answers on top of the starting purse must stay in bounds,
    // or choosing them could not start a game. Amounts already out of
    // bounds on their own are reported as such.
    if let Some(economy) = w.economy().filter(|e| e.currency.start <= CURRENCY_BOUND) {
        let granted = |o: &StartOption| {
            o.effects
                .iter()
                .filter_map(|e| match e {
                    Effect::GrantCurrency { amount } if *amount <= CURRENCY_BOUND => Some(*amount),
                    _ => None,
                })
                .fold(0, u64::saturating_add)
        };
        let most = questions
            .iter()
            .map(|q| q.options.iter().map(granted).max().unwrap_or(0))
            .fold(economy.currency.start, u64::saturating_add);
        if most > CURRENCY_BOUND {
            issue(
                out,
                &w.world.id,
                "start_overflow",
                format!("the starting currency and the most the start answers grant come to {most}, past {CURRENCY_BOUND}"),
            );
        }
    }
}
