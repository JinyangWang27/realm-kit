//! Troop lines, recruiting, upkeep, armies and the mass-battle rules.

use super::*;

/// Every check on troops, battles, recruiting places and armies.
pub(super) fn rules(out: &mut Vec<Diagnostic>, w: &WorldSpec) {
    let owner = w.world.id.as_str();
    if let Some(troops) = w.troops() {
        if w.combat().is_none() {
            issue(
                out,
                owner,
                "combat_disabled",
                "troops fight, so they need the world's combat block",
            );
        }
        lines(out, w, troops);
    }
    if let Some(battle) = w.battle() {
        match w.troops() {
            None => issue(
                out,
                owner,
                "troops_disabled",
                "battles are fought by troops, so they need the world's troops block",
            ),
            Some(troops) => rules_of_battle(out, troops, battle),
        }
    }
    for location in &w.locations {
        if let Some(recruits) = &location.recruits {
            recruiting(out, w, &location.id, recruits);
        }
    }
    for character in &w.characters {
        if let Some(army) = &character.army {
            army_of(out, w, character, army);
        }
    }
}

fn percent(out: &mut Vec<Diagnostic>, owner: &str, value: u32, most: u32, what: &str) {
    if value > most {
        issue(
            out,
            owner,
            "invalid_percent",
            format!("{what} is at most {most} percent"),
        );
    }
}

fn lines(out: &mut Vec<Diagnostic>, w: &WorldSpec, troops: &Troops) {
    let owner = w.world.id.as_str();
    ids(
        out,
        "troop class",
        troops.classes.iter().map(|c| c.id.as_str()),
    );
    for class in &troops.classes {
        if class.name.trim().is_empty() || (class.ranged && class.mounted) {
            issue(
                out,
                &class.id,
                "invalid_class",
                "a troop class has a name and is ranged, mounted or neither",
            );
        }
    }
    ids(
        out,
        "troop line",
        troops.lines.iter().map(|l| l.id.as_str()),
    );
    for line in &troops.lines {
        reference(
            out,
            &line.id,
            "troop class",
            &line.class,
            troops.class(&line.class).is_some(),
        );
        let named_first = line.levels.first().is_some_and(|l| l.name.is_some());
        let xp_ok = line
            .levels
            .iter()
            .enumerate()
            .all(|(i, l)| (i == 0) == (l.xp == 0));
        let names_ok = line
            .levels
            .iter()
            .all(|l| l.name.as_ref().is_none_or(|n| !n.trim().is_empty()));
        if !named_first || !xp_ok || !names_ok {
            issue(
                out,
                &line.id,
                "invalid_line",
                "a line's first level is named and alone needs no XP; every later level needs some",
            );
        }
        let share = w.combat().map_or(0, |c| c.cross_share);
        for level in &line.levels {
            combat::stats(out, &line.id, &level.stats);
            if level.stats.combined(line.channel, share, false) == 0 {
                issue(
                    out,
                    &line.id,
                    "no_attack",
                    "every level of a line has some attack in the line's channel",
                );
            }
            if level.wage.is_some_and(|wage| wage > CURRENCY_BOUND) {
                issue(
                    out,
                    &line.id,
                    "invalid_amount",
                    format!("a wage is at most {CURRENCY_BOUND}"),
                );
            }
        }
        if line
            .levels
            .iter()
            .any(|l| l.wage.is_some_and(|wage| wage > 0))
        {
            economy::needed(out, w, &line.id);
        }
        let mut seen = BTreeSet::new();
        for upgrade in &line.upgrades {
            reference(
                out,
                &line.id,
                "troop line",
                &upgrade.to,
                troops.line(&upgrade.to).is_some(),
            );
            if !seen.insert(&upgrade.to) || upgrade.to == line.id {
                issue(
                    out,
                    &line.id,
                    "invalid_line",
                    format!(
                        "{} is an upgrade once, and not into its own line",
                        upgrade.to
                    ),
                );
            }
            if upgrade.cost > 0 {
                economy::amount(out, w, &line.id, upgrade.cost);
            }
        }
    }
    if troops.limit == 0 || troops.limit > ROSTER_BOUND {
        issue(
            out,
            owner,
            "invalid_limit",
            format!("the roster limit is 1 to {ROSTER_BOUND}"),
        );
    }
    percent(out, owner, troops.wounded_percent, 100, "the wounded share");
    if troops.payroll == Payroll::Monthly {
        time::calendar_needed(out, w, owner);
        if troops.upkeep.is_none() {
            issue(
                out,
                owner,
                "invalid_payroll",
                "monthly wages take their desertion share from `upkeep`, so they need one",
            );
        }
    }
    if let Some(upkeep) = &troops.upkeep {
        time::needed(out, w, owner);
        time::schedule(out, w, owner, &upkeep.schedule);
        percent(out, owner, upkeep.recover_percent, 100, "recovery");
        percent(out, owner, upkeep.desert_percent, 100, "desertion");
    } else if troops.payroll == Payroll::Upkeep
        && troops
            .lines
            .iter()
            .any(|l| l.levels.iter().any(|v| v.wage.is_some_and(|wage| wage > 0)))
    {
        warn(
            out,
            owner,
            "unused_wages",
            "wages fall due only on an upkeep schedule, and there is none",
        );
    }
}

