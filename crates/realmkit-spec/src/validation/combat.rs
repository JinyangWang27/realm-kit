//! The combat block: timeline, resources, levels, groups, skills, the
//! player's attacks and the narrative, plus checks shared with profiles.

use super::*;

pub(super) fn rules(out: &mut Vec<Diagnostic>, w: &WorldSpec, combat: &Combat) {
    let owner = &w.world.id;
    basics(out, owner, combat);
    resources(out, owner, combat);
    progression::stat_points(out, w, owner, combat);
    progression::techniques(out, w, combat);
    equipment::equipment(out, w, combat);
    levels(out, owner, &combat.levels);
    groups(out, w, combat);
    skills(out, combat);
    player_skills(out, w, combat);
    player_attacks(out, w, combat);
    narrative(out, &combat.narrative);
}

/// The special channel's name, the cross share, the basic crit and the timeline.
fn basics(out: &mut Vec<Diagnostic>, owner: &str, combat: &Combat) {
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
}

/// Unused resource constants are harmless, so they only warn.
fn resources(out: &mut Vec<Diagnostic>, owner: &str, combat: &Combat) {
    let uses = |resource| {
        combat
            .skills
            .iter()
            .any(|s| s.resource == resource && s.cost > 0)
    };
    let r = combat.resources;
    let rage = r.rage_per_action > 0 || r.rage_per_max_hp > 0;
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
                out,
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
            warn(
                out,
                &skill.id,
                "unusable_skill",
                "costs rage, but this world has no rage_per_action or rage_per_max_hp",
            );
        }
    }
}

fn levels(out: &mut Vec<Diagnostic>, owner: &str, levels: &[Level]) {
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
}

fn groups(out: &mut Vec<Diagnostic>, w: &WorldSpec, combat: &Combat) {
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
}

fn skills(out: &mut Vec<Diagnostic>, combat: &Combat) {
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
}

/// The player's skills exist and unlock within the level table; technique
/// rank skills are the player's too once learned.
fn player_skills(out: &mut Vec<Diagnostic>, w: &WorldSpec, combat: &Combat) {
    ids(
        out,
        "skill reference",
        combat.player_skills.iter().map(String::as_str),
    );
    let rank_skills = combat
        .techniques
        .iter()
        .flat_map(|t| &t.ranks)
        .filter_map(|r| r.skill.as_ref());
    for id in &combat.player_skills {
        reference(out, &w.world.id, "skill", id, w.skill(id).is_some());
    }
    for id in combat.player_skills.iter().chain(rank_skills) {
        if w.skill(id).is_some_and(|s| s.level > combat.levels.len()) {
            issue(
                out,
                id,
                "invalid_level",
                "the player never reaches this skill's level",
            );
        }
    }
}

/// The player's basic attack, skills and weapons can each hit at the stats
/// they are first used with.
fn player_attacks(out: &mut Vec<Diagnostic>, w: &WorldSpec, combat: &Combat) {
    let Some(first) = combat.levels.first() else {
        return;
    };
    // Starting techniques guarantee their passives from the first moment;
    // a rank skill also has its own rank's passive in place of its
    // technique's starting one.
    let starting: Vec<(&Id, &BTreeMap<Stat, u32>)> = combat
        .player_techniques
        .iter()
        .filter_map(|g| {
            let technique = w.technique(&g.technique)?;
            let rank = technique.ranks.get(g.rank.unwrap_or(1).checked_sub(1)?)?;
            Some((&technique.id, &rank.passive))
        })
        .collect();
    let boosted = |base: Stats, own: Option<(&Id, &BTreeMap<Stat, u32>)>| {
        let mut stats = base;
        let others = starting
            .iter()
            .filter(|(id, _)| own.is_none_or(|(o, _)| o != *id));
        for (_, passive) in others.map(|(id, p)| (*id, *p)).chain(own) {
            for (stat, bonus) in passive {
                let total = stats.get_mut(*stat);
                *total = total.saturating_add(*bonus);
            }
        }
        stats
    };
    let with_rank: Vec<(&Skill, Stats)> = combat
        .player_skills
        .iter()
        .filter_map(|id| Some((w.skill(id)?, None)))
        .chain(combat.techniques.iter().flat_map(|t| {
            t.ranks
                .iter()
                .filter_map(move |r| Some((w.skill(r.skill.as_ref()?)?, Some((&t.id, &r.passive)))))
        }))
        .filter_map(|(skill, own)| {
            let base = combat.levels.get(skill.level.checked_sub(1)?)?.stats;
            Some((skill, boosted(base, own)))
        })
        .collect();
    let unlocked = with_rank.iter().map(|(skill, stats)| (*skill, stats));
    let first = boosted(first.stats, None);
    usable_skills(
        out,
        combat,
        &w.world.player,
        combat.player_basic_channel,
        unlocked,
        &first,
    );
    equipment::weapon_channels(out, w, combat, first);
}

fn narrative(out: &mut Vec<Diagnostic>, narrative: &Narrative) {
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

pub(super) fn stats(out: &mut Vec<Diagnostic>, owner: &str, stats: &Stats) {
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

pub(super) fn crit(out: &mut Vec<Diagnostic>, owner: &str, crit: Option<Crit>) {
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
pub(super) fn usable_skills<'a>(
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
