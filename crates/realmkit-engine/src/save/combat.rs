//! Save checks for fighting progress: gear, level, defeats, stat points,
//! techniques, vitals and an open encounter.

use super::*;
use crate::character::{self, resources, Pools};
use realmkit_spec::Combat;

pub(super) fn check(
    world: &WorldSpec,
    state: &GameState,
    combat: &CombatState,
    rules: &Combat,
    progress: &Progress,
) -> Result<(), String> {
    // Gear first: every stat check below derives stats from worn pieces.
    gear(world, combat)?;
    level(combat, rules)?;
    defeats(world, combat)?;
    allocation(world, combat, rules)?;
    techniques(world, state, combat, rules, progress)?;
    crafting(world, state, combat, rules, progress)?;
    let max = character::player_stats(world, combat);
    let pools = resources::player_pools(world, &max);
    ensure(
        within(rules, &combat.vitals, &max, pools),
        "player vitals exceed their maximums",
    )?;
    // Rage that belongs to one fight is gone outside it.
    let fighting = matches!(combat.stance, Stance::Fighting(_) | Stance::Realtime(_));
    ensure(
        fighting
            || rules.resources.rage_carries
            || (combat.vitals.rage == 0 && combat.vitals.rage_remainder == 0),
        "rage outlasts the fight it belongs to",
    )?;
    match &combat.stance {
        Stance::Exploring => {}
        Stance::Fighting(encounter) => encounter_state(world, state, combat, rules, encounter)?,
        Stance::Realtime(duel) => super::realtime::check(world, state, combat, rules, duel)?,
        Stance::Battle(battle) => super::battle::check(world, state, combat, battle)?,
    }
    ensure(
        combat.xp >= progress.xp_floor
            && (progress.any_repeatable || combat.xp <= progress.xp_ceiling),
        "experience does not match progress",
    )
}

/// Vitals the rules could leave at stats `max` and `pools`: HP and each
/// resource within its ceiling, nothing in a resource the character does not
/// use, and damage-rage progress below one point (none at a world's cap).
pub(super) fn within(rules: &Combat, v: &Vitals, max: &Stats, pools: Pools) -> bool {
    let capped = rules.resources.rage_cap.is_some() && v.rage == pools.rage_cap();
    v.hp <= max.hp
        && v.mp <= pools.mp_max()
        && v.rage <= pools.rage_cap()
        && v.rage_remainder < u64::from(max.hp)
        && (pools.rage.is_some() || v.rage_remainder == 0)
        && !(capped && v.rage_remainder > 0)
}

/// Pieces are real equipment at a tier it has, with IDs below the counter,
/// and worn pieces never share a slot.
fn gear(world: &WorldSpec, combat: &CombatState) -> Result<(), String> {
    let mut worn = BTreeSet::new();
    ensure(
        combat.gear.iter().all(|(id, piece)| {
            *id < combat.next_gear
                && world
                    .item(&piece.item)
                    .and_then(|i| i.equipment.as_ref())
                    .is_some_and(|e| {
                        piece.tier <= e.tiers.len()
                            && (!piece.equipped
                                || e.slots.iter().all(|slot| worn.insert(slot.clone())))
                    })
        }),
        "invalid equipment",
    )
}

fn level(combat: &CombatState, rules: &Combat) -> Result<(), String> {
    let stats = combat
        .level
        .checked_sub(1)
        .and_then(|i| rules.levels.get(i))
        .ok_or("invalid player level")?;
    let next = rules.levels.get(combat.level);
    ensure(
        combat.xp >= stats.xp && next.is_none_or(|next| combat.xp < next.xp),
        "experience does not match level",
    )
}

/// Only placed, non-repeatable fighters are recorded as defeated.
fn defeats(world: &WorldSpec, combat: &CombatState) -> Result<(), String> {
    ensure(
        combat.defeated.iter().all(|id| {
            let character = world.character(id);
            // A fighter outside a repeatable group, or a hostile army that is not repeatable.
            let fighter = character.is_some_and(|c| c.combat.is_some()) && !repeatable(world, id);
            let army = character
                .and_then(|c| c.army.as_ref())
                .is_some_and(|a| a.joins.is_none() && !a.repeatable);
            (fighter || army) && world.locations.iter().any(|l| l.characters.contains(id))
        }),
        "invalid defeated characters",
    )
}

/// Only stats that accept points, within caps and the points granted so far.
fn allocation(world: &WorldSpec, combat: &CombatState, rules: &Combat) -> Result<(), String> {
    let spec = rules.stat_points.as_ref();
    let spent: u64 = combat.allocation.values().map(|p| u64::from(*p)).sum();
    ensure(
        combat.allocation.iter().all(|(stat, points)| {
            spec.is_some_and(|s| {
                s.values.contains_key(stat) && s.caps.get(stat).is_none_or(|cap| points <= cap)
            })
        }) && spent <= rules::granted_points(world, combat.level),
        "invalid stat point allocation",
    )
}