/// A line and level that exist.
fn level_of(out: &mut Vec<Diagnostic>, w: &WorldSpec, owner: &str, line: &str, level: usize) {
    let Some(troops) = w.troops() else {
        return;
    };
    match troops.line(line) {
        None => reference(out, owner, "troop line", line, false),
        Some(found) if level == 0 || level > found.levels.len() => issue(
            out,
            owner,
            "invalid_level",
            format!("{line} has no level {level}"),
        ),
        Some(_) => {}
    }
}

fn rules_of_battle(out: &mut Vec<Diagnostic>, troops: &Troops, battle: &Battle) {
    let owner = "battle";
    if battle.frontage == 0
        || battle.frontage > ROSTER_BOUND
        || battle.rounds == 0
        || battle.rounds > 1_000
    {
        issue(
            out,
            owner,
            "invalid_battle",
            format!("frontage is 1 to {ROSTER_BOUND} and rounds 1 to 1,000"),
        );
    }
    let [lo, hi] = battle.roll;
    if lo == 0 || lo > hi || hi > 1_000 {
        issue(
            out,
            owner,
            "invalid_battle",
            "a roll is between 1 and 1,000 percent, low first",
        );
    }
    for (attacker, targets) in &battle.matchups {
        reference(
            out,
            owner,
            "troop class",
            attacker,
            troops.class(attacker).is_some(),
        );
        for (target, value) in targets {
            reference(
                out,
                owner,
                "troop class",
                target,
                troops.class(target).is_some(),
            );
            percent(out, owner, *value, 1_000, "a matchup");
        }
    }
    if let Some(class) = &battle.pursuit_class {
        reference(
            out,
            owner,
            "troop class",
            class,
            troops.class(class).is_some(),
        );
    }
    percent(out, owner, battle.hold_percent, 1_000, "holding");
    percent(out, owner, battle.flank_percent, 1_000, "flanking");
    percent(
        out,
        owner,
        battle.player_xp_percent,
        100,
        "the player's XP share",
    );
    if let Some(morale) = battle.morale {
        percent(out, owner, morale.floor, 100, "the morale floor");
        percent(out, owner, morale.rout, 99, "the rout threshold");
        percent(out, owner, morale.factor, 1_000, "the morale factor");
    }
}

fn recruiting(out: &mut Vec<Diagnostic>, w: &WorldSpec, owner: &str, recruits: &Recruits) {
    if w.troops().is_none() {
        issue(
            out,
            owner,
            "troops_disabled",
            "recruits are troops, so they need the world's troops block",
        );
    }
    let mut seen = BTreeSet::new();
    for offer in &recruits.troops {
        level_of(out, w, owner, &offer.line, 1);
        if !seen.insert(&offer.line) {
            issue(
                out,
                owner,
                "duplicate_id",
                format!("{} is offered here once", offer.line),
            );
        }
        if offer.price > 0 {
            economy::amount(out, w, owner, offer.price);
        }
        if offer.size == 0 || offer.size > ROSTER_BOUND {
            issue(
                out,
                owner,
                "invalid_limit",
                format!("a recruiting pool holds 1 to {ROSTER_BOUND}"),
            );
        }
    }
    if let Some(refill) = &recruits.refill {
        time::needed(out, w, owner);
        time::schedule(out, w, owner, &refill.schedule);
        if refill.amount == 0 || refill.amount > ROSTER_BOUND {
            issue(
                out,
                owner,
                "invalid_limit",
                format!("a refill adds 1 to {ROSTER_BOUND}"),
            );
        }
    }
}

fn army_of(out: &mut Vec<Diagnostic>, w: &WorldSpec, character: &Character, army: &Army) {
    let owner = character.id.as_str();
    if w.battle().is_none() {
        issue(
            out,
            owner,
            "battle_disabled",
            "an army fights mass battles, so it needs the world's battle block",
        );
    }
    if character.combat.is_some() {
        issue(
            out,
            owner,
            "invalid_army",
            "a character leads an army or fights alone, not both",
        );
    }
    if army.troops.is_empty() {
        issue(out, owner, "invalid_army", "an army has troops");
    }
    for stack in &army.troops {
        level_of(out, w, owner, &stack.line, stack.level);
        if stack.count == 0 || stack.count > ROSTER_BOUND {
            issue(
                out,
                owner,
                "invalid_army",
                format!("an army stack holds 1 to {ROSTER_BOUND}"),
            );
        }
    }
    items(out, w, owner, &army.loot);
    condition(out, w, owner, army.joins.as_ref());
    if army.joins.is_some() && (army.xp > 0 || !army.loot.is_empty() || army.repeatable) {
        warn(
            out,
            owner,
            "unused_reward",
            "an ally is never defeated, so its XP, loot and repeatability do nothing",
        );
    }
}
