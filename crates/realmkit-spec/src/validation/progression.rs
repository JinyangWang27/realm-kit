//! Growth beyond the level table: stat points and techniques.

use super::*;

/// Points need a use, a use needs points, and even every point in one stat
/// (up to its cap) must keep that stat within the engine bound at every level.
pub(super) fn stat_points(out: &mut Vec<Diagnostic>, w: &WorldSpec, owner: &str, combat: &Combat) {
    let granted = combat.levels.iter().any(|l| l.points > 0);
    let passive = |stat| worst_bonus(w, combat, stat);
    for level in &combat.levels {
        if Stat::ALL
            .iter()
            .any(|s| u64::from(level.stats.get(*s)) + passive(*s) > u64::from(STAT_BOUND))
        {
            issue(
                out,
                owner,
                "invalid_points",
                format!("technique bonuses could raise a stat above {STAT_BOUND}"),
            );
            return;
        }
    }
    let Some(points) = &combat.stat_points else {
        if granted {
            issue(
                out,
                owner,
                "invalid_points",
                "levels grant stat points, but the combat block has no stat_points",
            );
        }
        return;
    };
    if !granted {
        warn(
            out,
            owner,
            "unused_points",
            "stat_points is configured but no level grants points",
        );
    }
    if points.values.is_empty() || points.values.values().any(|v| *v == 0) {
        issue(
            out,
            owner,
            "invalid_points",
            "stat points must add a positive amount to at least one stat",
        );
    }
    if points
        .caps
        .keys()
        .any(|stat| !points.values.contains_key(stat))
    {
        issue(
            out,
            owner,
            "invalid_points",
            "caps may only limit stats that accept points",
        );
    }
    let mut total = 0_u64;
    for level in &combat.levels {
        total += u64::from(level.points);
        for (stat, value) in &points.values {
            let taken = points
                .caps
                .get(stat)
                .map_or(total, |cap| total.min(u64::from(*cap)));
            if u64::from(level.stats.get(*stat)) + taken * u64::from(*value) + passive(*stat)
                > u64::from(STAT_BOUND)
            {
                issue(
                    out,
                    owner,
                    "invalid_points",
                    format!("stat points could raise a stat above {STAT_BOUND}; add a cap or lower the value"),
                );
                return;
            }
        }
    }
    // When every accepted stat is capped, the caps must be able to take every
    // point granted, or the rest can never be spent.
    let capacity: Option<u64> = points
        .values
        .keys()
        .map(|stat| points.caps.get(stat).map(|cap| u64::from(*cap)))
        .sum();
    if let Some(capacity) = capacity.filter(|capacity| total > *capacity) {
        warn(
            out,
            owner,
            "stranded_points",
            format!("levels grant {total} points, but the caps only take {capacity}"),
        );
    }
}

/// The most passive technique bonuses and the best gear for every slot could
/// add to `stat` at once.
pub(super) fn worst_bonus(w: &WorldSpec, combat: &Combat, stat: Stat) -> u64 {
    let gear: u64 = combat
        .slots
        .iter()
        .map(|slot| {
            w.items
                .iter()
                .filter_map(|i| i.equipment.as_ref())
                .filter(|e| e.slots.contains(slot))
                // A piece's bonus counts once, spread over its slots, so
                // the sum over slots bounds any set of worn pieces.
                .map(|e| {
                    // The best tier's bonus, since a piece may be improved,
                    // plus the best enchantment that fits it.
                    let tier = (0..=e.tiers.len())
                        .map(|t| u64::from(e.bonuses_at(t).get(&stat).copied().unwrap_or(0)))
                        .max()
                        .unwrap_or(0);
                    let enchantment = combat
                        .enchantments
                        .iter()
                        .filter(|x| x.fits(e))
                        .map(|x| u64::from(x.bonuses.get(&stat).copied().unwrap_or(0)))
                        .max()
                        .unwrap_or(0);
                    let bonus = tier + enchantment;
                    bonus.div_ceil(e.slots.len().max(1) as u64)
                })
                .max()
                .unwrap_or(0)
        })
        .sum();
    gear + combat
        .techniques
        .iter()
        .map(|t| {
            t.ranks
                .iter()
                .map(|r| u64::from(r.passive.get(&stat).copied().unwrap_or(0)))
                .max()
                .unwrap_or(0)
        })
        .sum::<u64>()
}

