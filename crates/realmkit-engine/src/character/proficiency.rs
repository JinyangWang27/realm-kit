//! Weapon proficiency: direct XP per family and discipline is all that is
//! kept; ranks, borrowed rank and an attack's timing derive from it. A family
//! borrows only from another's direct rank, the single best donor, so there
//! are no chains, no stacking and no dependence on content order. Learning a
//! move is a technique's business: nothing here teaches one.

use super::*;
use realmkit_spec::{Equipment, WeaponProficiency};

/// A family's rank three ways.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FamilyStanding {
    pub direct: u32,
    pub borrowed: u32,
    pub effective: u32,
}

/// The weapon in hand and what training gives it now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeaponStanding {
    pub family: Id,
    pub rank: FamilyStanding,
    /// The handling disciplines' ranks, weighted by the weapon's profile.
    pub handling: u32,
    /// Family and handling in the world's proportion, 0 to 100.
    pub competence: u32,
    pub windup_cut: u32,
    pub recovery_cut: u32,
}

fn rules(world: &WorldSpec) -> Option<&WeaponProficiency> {
    world.combat()?.weapon_proficiency.as_ref()
}

/// The highest rank `xp` reaches.
fn rank(rules: &WeaponProficiency, xp: u64) -> u32 {
    let reached = rules.ranks.iter().take_while(|r| r.xp <= xp);
    reached.last().map_or(0, |r| r.rank)
}

fn direct(rules: &WeaponProficiency, combat: &CombatState, family: &str) -> u32 {
    let xp = combat.weapon_training.families.get(family);
    rank(rules, xp.copied().unwrap_or(0))
}

fn discipline_rank(rules: &WeaponProficiency, combat: &CombatState, discipline: &str) -> u32 {
    let xp = combat.weapon_training.disciplines.get(discipline);
    rank(rules, xp.copied().unwrap_or(0))
}

/// `borrowed = max over donors of floor(affinity × direct / 100)`, and
/// `effective = direct + floor((100 − direct) × borrowed / 100)`, which is
/// within 0 to 100 and never falls as direct rank rises.
fn standing_in(rules: &WeaponProficiency, combat: &CombatState, family: &str) -> FamilyStanding {
    let own = direct(rules, combat, family);
    let borrowed = rules
        .families
        .iter()
        .filter_map(|donor| {
            let affinity = donor.affinities.get(family)?;
            Some(affinity * direct(rules, combat, &donor.id) / 100)
        })
        .max()
        .unwrap_or(0);
    FamilyStanding {
        direct: own,
        borrowed,
        effective: own + (100 - own) * borrowed / 100,
    }
}

pub(crate) fn family(
    world: &WorldSpec,
    combat: &CombatState,
    family: &str,
) -> Option<FamilyStanding> {
    let rules = rules(world)?;
    let known = rules.families.iter().any(|f| f.id == family);
    known.then(|| standing_in(rules, combat, family))
}

pub(crate) fn discipline(world: &WorldSpec, combat: &CombatState, discipline: &str) -> Option<u32> {
    let rules = rules(world)?;
    let known = rules.disciplines.iter().any(|d| d.id == discipline);
    known.then(|| discipline_rank(rules, combat, discipline))
}

/// The worn piece that trains a family; validation lets only one be worn.
fn in_hand<'w>(world: &'w WorldSpec, combat: &CombatState) -> Option<&'w Equipment> {
    rules(world)?;
    gear::worn(world, combat)
        .into_iter()
        .find(|p| p.weapon.is_some())
}

pub(crate) fn standing(world: &WorldSpec, combat: &CombatState) -> Option<WeaponStanding> {
    let rules = rules(world)?;
    let profile = in_hand(world, combat)?.weapon.as_ref()?;
    let rank = standing_in(rules, combat, &profile.family);
    // A weight inside one layer, so handling never adds to itself.
    let handling = profile
        .handling
        .iter()
        .map(|(d, weight)| weight * discipline_rank(rules, combat, d))
        .sum::<u32>()
        / 100;
    let competence =
        (rules.family_percent * rank.effective + (100 - rules.family_percent) * handling) / 100;
    Some(WeaponStanding {
        family: profile.family.clone(),
        rank,
        handling,
        competence,
        windup_cut: rules.windup_percent * competence / 100,
        recovery_cut: rules.recovery_percent * competence / 100,
    })
}

