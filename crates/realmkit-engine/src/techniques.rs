//! Learned techniques: named ranks reached through technique XP (use in
//! encounters, a share of victories) or teaching, held back by breakthrough
//! gates until their conditions hold.

use super::*;
use realmkit_spec::{Technique, TechniqueGrant};

fn spec<'w>(world: &'w WorldSpec, id: &str) -> &'w Technique {
    world.technique(id).unwrap()
}

/// The skills the player can use: authored player skills plus each learned
/// technique's skill at its current rank.
pub(super) fn player_skills<'w>(world: &'w WorldSpec, combat: &CombatState) -> Vec<&'w Skill> {
    let rules = world.combat().unwrap();
    let techniques = combat
        .techniques
        .iter()
        .filter_map(|(id, learned)| spec(world, id).ranks[learned.rank - 1].skill.as_deref());
    rules
        .player_skills
        .iter()
        .map(String::as_str)
        .chain(techniques)
        .filter_map(|id| world.skill(id))
        .collect()
}

/// The learned technique whose current rank is used as `skill`, if any.
pub(super) fn of_skill<'w>(
    world: &'w WorldSpec,
    combat: &CombatState,
    skill: &str,
) -> Option<&'w str> {
    combat.techniques.iter().find_map(|(id, learned)| {
        let rank = &spec(world, id).ranks[learned.rank - 1];
        (rank.skill.as_deref() == Some(skill)).then_some(spec(world, id).id.as_str())
    })
}

/// Each learned technique's current rank bonus; a rank's bonus replaces the
/// previous rank's.
pub(super) fn add_passives(world: &WorldSpec, combat: &CombatState, stats: &mut Stats) {
    for (id, learned) in &combat.techniques {
        for (stat, bonus) in &spec(world, id).ranks[learned.rank - 1].passive {
            let total = stats.get_mut(*stat);
            *total = total.saturating_add(*bonus);
        }
    }
}

/// Teaches the technique if unknown, raises it to at least the grant's rank
/// (teaching passes gates), then adds the grant's XP.
pub(super) fn grant(
    world: &WorldSpec,
    state: &mut GameState,
    grant: &TechniqueGrant,
    events: &mut Vec<Event>,
) -> Result<(), EngineError> {
    let technique = spec(world, &grant.technique);
    let combat = state.combat.as_mut().unwrap();
    let learned = combat
        .techniques
        .entry(technique.id.clone())
        .or_insert_with(|| {
            events.push(Event::TechniqueLearned {
                technique: technique.id.clone(),
            });
            TechniqueState { rank: 1, xp: 0 }
        });
    if let Some(rank) = grant.rank.filter(|r| *r > learned.rank) {
        learned.rank = rank;
        learned.xp = learned.xp.max(technique.ranks[rank - 1].xp);
        events.push(Event::TechniqueRankUp {
            technique: technique.id.clone(),
            rank,
        });
    }
    add_xp(world, state, &technique.id, grant.xp, events)?;
    clamp_vitals(world, state);
    Ok(())
}

/// A rank's bonus replaces the previous one's and may be smaller, so current
/// HP and MP never stay above the maxima the new ranks give.
fn clamp_vitals(world: &WorldSpec, state: &mut GameState) {
    let combat = state.combat.as_mut().unwrap();
    let max = rules::player_stats(world, combat);
    let (hp, mp) = match &mut combat.stance {
        Stance::Exploring(vitals) => (&mut vitals.hp, &mut vitals.mp),
        Stance::Fighting(encounter) => {
            let player = &mut encounter.participants[0];
            // Rage progress is counted in maximum HP: carry whole points over.
            let whole = player.rage_remainder / u64::from(max.hp);
            player.rage = player
                .rage
                .saturating_add(whole.try_into().unwrap_or(u32::MAX));
            player.rage_remainder %= u64::from(max.hp);
            (&mut player.hp, &mut player.mp)
        }
    };
    *hp = (*hp).min(max.hp);
    *mp = (*mp).min(max.mp);
}

/// Adds technique XP, then promotes through every open rank.
pub(super) fn add_xp(
    world: &WorldSpec,
    state: &mut GameState,
    technique: &str,
    amount: u64,
    events: &mut Vec<Event>,
) -> Result<(), EngineError> {
    if amount == 0 {
        return Ok(());
    }
    // A mastered technique has nothing left to learn.
    let learned = state.combat.as_ref().unwrap().techniques[technique];
    if learned.rank == spec(world, technique).ranks.len() {
        return Ok(());
    }
    // XP stops at the first closed gate it reaches; only what is kept counts.
    let raw = learned.xp.checked_add(amount);
    let mut kept = raw.unwrap_or(u64::MAX);
    let mut capped = false;
    for next in &spec(world, technique).ranks[learned.rank..] {
        if kept < next.xp {
            break;
        }
        if !rules::conditions_met(state, &next.requires) {
            (kept, capped) = (next.xp, true);
            break;
        }
    }
    if !capped && raw.is_none() {
        return Err(EngineError::NumericLimit);
    }
    let gained = kept - learned.xp;
    if gained == 0 {
        return Ok(());
    }
    let combat = state.combat.as_mut().unwrap();
    combat.techniques.get_mut(technique).unwrap().xp = kept;
    events.push(Event::TechniqueXpGained {
        technique: technique.into(),
        amount: gained,
    });
    promote(world, state, events);
    Ok(())
}

/// Rises through every rank whose threshold is met and whose gate holds; XP
/// waits at a closed gate's threshold. Run after every change, so opening a
/// gate promotes a technique that was waiting.
pub(super) fn promote(world: &WorldSpec, state: &mut GameState, events: &mut Vec<Event>) {
    let Some(combat) = &state.combat else {
        return;
    };
    let ids: Vec<Id> = combat.techniques.keys().cloned().collect();
    for id in ids {
        let technique = spec(world, &id);
        loop {
            let learned = state.combat.as_ref().unwrap().techniques[&id];
            let Some(next) = technique.ranks.get(learned.rank) else {
                break;
            };
            if learned.xp < next.xp {
                break;
            }
            let open = rules::conditions_met(state, &next.requires);
            let learned = state
                .combat
                .as_mut()
                .unwrap()
                .techniques
                .get_mut(&id)
                .unwrap();
            if !open {
                learned.xp = next.xp;
                break;
            }
            learned.rank += 1;
            events.push(Event::TechniqueRankUp {
                technique: id.clone(),
                rank: learned.rank,
            });
        }
    }
    clamp_vitals(world, state);
}