/// A grant names a known technique and, if it sets one, an existing rank.
pub(super) fn technique_grant(
    out: &mut Vec<Diagnostic>,
    w: &WorldSpec,
    owner: &str,
    grant: &TechniqueGrant,
) {
    let technique = w.technique(&grant.technique);
    reference(
        out,
        owner,
        "technique",
        &grant.technique,
        technique.is_some(),
    );
    if let (Some(technique), Some(rank)) = (technique, grant.rank) {
        if rank == 0 || rank > technique.ranks.len() {
            issue(
                out,
                owner,
                "invalid_rank",
                format!("{} has no rank {rank}", technique.id),
            );
        }
    }
}

/// Named ranks with rising thresholds, existing skills and declared gates.
pub(super) fn techniques(out: &mut Vec<Diagnostic>, w: &WorldSpec, combat: &Combat) {
    ids(
        out,
        "technique",
        combat.techniques.iter().map(|t| t.id.as_str()),
    );
    skill_owners(out, combat);
    for technique in &combat.techniques {
        technique_ranks(out, w, combat, technique);
    }
    for grant in &combat.player_techniques {
        technique_grant(out, w, &w.world.player, grant);
    }
    if let Some(core) = &combat.core_art {
        core_art(out, w, combat, core);
    }
}

/// A skill belongs to one technique, so its use trains exactly that one.
fn skill_owners(out: &mut Vec<Diagnostic>, combat: &Combat) {
    let mut owners: BTreeMap<&Id, &Id> = BTreeMap::new();
    for technique in &combat.techniques {
        for skill in technique.ranks.iter().filter_map(|r| r.skill.as_ref()) {
            if owners
                .insert(skill, &technique.id)
                .is_some_and(|other| other != &technique.id)
            {
                issue(
                    out,
                    &technique.id,
                    "invalid_skill",
                    format!("{skill} already belongs to another technique"),
                );
            }
        }
    }
}

fn technique_ranks(
    out: &mut Vec<Diagnostic>,
    w: &WorldSpec,
    combat: &Combat,
    technique: &Technique,
) {
    let id = &technique.id;
    if technique.name.trim().is_empty()
        || technique.ranks.is_empty()
        || technique.ranks.iter().any(|r| r.name.trim().is_empty())
    {
        issue(
            out,
            id,
            "empty_name",
            "a technique and each of its ranks need a name",
        );
    }
    let thresholds: Vec<u64> = technique.ranks.iter().map(|r| r.xp).collect();
    if thresholds.first().is_some_and(|xp| *xp != 0) || thresholds.windows(2).any(|t| t[0] >= t[1])
    {
        issue(
            out,
            id,
            "invalid_levels",
            "rank XP must start at 0 and strictly increase",
        );
    }
    if technique.xp_share_percent > 100 {
        issue(
            out,
            id,
            "invalid_share",
            "an XP share is a percentage from 0 to 100",
        );
    }
    for rank in &technique.ranks {
        if let Some(skill) = &rank.skill {
            reference(out, id, "skill", skill, w.skill(skill).is_some());
            if combat.player_skills.contains(skill) {
                issue(
                    out,
                    id,
                    "invalid_skill",
                    format!("{skill} is a technique's rank and cannot also be a player skill"),
                );
            }
        }
        condition(out, w, id, rank.requires.as_ref());
    }
}

/// The realm's art exists and something teaches it.
fn core_art(out: &mut Vec<Diagnostic>, w: &WorldSpec, combat: &Combat, core: &Id) {
    reference(
        out,
        &w.world.id,
        "core art",
        core,
        w.technique(core).is_some(),
    );
    let teachable = combat
        .player_techniques
        .iter()
        .any(|g| &g.technique == core)
        || w.quests
            .iter()
            .flat_map(|q| &q.reward_techniques)
            .any(|g| &g.technique == core)
        || w.effects()
            .any(|e| matches!(e, Effect::GrantTechnique(g) if &g.technique == core));
    if !teachable {
        issue(
            out,
            core,
            "unreachable_core_art",
            "no starting technique, dialogue or quest teaches the core art",
        );
    }
}