/// Whether the attack is made with the weapon in hand: a basic attack is, and
/// a skill that requires something the weapon provides, if it is of a family
/// the skill allows.
fn uses(weapon: &Equipment, skill: Option<&Skill>) -> bool {
    skill.is_none_or(|s| {
        s.requires.iter().any(|c| weapon.capabilities.contains(c)) && allows_family(weapon, s)
    })
}

fn allows_family(weapon: &Equipment, skill: &Skill) -> bool {
    let Some(mastery) = skill.mastery.as_ref().filter(|m| !m.families.is_empty()) else {
        return true;
    };
    let family = weapon.weapon.as_ref().map(|w| &w.family);
    family.is_some_and(|f| mastery.families.contains(f))
}

/// Percent off the wind-up and recovery of the player's attack with `skill`.
pub(crate) fn cuts(world: &WorldSpec, combat: &CombatState, skill: Option<&Skill>) -> (u32, u32) {
    in_hand(world, combat)
        .filter(|weapon| uses(weapon, skill))
        .and_then(|_| standing(world, combat))
        .map_or((0, 0), |w| (w.windup_cut, w.recovery_cut))
}

/// Whether the weapon in hand is one `skill` allows, and the player has the
/// rank in its family that it asks. A family-specific skill refuses every other
/// weapon outright, whatever its capabilities and rank; so do bare hands.
pub(crate) fn qualifies(
    world: &WorldSpec,
    combat: &CombatState,
    skill: &Skill,
) -> Result<(), EngineError> {
    let Some(mastery) = &skill.mastery else {
        return Ok(());
    };
    let weapon = in_hand(world, combat);
    let allowed = match weapon {
        Some(weapon) => allows_family(weapon, skill),
        None => mastery.families.is_empty(),
    };
    if !allowed {
        return Err(EngineError::Unwielded(skill.id.clone()));
    }
    let rank = weapon
        .filter(|w| uses(w, Some(skill)))
        .and_then(|_| standing(world, combat))
        .map_or((0, 0), |w| (w.rank.direct, w.rank.effective));
    match rank.0 >= mastery.direct && rank.1 >= mastery.effective {
        true => Ok(()),
        false => Err(EngineError::Unpracticed(skill.id.clone())),
    }
}

/// The player's weapon attack did damage to `foe`.
pub(crate) fn struck(
    world: &WorldSpec,
    state: &mut GameState,
    skill: Option<&Skill>,
    foe: &str,
    events: &mut Vec<Event>,
) -> Result<(), EngineError> {
    let Some(rules) = rules(world) else {
        return Ok(());
    };
    let combat = state.combat.as_ref().unwrap();
    match in_hand(world, combat) {
        Some(weapon) if uses(weapon, skill) => teach(world, state, rules.xp.hit, foe, events),
        _ => Ok(()),
    }
}

/// The player's weapon stopped a blow from `foe`.
pub(crate) fn defended(
    world: &WorldSpec,
    state: &mut GameState,
    foe: &str,
    events: &mut Vec<Event>,
) -> Result<(), EngineError> {
    let Some(rules) = rules(world) else {
        return Ok(());
    };
    teach(world, state, rules.xp.defense, foe, events)
}

/// How much of `amount` a foe of `challenge` teaches someone of `effective` rank:
/// all of it at or below the challenge, evenly less above, nothing at
/// `margin` past it, rounded down.
fn challenged(amount: u32, effective: u32, challenge: u32, margin: u32) -> u64 {
    let past = effective.saturating_sub(challenge);
    u64::from(amount) * u64::from(margin.saturating_sub(past)) / u64::from(margin)
}