/// Techniques some grant can teach, at an existing rank, with XP at least that
/// rank's threshold and at the next threshold only while its gate is closed.
/// Starting techniques are never forgotten.
fn techniques(
    world: &WorldSpec,
    state: &GameState,
    combat: &CombatState,
    rules: &Combat,
    progress: &Progress,
) -> Result<(), String> {
    let crafting = crafting_lessons(world, combat, rules, progress);
    let teachable: BTreeSet<&Id> = rules
        .player_techniques
        .iter()
        .chain(crafting)
        .chain(world.quests.iter().flat_map(|q| &q.reward_techniques))
        .chain(progress.fired.iter().filter_map(|e| match e {
            Effect::GrantTechnique(grant) => Some(grant),
            _ => None,
        }))
        .map(|g| &g.technique)
        .collect();
    ensure(
        combat.techniques.iter().all(|(id, learned)| {
            teachable.contains(id)
                && world.technique(id).is_some_and(|t| {
                    let Some(rank) = learned.rank.checked_sub(1).and_then(|i| t.ranks.get(i))
                    else {
                        return false;
                    };
                    learned.xp >= rank.xp
                        && t.ranks.get(learned.rank).is_none_or(|next| {
                            learned.xp < next.xp
                                || (learned.xp == next.xp
                                    && !rules::allowed(state, next.requires.as_ref()))
                        })
                })
        }) && rules.player_techniques.iter().all(|grant| {
            combat
                .techniques
                .get(&grant.technique)
                .is_some_and(|t| t.rank >= grant.rank.unwrap_or(1))
        }),
        "invalid technique state",
    )
}

/// Crafting the player qualified for: every tier a piece rose through, and
/// some recipe for each piece forged beyond the grants, had its lasting
/// conditions met and its trained technique learned.
fn crafting(
    world: &WorldSpec,
    state: &GameState,
    combat: &CombatState,
    rules: &Combat,
    progress: &Progress,
) -> Result<(), String> {
    // An enchantment that exists, fits its piece, and was the player's to lay.
    let enchantments_ok = combat.gear.values().all(|gear| {
        let equipment = world.item(&gear.item).unwrap().equipment.as_ref().unwrap();
        gear.enchantment.as_ref().is_none_or(|id| {
            world.enchantment(id).is_some_and(|e| {
                e.fits(equipment)
                    && lasting(world, state, e.known_when.as_ref())
                    && lasting(world, state, e.requires.as_ref())
                    && trained(state, e.trains.as_ref())
            })
        })
    });
    let tiers_ok = combat.gear.values().all(|gear| {
        let tiers = &world
            .item(&gear.item)
            .unwrap()
            .equipment
            .as_ref()
            .unwrap()
            .tiers;
        tiers[..gear.tier.min(tiers.len())].iter().all(|t| {
            lasting(world, state, t.requires.as_ref()) && trained(state, t.trains.as_ref())
        })
    });
    let forged_ok = progress.forged.iter().all(|output| {
        let pieces = combat.gear.values().filter(|g| &&g.item == output).count() as u64;
        let granted = progress.inventory.get(*output).copied().unwrap_or(0);
        pieces <= granted
            || progress.renewable.contains(output)
            || progress.loose.contains(output)
            || rules
                .recipes
                .iter()
                .any(|r| &&r.output == output && qualified(world, state, r))
    });
    ensure(
        tiers_ok && forged_ok && enchantments_ok,
        "crafting the player was not able to do",
    )
}

/// Techniques crafting could have taught, shown by the crafting it left
/// behind: a forged piece, a tier reached, an enchantment laid. A lesson
/// whose own conditions need the technique it teaches explains nothing.
fn crafting_lessons<'w>(
    world: &'w WorldSpec,
    combat: &CombatState,
    rules: &'w Combat,
    progress: &Progress,
) -> Vec<&'w realmkit_spec::TechniqueGrant> {
    let needs_itself = |grant: &realmkit_spec::TechniqueGrant,
                        conditions: &[Option<&Condition>]| {
        conditions.iter().flatten().any(|c| {
                c.requires(&|leaf| {
                    matches!(leaf, Condition::Technique { technique, .. } if *technique == grant.technique)
                })
            })
    };
    let mut lessons = Vec::new();
    for recipe in &rules.recipes {
        let pieces = combat
            .gear
            .values()
            .filter(|g| g.item == recipe.output)
            .count() as u64;
        let granted = progress.inventory.get(&recipe.output).copied().unwrap_or(0);
        // Pieces are never taken away, so only a piece beyond those granted
        // can have been forged; one bought or looted again is no lesson, but
        // with no way to tell them apart, any extra piece may be.
        let forged = pieces > granted;
        if let Some(grant) = recipe.trains.as_ref().filter(|g| {
            forged && !needs_itself(g, &[recipe.known_when.as_ref(), recipe.requires.as_ref()])
        }) {
            lessons.push(grant);
        }
    }
    for gear in combat.gear.values() {
        let tiers = &world
            .item(&gear.item)
            .unwrap()
            .equipment
            .as_ref()
            .unwrap()
            .tiers;
        for tier in &tiers[..gear.tier.min(tiers.len())] {
            if let Some(grant) = tier
                .trains
                .as_ref()
                .filter(|g| !needs_itself(g, &[tier.requires.as_ref()]))
            {
                lessons.push(grant);
            }
        }
        let enchantment = gear.enchantment.as_ref().and_then(|e| world.enchantment(e));
        if let Some(e) = enchantment {
            if let Some(grant) = e
                .trains
                .as_ref()
                .filter(|g| !needs_itself(g, &[e.known_when.as_ref(), e.requires.as_ref()]))
            {
                lessons.push(grant);
            }
        }
    }
    lessons
}

