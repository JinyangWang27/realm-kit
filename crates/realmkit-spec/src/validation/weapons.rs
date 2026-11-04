//! Weapon families, handling disciplines, and what names them.

use super::*;

pub(super) fn weapons(out: &mut Vec<Diagnostic>, w: &WorldSpec, combat: &Combat) {
    let rules = combat.weapon_proficiency.as_ref();
    if let Some(rules) = rules {
        table(out, &w.world.id, rules);
    }
    for item in &w.items {
        let Some((gear, profile)) = item
            .equipment
            .as_ref()
            .and_then(|g| Some((g, g.weapon.as_ref()?)))
        else {
            continue;
        };
        if gear.ranged.is_some() {
            issue(
                out,
                &item.id,
                "invalid_weapon_profile",
                "a drawn weapon cannot have a weapon profile yet",
            );
        }
        let known = rules.is_some_and(|r| r.families.iter().any(|f| f.id == profile.family));
        reference(out, &item.id, "weapon family", &profile.family, known);
        for discipline in profile.handling.keys() {
            let known = rules.is_some_and(|r| r.disciplines.iter().any(|d| d.id == *discipline));
            reference(out, &item.id, "weapon discipline", discipline, known);
        }
        let total: u64 = profile.handling.values().map(|v| u64::from(*v)).sum();
        if total != 100 || profile.handling.values().any(|v| *v == 0) {
            issue(
                out,
                &item.id,
                "invalid_handling",
                "a weapon's handling weights are positive and total 100",
            );
        }
    }
    for character in &w.characters {
        let challenge = character.combat.as_ref().and_then(|c| c.weapon_challenge);
        if challenge.is_some_and(|c| rules.is_none() || c > PROFICIENCY_RANK_BOUND) {
            issue(
                out,
                &character.id,
                "invalid_weapon_challenge",
                format!("a weapon challenge needs weapon proficiency in the world and is at most {PROFICIENCY_RANK_BOUND}"),
            );
        }
    }
    for skill in &combat.skills {
        let Some(mastery) = &skill.mastery else {
            continue;
        };
        for family in &mastery.families {
            let known = rules.is_some_and(|r| r.families.iter().any(|f| f.id == *family));
            reference(out, &skill.id, "weapon family", family, known);
        }
        // A family no weapon of which allows the skill would lock it for good.
        let wieldable = mastery.families.is_empty()
            || w.items
                .iter()
                .filter_map(|i| i.equipment.as_ref())
                .any(|g| {
                    g.weapon
                        .as_ref()
                        .is_some_and(|p| mastery.families.contains(&p.family))
                        && skill.requires.iter().all(|c| g.capabilities.contains(c))
                });
        if rules.is_none()
            || skill.requires.is_empty()
            || mastery.direct > PROFICIENCY_RANK_BOUND
            || mastery.effective > PROFICIENCY_RANK_BOUND
            || !wieldable
        {
            issue(
                out,
                &skill.id,
                "invalid_skill_mastery",
                format!("a skill's mastery needs weapon proficiency in the world, a required capability, ranks up to {PROFICIENCY_RANK_BOUND}, and a weapon of its families that allows it"),
            );
        }
    }
}

fn table(out: &mut Vec<Diagnostic>, owner: &str, rules: &WeaponProficiency) {
    ids(
        out,
        "weapon discipline",
        rules.disciplines.iter().map(|d| d.id.as_str()),
    );
    ids(
        out,
        "weapon family",
        rules.families.iter().map(|f| f.id.as_str()),
    );
    for family in &rules.families {
        for (target, percent) in &family.affinities {
            let known = rules.families.iter().any(|f| f.id == *target);
            reference(out, &family.id, "weapon family", target, known);
            if *percent > 100 || *percent == 0 || target == &family.id {
                issue(
                    out,
                    &family.id,
                    "invalid_affinity",
                    "an affinity is 1 to 100 percent, toward another family",
                );
            }
        }
    }
    let ordered = rules
        .ranks
        .windows(2)
        .all(|r| r[0].rank < r[1].rank && r[0].xp < r[1].xp);
    let bounded = rules.ranks.iter().all(|r| {
        (1..=PROFICIENCY_RANK_BOUND).contains(&r.rank) && (1..=RANK_XP_BOUND).contains(&r.xp)
    });
    if rules.ranks.is_empty() || !ordered || !bounded {
        issue(
            out,
            owner,
            "invalid_proficiency_ranks",
            format!("proficiency ranks rise strictly, ranks 1 to {PROFICIENCY_RANK_BOUND} at 1 to {RANK_XP_BOUND} XP"),
        );
    }
    if rules.family_percent > 100
        || rules.windup_percent > ATTACK_CUT_BOUND
        || rules.recovery_percent > ATTACK_CUT_BOUND
    {
        issue(
            out,
            owner,
            "invalid_proficiency_timing",
            format!("family_percent is at most 100; wind-up and recovery cuts at most {ATTACK_CUT_BOUND}"),
        );
    }
    let xp = rules.xp;
    if [xp.hit, xp.defense, xp.target_budget]
        .iter()
        .any(|v| *v > WEAPON_XP_BOUND)
        || xp.target_budget == 0
        || xp.hit == 0
        || xp.challenge_per_level > PROFICIENCY_RANK_BOUND
        || !(1..=PROFICIENCY_RANK_BOUND).contains(&xp.trivial_margin)
    {
        issue(
            out,
            owner,
            "invalid_proficiency_xp",
            format!("a hit's XP and a foe's budget are 1 to {WEAPON_XP_BOUND}, defense XP at most that; the challenge per level is at most {PROFICIENCY_RANK_BOUND} and the trivial margin 1 to {PROFICIENCY_RANK_BOUND}"),
        );
    }
}