/// `amount` XP, falling off as the weapon's rank passes the foe's challenge, up
/// to what `foe` has left to teach, to the weapon in hand's family and, by its
/// weights, its disciplines. Character level plays no part.
fn teach(
    world: &WorldSpec,
    state: &mut GameState,
    amount: u32,
    foe: &str,
    events: &mut Vec<Event>,
) -> Result<(), EngineError> {
    let rules = rules(world).unwrap();
    let combat = state.combat.as_mut().unwrap();
    let Some(profile) = in_hand(world, combat).and_then(|w| w.weapon.as_ref()) else {
        return Ok(());
    };
    let target = character::profile(world, foe);
    if target.inert {
        return Ok(());
    }
    let taught = combat.weapon_training.taught.get(foe).copied().unwrap_or(0);
    let left = u64::from(rules.xp.target_budget).saturating_sub(taught);
    let effective = standing_in(rules, combat, &profile.family).effective;
    let by_level = (target.level as u64).saturating_mul(u64::from(rules.xp.challenge_per_level));
    let challenge = target.weapon_challenge.unwrap_or(by_level.min(100) as u32);
    let gain = challenged(amount, effective, challenge, rules.xp.trivial_margin).min(left);
    if gain == 0 {
        return Ok(());
    }
    let added = |xp: u64| xp.checked_add(gain).ok_or(EngineError::NumericLimit);
    let training = &mut combat.weapon_training;
    let family = training.families.get(&profile.family).copied().unwrap_or(0);
    let total = added(family)?;
    training.families.insert(profile.family.clone(), total);
    training.taught.insert(foe.into(), taught + gain);
    events.push(Event::WeaponTrained {
        family: profile.family.clone(),
        amount: gain,
    });
    let risen = rank(rules, total);
    if risen > rank(rules, family) {
        events.push(Event::WeaponRankUp {
            family: profile.family.clone(),
            rank: risen,
        });
    }
    for (discipline, share) in split(gain, &profile.handling) {
        if share == 0 {
            continue;
        }
        let before = training.disciplines.get(discipline).copied().unwrap_or(0);
        let after = before.checked_add(share).ok_or(EngineError::NumericLimit)?;
        training.disciplines.insert(discipline.clone(), after);
        let risen = rank(rules, after);
        if risen > rank(rules, before) {
            events.push(Event::DisciplineRankUp {
                discipline: discipline.clone(),
                rank: risen,
            });
        }
    }
    Ok(())
}

/// `gain` by weights totalling 100, whole points that add up to it exactly:
/// floors, then one more to each largest remainder, ties by ID.
fn split(gain: u64, weights: &BTreeMap<Id, u32>) -> Vec<(&Id, u64)> {
    let mut shares: Vec<_> = weights
        .iter()
        .map(|(id, w)| (id, gain * u64::from(*w) / 100, gain * u64::from(*w) % 100))
        .collect();
    let spare = gain - shares.iter().map(|s| s.1).sum::<u64>();
    let mut order: Vec<usize> = (0..shares.len()).collect();
    // A stable sort keeps ID order among equal remainders.
    order.sort_by_key(|&i| std::cmp::Reverse(shares[i].2));
    for &i in order.iter().take(spare as usize) {
        shares[i].1 += 1;
    }
    shares
        .into_iter()
        .map(|(id, share, _)| (id, share))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::challenged;

    #[test]
    fn a_foe_teaches_all_up_to_its_challenge_then_evenly_less() {
        assert_eq!(challenged(10, 0, 30, 40), 10);
        assert_eq!(challenged(10, 30, 30, 40), 10);
        assert_eq!(challenged(10, 50, 30, 40), 5);
        assert_eq!(challenged(10, 69, 30, 40), 0, "rounded down");
        assert_eq!(challenged(10, 70, 30, 40), 0);
        assert_eq!(challenged(10, 100, 30, 40), 0);
        assert_eq!(challenged(10, 100, 100, 1), 10);
        assert_eq!(challenged(1_000_000, 100, 0, 100), 0);
    }
}