/// An encounter is consistent with the rules that produce it: the player
/// first, opponents present and undefeated, vitals within maxima, remainders
/// below one point and turns not in the past.
fn encounter_state(
    world: &WorldSpec,
    state: &GameState,
    combat: &CombatState,
    rules: &Combat,
    encounter: &Encounter,
) -> Result<(), String> {
    let invalid = || "invalid encounter state".to_string();
    let timeline = rules.timeline.as_ref().ok_or_else(invalid)?;
    let per_point = 100 * u128::from(encounter::baseline_turn(timeline).map_err(|_| invalid())?);
    let (player, opponents) = encounter.participants.split_first().ok_or_else(invalid)?;
    // The player's vitals are the character's own; each opponent's, the encounter's.
    let player_ok = player.character == world.world.player
        && player.side == 0
        && player.control == Control::Player
        && player.vitals.is_none();
    // Exactly the opponents engaging the first one would bring in.
    let expected = opponents
        .first()
        .filter(|p| rules::character_here(world, state, &p.character).is_some())
        .map(|p| encounter::opponents_for(world, state, &p.character));
    let listed: Vec<_> = opponents.iter().map(|p| p.character.clone()).collect();
    let opponents_ok = expected.as_ref() == Some(&listed)
        && opponents.iter().all(|p| {
            p.side == 1
                && p.control == Control::Policy
                && p.vitals.is_some()
                && !combat.defeated.contains(&p.character)
                && rules::character_here(world, state, &p.character)
                    .is_some_and(|c| c.combat.is_some())
        });
    let ids: BTreeSet<_> = encounter
        .participants
        .iter()
        .map(|p| &p.character)
        .collect();
    // Identities first: stats exist only for the player and characters that can fight.
    if !(player_ok && opponents_ok && ids.len() == encounter.participants.len()) {
        return Err(invalid());
    }
    let yield_share = encounter::group(world, encounter).and_then(|g| g.yield_share);
    let me = character::player_stats(world, combat);
    let fighting = |i: usize| encounter.fighting(i, &combat.vitals);
    let vitals_ok = encounter.participants.iter().enumerate().all(|(i, p)| {
        let max = encounter::stats(world, me, p);
        let pools = match p.control {
            Control::Player => resources::player_pools(world, &me),
            Control::Policy => {
                resources::profile_pools(world, character::profile(world, &p.character))
            }
        };
        let v = encounter.vitals(i, &combat.vitals);
        // In a yielding group nobody dies, and a participant has yielded exactly
        // when a hit left it at or below its threshold. An untouched participant
        // at full HP has not yielded even if full HP is within the threshold
        // (a 100% share). Elsewhere nobody yields.
        let yield_ok = match yield_share {
            Some(share) => {
                let low = v.hp <= encounter::yield_threshold(max.hp, share);
                v.hp > 0 && (p.yielded == low || (!p.yielded && v.hp == max.hp))
            }
            None => !p.yielded,
        };
        yield_ok
            && within(rules, v, &max, pools)
            && u128::from(p.mp_remainder) < per_point
            // Only participants still fighting keep up with the schedule.
            && (!fighting(i) || p.next_time >= encounter.now)
    });
    // A live encounter always rests at the player's turn: the player acts now,
    // and every opponent later or, on a tie, after the player's side.
    let dead = combat.vitals.hp == 0;
    let paused = dead || player.next_time == encounter.now;
    // A finished fight never stays open: the player still fights an opponent who
    // still fights, or the player is dead.
    let open = dead || (fighting(0) && (1..encounter.participants.len()).any(fighting));
    if vitals_ok && paused && open && state.dialogue.is_none() {
        Ok(())
    } else {
        Err(invalid())
    }
}
