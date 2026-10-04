//! World time, roads, scheduled events and characters who move.

use super::*;

/// Content that passes time needs the world's `time` block.
pub(super) fn needed(out: &mut Vec<Diagnostic>, w: &WorldSpec, owner: &str) {
    if w.world.time.is_none() {
        issue(
            out,
            owner,
            "time_disabled",
            "this world has no time block, so nothing can take time",
        );
    }
}

pub(super) fn rules(out: &mut Vec<Diagnostic>, w: &WorldSpec) {
    if let Some(time) = &w.world.time {
        clock(out, w, time);
    }
    roads(out, w);
    ids(out, "event", w.world.events.iter().map(|e| e.id.as_str()));
    for event in &w.world.events {
        needed(out, w, &event.id);
        schedule(out, w, &event.id, &event.schedule);
        condition(out, w, &event.id, event.requires.as_ref());
        for effect in &event.effects {
            event_effect(out, w, &event.id, effect, event.schedule.every.is_some());
        }
    }
    for character in &w.characters {
        if let Some(moves) = &character.moves {
            mover(out, w, character, moves);
        }
    }
}

fn clock(out: &mut Vec<Diagnostic>, w: &WorldSpec, time: &WorldTime) {
    let owner = &w.world.id;
    if time.start > WORLD_TIME_BOUND {
        issue(
            out,
            owner,
            "invalid_time",
            format!("world time starts at most at minute {WORLD_TIME_BOUND}"),
        );
    }
    template(out, owner, &time.clock.0, &["day", "hour", "minute"]);
    for (minutes, what) in [(time.wait, "a wait"), (time.rest, "a rest")] {
        if minutes.is_some_and(|m| m == 0 || m > DURATION_BOUND) {
            issue(
                out,
                owner,
                "invalid_duration",
                format!("{what} lasts 1 to {DURATION_BOUND} minutes"),
            );
        }
    }
    if time.rest.is_some() && w.combat().is_none() {
        issue(
            out,
            owner,
            "combat_disabled",
            "this world has no combat block, so there is nothing to rest from",
        );
    }
}

/// Roads join two different locations, at most one road per pair.
fn roads(out: &mut Vec<Diagnostic>, w: &WorldSpec) {
    ids(out, "road", w.world.roads.iter().map(|r| r.id.as_str()));
    let mut pairs = BTreeSet::new();
    for road in &w.world.roads {
        for end in &road.between {
            reference(out, &road.id, "road end", end, w.location(end).is_some());
        }
        let [a, b] = &road.between;
        if a == b {
            issue(
                out,
                &road.id,
                "invalid_road",
                "a road joins two different locations",
            );
        }
        if !pairs.insert((a.min(b), a.max(b))) {
            issue(
                out,
                &road.id,
                "duplicate_road",
                format!("another road already joins {a} and {b}"),
            );
        }
        if road.minutes > DURATION_BOUND {
            issue(
                out,
                &road.id,
                "invalid_duration",
                format!("a road takes at most {DURATION_BOUND} minutes"),
            );
        }
        if road.minutes > 0 {
            needed(out, w, &road.id);
        }
        condition(out, w, &road.id, road.requires.as_ref());
        if road.requires.is_some() != road.blocked_text.is_some() {
            issue(
                out,
                &road.id,
                "invalid_road",
                "a road has `blocked_text` exactly when it has `requires`",
            );
        }
    }
}

/// Every occurrence is after the start of play, so nothing is due at once
/// and nothing can schedule itself at its own minute.
pub(super) fn schedule(out: &mut Vec<Diagnostic>, w: &WorldSpec, owner: &str, schedule: &Schedule) {
    let start = w.world.time.as_ref().map_or(0, |t| t.start);
    if schedule.at <= start || schedule.at > WORLD_TIME_BOUND {
        issue(
            out,
            owner,
            "invalid_schedule",
            format!("a schedule first falls after the start, minute {start}, and by minute {WORLD_TIME_BOUND}"),
        );
    }
    if schedule
        .every
        .is_some_and(|e| e == 0 || e > WORLD_TIME_BOUND)
    {
        issue(
            out,
            owner,
            "invalid_schedule",
            "a recurring schedule's period is at least one minute",
        );
    }
}

/// Events change the world, not the player's conversations: they cannot
/// accept or complete quests at a giver, or take what the player carries.
fn event_effect(
    out: &mut Vec<Diagnostic>,
    w: &WorldSpec,
    owner: &str,
    effect: &Effect,
    recurs: bool,
) {
    match effect {
        Effect::SetFlag { .. } | Effect::GrantItems { .. } | Effect::GrantCurrency { .. } => {
            story::effect(out, w, owner, effect)
        }
        // A one-shot event happens once, so it may grant XP like a quest.
        Effect::GrantTechnique(grant) => {
            progression::technique_grant(out, w, owner, grant);
            if grant.xp > 0 && recurs {
                issue(
                    out,
                    owner,
                    "repeatable_reward",
                    "a recurring event may teach a technique or rank, but not XP",
                );
            }
        }
        Effect::AcceptQuest { .. }
        | Effect::CompleteQuest { .. }
        | Effect::TakeItems { .. }
        | Effect::PayCurrency { .. }
        | Effect::BuyWorkshop { .. }
        | Effect::SellWorkshop { .. }
        | Effect::RaiseProficiency { .. }
        | Effect::DiscoverEvidence { .. } => issue(
            out,
            owner,
            "invalid_effect",
            "an event may set flags, grant items or currency and teach techniques",
        ),
    }
}

/// A mover is placed once, at one of the places it moves among, and is no
/// fighter: a fight's opponents come from where they are placed.
fn mover(out: &mut Vec<Diagnostic>, w: &WorldSpec, character: &Character, moves: &Moves) {
    let owner = &character.id;
    needed(out, w, owner);
    schedule(out, w, owner, &moves.schedule);
    ids(
        out,
        "mover location",
        moves.among.iter().map(String::as_str),
    );
    for id in &moves.among {
        reference(out, owner, "location", id, w.location(id).is_some());
    }
    let placed: Vec<_> = w
        .locations
        .iter()
        .filter(|l| l.characters.contains(owner))
        .collect();
    let valid = moves.among.len() >= 2
        && character.combat.is_none()
        && *owner != w.world.player
        && placed.len() == 1
        && moves.among.contains(&placed[0].id);
    if !valid {
        issue(
            out,
            owner,
            "invalid_mover",
            "a mover moves among two or more locations, starts placed at exactly one of them, and has no combat profile",
        );
    }
}
