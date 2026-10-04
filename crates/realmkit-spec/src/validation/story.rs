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
        condition(out, w, &quest.id, quest.requires.as_ref());
        // The main story may branch on side stories, never wait on one.
        if quest.main {
            for side in prerequisites(w, quest).filter(|p| !p.main) {
                issue(
                    out,
                    &quest.id,
                    "main_requires_side",
                    format!(
                        "main quest {} cannot require side quest {}",
                        quest.id, side.id
                    ),
                );
            }
        }
    }
    quest_cycles(out, w);
}

/// Quests `quest` cannot be taken up without having taken up first: those
/// its condition requires past `available` on every branch.
fn prerequisites<'w>(w: &'w WorldSpec, quest: &'w Quest) -> impl Iterator<Item = &'w Quest> {
    w.quests.iter().filter(|other| {
        quest.requires.as_ref().is_some_and(|c| {
            c.requires(&|leaf| {
                matches!(leaf, Condition::Quest { quest, status }
                    if *quest == other.id && *status != QuestStatus::Available)
            })
        })
    })
}

/// A quest that waits, through its prerequisites, on itself can never be
/// taken up.
fn quest_cycles(out: &mut Vec<Diagnostic>, w: &WorldSpec) {
    for quest in &w.quests {
        let mut seen = BTreeSet::new();
        let mut next: Vec<&Quest> = prerequisites(w, quest).collect();
        while let Some(prior) = next.pop() {
            if prior.id == quest.id {
                issue(
                    out,
                    &quest.id,
                    "quest_cycle",
                    format!("{} waits on itself through its prerequisites", quest.id),
                );
                break;
            }
            if seen.insert(&prior.id) {
                next.extend(prerequisites(w, prior));
            }
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
        Effect::EnterPhase { phase } => {
            reference(out, owner, "phase", phase, w.phase_index(phase).is_some())
        }
        Effect::DiscoverEvidence { evidence } => reference(
            out,
            owner,
            "evidence",
            evidence,
            w.evidence(evidence).is_some(),
        ),
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
                | Effect::SellWorkshop { .. }
                | Effect::DiscoverEvidence { .. }
                | Effect::EnterPhase { .. } => issue(
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

/// Evidence definitions have unique IDs, and a linked item exists.
pub(super) fn evidence(out: &mut Vec<Diagnostic>, w: &WorldSpec) {
    ids(
        out,
        "evidence",
        w.world.evidence.iter().map(|e| e.id.as_str()),
    );
    for evidence in &w.world.evidence {
        if let Some(item) = &evidence.item {
            reference(out, &evidence.id, "item", item, w.item(item).is_some());
        }
    }
}

/// A condition on evidence names defined evidence that some authored effect
/// can discover; otherwise it could never hold.
pub(super) fn evidence_known(out: &mut Vec<Diagnostic>, w: &WorldSpec, owner: &str, id: &str) {
    reference(out, owner, "evidence", id, w.evidence(id).is_some());
    let discoverable = establishes(
        w,
        |e| matches!(e, Effect::DiscoverEvidence { evidence } if evidence == id),
        |leaf| matches!(leaf, Condition::Evidence { evidence } if evidence == id),
    );
    if w.evidence(id).is_some() && !discoverable {
        issue(
            out,
            owner,
            "undiscoverable_evidence",
            format!("no effect discovers {id}, so this condition can never hold"),
        );
    }
}

/// Whether some dialogue choice has an effect that `matches` without needing,
/// on every branch of its condition, what that effect would establish. Only
/// dialogue discovers evidence or enters phases.
fn establishes(
    w: &WorldSpec,
    matches: impl Fn(&Effect) -> bool,
    needs: impl Fn(&Condition) -> bool,
) -> bool {
    w.dialogues
        .iter()
        .flat_map(|d| &d.nodes)
        .flat_map(|n| &n.choices)
        .filter(|c| c.effects.iter().any(&matches))
        .any(|c| !c.requires.as_ref().is_some_and(|r| r.requires(&needs)))
}

/// Phases have unique IDs, and every phase after the first is entered by
/// some effect; otherwise the story could never reach it.
pub(super) fn phases(out: &mut Vec<Diagnostic>, w: &WorldSpec) {
    ids(out, "phase", w.world.phases.iter().map(|p| p.id.as_str()));
    for (index, phase) in w.world.phases.iter().enumerate().skip(1) {
        // A choice that needs this phase or a later one cannot be what enters it.
        let entered = establishes(
            w,
            |e| matches!(e, Effect::EnterPhase { phase: p } if *p == phase.id),
            |leaf| {
                matches!(leaf, Condition::Phase { phase: p }
                    if w.phase_index(p).is_some_and(|i| i >= index))
            },
        );
        if !entered {
            issue(
                out,
                &phase.id,
                "unreachable_phase",
                format!("no effect enters phase {}", phase.id),
            );
        }
    }
}

/// Outcomes have unique IDs and valid conditions, and none can hold at the
/// start: each requires, on every branch, something no start provides. That
/// is a quest taken up, evidence, a workshop, a phase after the first, or a
/// flag no start answer sets.
pub(super) fn outcomes(out: &mut Vec<Diagnostic>, w: &WorldSpec) {
    ids(
        out,
        "outcome",
        w.world.outcomes.iter().map(|o| o.id.as_str()),
    );
    let answered: BTreeSet<&Id> = w
        .start_options()
        .flat_map(|o| &o.effects)
        .filter_map(|e| match e {
            Effect::SetFlag { flag } => Some(flag),
            _ => None,
        })
        .collect();
    let never_at_start = |leaf: &Condition| match leaf {
        Condition::Quest { status, .. } => *status != QuestStatus::Available,
        Condition::Evidence { .. } | Condition::Workshop { .. } => true,
        Condition::Phase { phase } => w.phase_index(phase).is_some_and(|i| i > 0),
        Condition::Flag { flag } => !answered.contains(flag),
        _ => false,
    };
    for outcome in &w.world.outcomes {
        condition(out, w, &outcome.id, Some(&outcome.when));
        if !outcome.when.requires(&never_at_start) {
            issue(
                out,
                &outcome.id,
                "outcome_at_start",
                "an outcome must require something no start provides: a quest taken up, evidence, a workshop, a later phase or a flag no start answer sets",
            );
        }
    }
}
