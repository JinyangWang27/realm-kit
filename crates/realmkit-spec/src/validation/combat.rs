//! The combat block: timeline, resources, levels, groups, skills, the
//! player's attacks and the narrative, plus checks shared with profiles.

use super::*;

pub(super) fn rules(out: &mut Vec<Diagnostic>, w: &WorldSpec, combat: &Combat) {
    let owner = &w.world.id;
    basics(out, owner, combat);
    resources(out, w, owner, combat);
    progression::stat_points(out, w, owner, combat);
    progression::techniques(out, w, combat);
    equipment::equipment(out, w, combat);
    crafting::crafting(out, w, combat);
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
    realtime(out, owner, combat);
    let Some(timeline) = combat.timeline else {
        return;
    };
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

/// Real-time timing within its bounds, so the duel clock's arithmetic stays
/// small, and a stagger wherever something can interrupt.
fn realtime(out: &mut Vec<Diagnostic>, owner: &str, combat: &Combat) {
    let Some(r) = combat.realtime else {
        return;
    };
    let phase = |steps: u32| (1..=PHASE_BOUND).contains(&steps);
    let timed = |d: Option<TimedDefense>| d.is_none_or(|d| phase(d.window) && phase(d.recovery));
    if !(timed(r.parry) && timed(r.dodge)) || r.stagger > PHASE_BOUND || r.guard_poise > POISE_BOUND
    {
        issue(
            out,
            owner,
            "invalid_timed_defense",
            format!("a parry's or dodge's window and recovery are 1 to {PHASE_BOUND} steps, a stagger 0 to {PHASE_BOUND}, a guard's poise 0 to {POISE_BOUND}"),
        );
    }
    let interrupts = r.parry.is_some() || combat.skills.iter().any(|s| s.impact > 0);
    if interrupts && r.stagger == 0 {
        issue(
            out,
            owner,
            "missing_stagger",
            "a parry or a skill with impact staggers, so give the stagger 1 or more steps",
        );
    }
    if !(1..=STEP_RATE_BOUND).contains(&r.steps_per_second)
        || !(phase(r.windup) && phase(r.active) && phase(r.recovery) && phase(r.retreat))
        || r.guard_raise > PHASE_BOUND
        || r.guard_percent > 100
        || r.mp_regen_percent > 100
    {
        issue(
            out,
            owner,
            "invalid_realtime",
            format!("steps per second are 1 to {STEP_RATE_BOUND}; wind-up, active, recovery and retreat 1 to {PHASE_BOUND} steps, a guard's raise 0 to {PHASE_BOUND}; guard and regeneration percentages 0 to 100"),
        );
    }
}

/// Resource rules within their bounds. Unused resource constants are
/// harmless, so they only warn.
fn resources(out: &mut Vec<Diagnostic>, w: &WorldSpec, owner: &str, combat: &Combat) {
    let uses = |resource| combat.skills.iter().any(|s| s.cost_in(resource) > 0);
    let r = combat.resources;
    let rage = r.gains_rage();
    let periodic = |every: u64| (1..=DURATION_BOUND).contains(&every);
    if r.rage_cap.is_some_and(|cap| cap == 0 || cap > STAT_BOUND)
        || r.mp_recovery
            .is_some_and(|m| !(1..=100).contains(&m.percent) || !periodic(m.every))
        || r.rage_decay
            .is_some_and(|d| d.points == 0 || d.points > STAT_BOUND || !periodic(d.every))
    {
        issue(
            out,
            owner,
            "invalid_resources",
            format!("a rage cap is from 1 to {STAT_BOUND}; recovery is 1 to 100 percent and decay 1 to {STAT_BOUND} points, every 1 to {DURATION_BOUND} minutes"),
        );
    }
    for (set, used, what) in [
        (
            r.mp_regen_percent > 0
                || r.mp_recovery.is_some()
                || combat.realtime.is_some_and(|t| t.mp_regen_percent > 0),
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
    let clock = w.world.time.is_some();
    for (set, what) in [
        (
            r.mp_recovery.is_some() && !clock,
            "MP recovery needs world time, which this world does not keep",
        ),
        (
            r.rage_decay.is_some() && !clock,
            "rage decay needs world time, which this world does not keep",
        ),
        (
            r.rage_decay.is_some() && !r.rage_carries,
            "rage clears after every fight, so it has nothing to decay",
        ),
    ] {
        if set {
            warn(out, owner, "unused_resource", what);
        }
    }
    for skill in combat
        .skills
        .iter()
        .filter(|s| s.cost_in(Resource::Rage) > 0)
    {
        // Rage starts at 0, so without a source a rage cost is never met.
        if !rage {
            warn(
                out,
                &skill.id,
                "unusable_skill",
                "costs rage, but this world has no rage_per_action or rage_per_max_hp",
            );
        }
        if r.rage_cap
            .is_some_and(|cap| skill.cost_in(Resource::Rage) > cap)
        {
            issue(
                out,
                &skill.id,
                "unaffordable_skill",
                "costs more rage than the rage cap allows",
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
        if skill.costs.is_some() && skill.cost > 0 {
            issue(
                out,
                &skill.id,
                "conflicting_costs",
                "give costs in either cost and resource or costs, not both",
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
        if skill.impact > POISE_BOUND || skill.poise > POISE_BOUND {
            issue(
                out,
                &skill.id,
                "invalid_impact",
                format!("impact and poise are 0 to {POISE_BOUND}"),
            );
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
        combat.player_resources,
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
    uses: Option<ResourceUse>,
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
        // A resource the character does not use pays for nothing.
        for (resource, used) in uses.map_or(vec![], |u| {
            vec![(Resource::Mp, u.mp), (Resource::Rage, u.rage)]
        }) {
            if !used && skill.cost_in(resource) > 0 {
                issue(
                    out,
                    owner,
                    "inactive_resource",
                    format!(
                        "{} costs {}, which the character does not use",
                        skill.id,
                        match resource {
                            Resource::Mp => "MP",
                            Resource::Rage => "rage",
                        }
                    ),
                );
            }
        }
        // Rage accumulates during a fight, so only MP has a ceiling to check.
        if skill.cost_in(Resource::Mp) > stats.mp {
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
