//! How a personal fight ends, however it was run: the player returns to
//! exploring and the outcome is settled exactly once, in the same command
//! that ends the fight. The turn-based and real-time layers both end here.

use super::*;
use crate::character::{profile, resources};
use realmkit_spec::Group;

/// The player returns to exploring, with any Rage that belongs to one fight
/// gone. A victory then grants each opponent in `fallen` its loot and XP
/// (scaled by level difference), records it unless its group is repeatable,
/// advances defeat objectives and sets the group's victory flags; the
/// player's yield sets its defeat flags; a flight grants and records nothing.
/// The stance is gone before anything is granted, so nothing can settle twice.
pub(crate) fn settle(
    world: &WorldSpec,
    state: &mut GameState,
    outcome: Outcome,
    group: Option<&Group>,
    fallen: Vec<Id>,
    events: &mut Vec<Event>,
) -> Result<(), EngineError> {
    let combat = state.combat.as_mut().unwrap();
    resources::after_fight(world, &mut combat.vitals);
    combat.stance = Stance::Exploring;
    events.push(Event::EncounterEnded { outcome });
    let flags = match outcome {
        Outcome::Victory => group.map_or(&[][..], |g| &g.victory_flags[..]),
        Outcome::Yielded => group.map_or(&[][..], |g| &g.defeat_flags[..]),
        Outcome::Fled => return Ok(()),
    };
    if outcome == Outcome::Victory {
        let repeatable = group.is_some_and(|g| g.repeatable);
        victory_rewards(world, state, fallen, repeatable, events)?;
    }
    for flag in flags {
        rules::set_flag(world, state, flag, events);
    }
    Ok(())
}

/// Each fallen opponent's loot and XP, scaled from the level the player
/// fought at; its defeat is recorded unless the group is repeatable, and
/// advances defeat objectives. Passive arts then take their share.
fn victory_rewards(
    world: &WorldSpec,
    state: &mut GameState,
    fallen: Vec<Id>,
    repeatable: bool,
    events: &mut Vec<Event>,
) -> Result<(), EngineError> {
    // Every reward scales from the level the player fought at, so a
    // level-up from one opponent does not change the next one's XP.
    let level = state.combat.as_ref().unwrap().level;
    let mut earned = 0_u64;
    for id in fallen {
        let profile = profile(world, &id);
        let combat = state.combat.as_mut().unwrap();
        if !repeatable {
            combat.defeated.insert(id.clone());
        }
        let xp = xp_for_defeat(profile.xp, level, profile.level);
        earned = earned.saturating_add(xp);
        rules::grant_items(world, state, &profile.loot, events)?;
        rules::grant_xp(world, state, xp, events)?;
        let defeat = QuestObjective::Defeat { character: id };
        for q in &world.quests {
            if q.objective == defeat && state.quests[&q.id] == QuestStatus::Active {
                rules::progress(state, &q.id, events);
            }
        }
    }
    // Passive arts deepen by their share of the victory's XP.
    let shares: Vec<(Id, u64)> = state
        .combat
        .as_ref()
        .unwrap()
        .techniques
        .keys()
        .map(|id| {
            let share = u128::from(world.technique(id).unwrap().xp_share_percent);
            (id.clone(), (u128::from(earned) * share / 100) as u64)
        })
        .collect();
    for (id, gain) in shares {
        techniques::add_xp(world, state, &id, gain, events)?;
    }
    Ok(())
}
