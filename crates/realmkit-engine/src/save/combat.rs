//! Save checks for fighting progress: gear, level, defeats, stat points,
//! techniques, vitals and an open encounter.

use super::*;
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
    let max = rules::player_stats(world, combat);
    match &combat.stance {
        Stance::Exploring(v) => ensure(
            v.hp <= max.hp && v.mp <= max.mp,
            "player vitals exceed their maximums",
        )?,
        Stance::Fighting(encounter) => encounter_state(world, state, combat, rules, encounter)?,
    }
    ensure(
        combat.xp >= progress.xp_floor
            && (progress.any_repeatable || combat.xp <= progress.xp_ceiling),
        "experience does not match progress",
    )
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
            !repeatable(world, id)
                && world.character(id).is_some_and(|c| c.combat.is_some())
                && world.locations.iter().any(|l| l.characters.contains(id))
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
        .chain(
            world
                .dialogues
                .iter()
                .flat_map(|d| &d.nodes)
                .flat_map(|n| &n.choices)
                .filter_map(|c| match &c.effect {
                    Some(DialogueEffect::GrantTechnique(grant)) => Some(grant),
                    _ => None,
                }),
        )
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
                                    && !rules::conditions_met(state, &next.requires))
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
                    && lasting(state, &e.known_when)
                    && lasting(state, &e.requires)
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
        tiers[..gear.tier.min(tiers.len())]
            .iter()
            .all(|t| lasting(state, &t.requires) && trained(state, t.trains.as_ref()))
    });
    let forged_ok = progress.forged.iter().all(|output| {
        let pieces = combat.gear.values().filter(|g| &&g.item == output).count() as u64;
        let granted = progress.inventory.get(*output).copied().unwrap_or(0);
        pieces <= granted
            || progress.repeatable_loot.contains(output)
            || rules
                .recipes
                .iter()
                .any(|r| &&r.output == output && qualified(state, r))
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
    let needs_itself = |grant: &realmkit_spec::TechniqueGrant, conditions: &[&[Condition]]| {
        conditions.iter().flat_map(|c| c.iter()).any(|c| {
            matches!(c, Condition::Technique { technique, .. } if *technique == grant.technique)
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
        let forged = pieces > granted || progress.repeatable_loot.contains(&recipe.output);
        if let Some(grant) = recipe
            .trains
            .as_ref()
            .filter(|g| forged && !needs_itself(g, &[&recipe.known_when, &recipe.requires]))
        {
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
                .filter(|g| !needs_itself(g, &[&tier.requires]))
            {
                lessons.push(grant);
            }
        }
        let enchantment = gear.enchantment.as_ref().and_then(|e| world.enchantment(e));
        if let Some(e) = enchantment {
            if let Some(grant) = e
                .trains
                .as_ref()
                .filter(|g| !needs_itself(g, &[&e.known_when, &e.requires]))
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
    let per_point =
        100 * u128::from(encounter::baseline_turn(&rules.timeline).map_err(|_| invalid())?);
    let (player, opponents) = encounter.participants.split_first().ok_or_else(invalid)?;
    let player_ok = player.character == world.world.player
        && player.side == 0
        && player.control == Control::Player;
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
    let vitals_ok = encounter.participants.iter().all(|p| {
        let max = encounter::stats(world, rules::player_stats(world, combat), p);
        // In a yielding group nobody dies, and a participant has yielded exactly
        // when a hit left it at or below its threshold. An untouched participant
        // at full HP has not yielded even if full HP is within the threshold
        // (a 100% share). Elsewhere nobody yields.
        let yield_ok = match yield_share {
            Some(share) => {
                let low = p.hp <= encounter::yield_threshold(max.hp, share);
                p.hp > 0 && (p.yielded == low || (!p.yielded && p.hp == max.hp))
            }
            None => !p.yielded,
        };
        yield_ok
            && p.hp <= max.hp
            && p.mp <= max.mp
            && u128::from(p.mp_remainder) < per_point
            && p.rage_remainder < u64::from(max.hp)
            // Only participants still fighting keep up with the schedule.
            && (!p.fighting() || p.next_time >= encounter.now)
    });
    // A live encounter always rests at the player's turn: the player acts now,
    // and every opponent later or, on a tie, after the player's side.
    let paused = player.hp == 0 || player.next_time == encounter.now;
    // A finished fight never stays open: the player still fights an opponent who
    // still fights, or the player is dead.
    let open = player.hp == 0 || (player.fighting() && opponents.iter().any(Participant::fighting));
    if vitals_ok && paused && open && state.dialogue.is_none() {
        Ok(())
    } else {
        Err(invalid())
    }
}
