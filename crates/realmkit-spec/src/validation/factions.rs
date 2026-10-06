//! Factions, diplomacy and standing tracks.

use super::*;

pub(super) fn rules(out: &mut Vec<Diagnostic>, w: &WorldSpec) {
    let factions = &w.world.factions;
    ids(out, "faction", factions.iter().map(|f| f.id.as_str()));
    for faction in factions {
        named(out, &faction.id, &faction.name);
    }
    for character in &w.characters {
        if let Some(faction) = &character.faction {
            reference(
                out,
                &character.id,
                "faction",
                faction,
                w.faction(faction).is_some(),
            );
        }
    }
    if let Some(diplomacy) = &w.world.diplomacy {
        let mut seen = BTreeSet::new();
        for pair in &diplomacy.at_war {
            self::pair(out, w, &w.world.id, pair);
            if !seen.insert(faction_pair(pair)) {
                issue(
                    out,
                    &w.world.id,
                    "duplicate_pair",
                    format!("{} and {} are listed at war twice", pair[0], pair[1]),
                );
            }
        }
    }
    ids(
        out,
        "standing track",
        w.world.standing.iter().map(|t| t.id.as_str()),
    );
    for track in &w.world.standing {
        self::track(out, w, track);
    }
}

fn named(out: &mut Vec<Diagnostic>, owner: &str, name: &str) {
    if name.trim().is_empty() {
        issue(out, owner, "empty_name", "a name cannot be blank");
    }
}

fn track(out: &mut Vec<Diagnostic>, w: &WorldSpec, track: &StandingTrack) {
    let owner = &track.id;
    named(out, owner, &track.name);
    let bounded = |v: i32| (-STANDING_BOUND..=STANDING_BOUND).contains(&v);
    if track.min >= track.max || !bounded(track.min) || !bounded(track.max) {
        issue(
            out,
            owner,
            "invalid_bounds",
            format!("a track needs min below max, both within ±{STANDING_BOUND}"),
        );
    }
    let within = |v: i32| (track.min..=track.max).contains(&v);
    if !within(track.start) || !track.starts.values().all(|v| within(*v)) {
        issue(
            out,
            owner,
            "invalid_start",
            "a starting value must lie within the track's bounds",
        );
    }
    match track.scope {
        StandingScope::Global if !track.starts.is_empty() => issue(
            out,
            owner,
            "standing_scope",
            "only a faction track starts differently per faction",
        ),
        StandingScope::Faction if w.world.factions.is_empty() => issue(
            out,
            owner,
            "standing_scope",
            "a faction track needs factions",
        ),
        _ => {}
    }
    for faction in track.starts.keys() {
        reference(out, owner, "faction", faction, w.faction(faction).is_some());
    }
    let ascending = track.thresholds.windows(2).all(|t| t[0].at < t[1].at);
    if !ascending || !track.thresholds.iter().all(|t| within(t.at)) {
        issue(
            out,
            owner,
            "invalid_threshold",
            "thresholds rise strictly and lie within the track's bounds",
        );
    }
    for threshold in &track.thresholds {
        named(out, owner, &threshold.name);
    }
}

/// Two different, existing factions whose diplomacy can change.
pub(super) fn pair(out: &mut Vec<Diagnostic>, w: &WorldSpec, owner: &str, pair: &[Id; 2]) {
    for faction in pair {
        reference(out, owner, "faction", faction, w.faction(faction).is_some());
    }
    if pair[0] == pair[1] {
        issue(
            out,
            owner,
            "self_relation",
            format!("{} cannot be at war or peace with itself", pair[0]),
        );
    }
    if w.world.diplomacy.is_none() {
        issue(
            out,
            owner,
            "diplomacy_disabled",
            "this world has no diplomacy block, so war and peace never change",
        );
    }
}

/// An existing track, given a faction exactly when it is a faction track.
/// Returns the track for range checks.
pub(super) fn standing<'w>(
    out: &mut Vec<Diagnostic>,
    w: &'w WorldSpec,
    owner: &str,
    track: &str,
    faction: Option<&Id>,
) -> Option<&'w StandingTrack> {
    let found = w.standing_track(track);
    reference(out, owner, "standing track", track, found.is_some());
    if let Some(faction) = faction {
        reference(out, owner, "faction", faction, w.faction(faction).is_some());
    }
    let found = found?;
    match (found.scope, faction) {
        (StandingScope::Global, Some(_)) => issue(
            out,
            owner,
            "standing_scope",
            format!("{track} is global, so it names no faction"),
        ),
        (StandingScope::Faction, None) => issue(
            out,
            owner,
            "standing_scope",
            format!("{track} is held with each faction, so it needs one"),
        ),
        _ => {}
    }
    Some(found)
}

pub(super) fn at_least(
    out: &mut Vec<Diagnostic>,
    w: &WorldSpec,
    owner: &str,
    track: &str,
    faction: Option<&Id>,
    at_least: i32,
) {
    let Some(track) = standing(out, w, owner, track, faction) else {
        return;
    };
    if at_least <= track.min || at_least > track.max {
        issue(
            out,
            owner,
            "invalid_standing",
            format!(
                "{} runs from {} to {}, so at least {at_least} always or never holds",
                track.id, track.min, track.max
            ),
        );
    }
}

pub(super) fn change(
    out: &mut Vec<Diagnostic>,
    w: &WorldSpec,
    owner: &str,
    track: &str,
    faction: Option<&Id>,
    by: i32,
) {
    let Some(track) = standing(out, w, owner, track, faction) else {
        return;
    };
    // Bounded tracks keep the span small; anything wider only clamps.
    let span = i64::from(track.max) - i64::from(track.min);
    if by == 0 || i64::from(by).abs() > span {
        issue(
            out,
            owner,
            "invalid_change",
            format!("a change to {} is non-zero and at most {span}", track.id),
        );
    }
}
