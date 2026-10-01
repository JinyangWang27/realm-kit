//! The commands a client may offer in the current scene.

use super::*;

pub(crate) fn actions(world: &WorldSpec, state: &GameState) -> Vec<Action> {
    let available = |command| Action {
        command,
        available: true,
    };
    let mut panels = vec![
        available(Command::Inventory),
        available(Command::Status),
        available(Command::Quests),
    ];
    if world.combat().is_some_and(|c| !c.techniques.is_empty()) {
        panels.push(available(Command::Techniques));
    }
    // Death is not a locked door: offer only what can still be done.
    if dead(state) {
        return panels;
    }
    if let Some(encounter) = fighting(state) {
        return fight_actions(world, state, encounter, panels);
    }
    let location = world.location(&state.player.location).unwrap();
    let mut actions: Vec<_> = location
        .characters
        .iter()
        .filter(|id| npc_here(world, state, id))
        .map(|id| available(Command::Talk(id.clone())))
        .collect();
    if state.combat.is_some() {
        actions.extend(
            location
                .characters
                .iter()
                .filter(|id| !defeated(state, id))
                .filter(|id| character_here(world, state, id).is_some_and(|c| c.combat.is_some()))
                .map(|id| available(Command::Engage(id.clone()))),
        );
    }
    actions.extend(location.exits.iter().map(|(direction, exit)| Action {
        command: Command::Move(*direction),
        available: allowed(state, exit.requires.as_ref()),
    }));
    if state.combat.is_some() && location.safe {
        actions.push(available(Command::Rest));
    }
    if let (Some(combat), Some(points)) = (
        &state.combat,
        world.combat().and_then(|c| c.stat_points.as_ref()),
    ) {
        // One point at a time into each stat that can still take one.
        if unspent_points(world, combat) > 0 {
            for stat in points.values.keys() {
                let spent = combat.allocation.get(stat).copied().unwrap_or(0);
                if points.caps.get(stat).is_none_or(|cap| spent < *cap) {
                    actions.push(available(Command::Allocate {
                        stat: *stat,
                        points: 1,
                    }));
                }
            }
        }
        if points.respec == Respec::Safe && location.safe && !combat.allocation.is_empty() {
            actions.push(available(Command::Respec));
        }
    }
    // Pieces in the pack can be worn; removing is a typed command, since
    // wearing another piece already swaps.
    if let Some(combat) = &state.combat {
        actions.extend(
            combat
                .gear
                .iter()
                .filter(|(_, g)| !g.equipped)
                .map(|(id, _)| available(Command::Equip(*id))),
        );
    }
    actions.extend(crafting::offered(world, state));
    actions.extend(panels);
    actions
}

/// Attacks and skills on every opponent still fighting, then flight if the
/// group allows it.
fn fight_actions(
    world: &WorldSpec,
    state: &GameState,
    encounter: &Encounter,
    panels: Vec<Action>,
) -> Vec<Action> {
    let available = |command| Action {
        command,
        available: true,
    };
    let level = state.combat.as_ref().unwrap().level;
    let player = &encounter.participants[0];
    let skills: Vec<_> = techniques::player_skills(world, state.combat.as_ref().unwrap())
        .into_iter()
        .filter(|s| s.level <= level)
        .collect();
    let mut actions = Vec::new();
    for foe in encounter
        .participants
        .iter()
        .filter(|p| p.side != 0 && p.fighting())
    {
        actions.push(available(Command::Attack(foe.character.clone())));
        // Unaffordable skills stay listed, so the player sees why they are not usable.
        actions.extend(skills.iter().map(|s| Action {
            command: Command::UseSkill {
                skill: s.id.clone(),
                target: foe.character.clone(),
            },
            available: encounter::check_skill(player, level, s).is_ok(),
        }));
    }
    if !encounter::group(world, encounter).is_some_and(|g| g.no_flee) {
        actions.push(available(Command::Flee));
    }
    actions.extend(panels);
    actions
}
