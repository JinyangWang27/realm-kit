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
    let here = placed_here(world, state);
    let mut actions: Vec<_> = here
        .iter()
        .filter(|id| npc_here(world, state, id))
        .map(|id| available(Command::Talk((*id).clone())))
        .collect();
    if state.combat.is_some() {
        actions.extend(
            here.iter()
                .filter(|id| !defeated(state, id))
                .filter(|id| character_here(world, state, id).is_some_and(|c| c.combat.is_some()))
                .map(|id| available(Command::Engage((*id).clone()))),
        );
    }
    actions.extend(location.exits.iter().map(|(direction, exit)| Action {
        command: Command::Move(*direction),
        available: allowed(state, exit.requires.as_ref()),
    }));
    // Roads in authored order; a closed one is listed to explain itself.
    actions.extend(world.world.roads.iter().filter_map(|road| {
        Some(Action {
            command: Command::Travel(road.leads(&location.id)?.clone()),
            available: allowed(state, road.requires.as_ref()),
        })
    }));
    if let Some(step) = world.world.time.as_ref().and_then(|t| t.wait) {
        actions.push(available(Command::Wait(step)));
    }
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
    actions.extend(consumables(world, state));
    actions.extend(crafting::offered(world, state));
    actions.extend(trade(world, state));
    actions.extend(panels);
    actions
}

/// One entry per carried consumable, in inventory order; unavailable when
/// it would restore nothing.
fn consumables(world: &WorldSpec, state: &GameState) -> Vec<Action> {
    let (Some(combat), Some(now)) = (&state.combat, player_vitals(state)) else {
        return Vec::new();
    };
    let max = player_stats(world, combat);
    state
        .player
        .inventory
        .keys()
        .filter_map(|id| {
            let restores = consume::consumable(world, id).ok()?;
            let (hp, mp) = consume::gain(now.hp, now.mp, max, restores);
            Some(Action {
                command: Command::Use(id.clone()),
                available: hp > 0 || mp > 0,
            })
        })
        .collect()
}

/// At an open market: its prices, then buying one unit of each good, where
/// affordable, and selling one of each good the player carries.
fn trade(world: &WorldSpec, state: &GameState) -> Vec<Action> {
    let Ok((economy, market)) = economy::market_here(world, state) else {
        return Vec::new();
    };
    let wallet = state.economy.as_ref().unwrap();
    let held = |good: &Id| state.player.inventory.get(good).copied();
    let mut actions = vec![Action {
        command: Command::Market,
        available: true,
    }];
    for good in &economy.goods {
        let quote = economy::quote(world, state, &good.item).unwrap();
        actions.push(Action {
            command: Command::Buy {
                good: good.item.clone(),
                quantity: 1,
            },
            // Carrying one more must fit the count too.
            available: wallet.currency >= quote.buy
                && held(&good.item).is_none_or(|n| n.checked_add(1).is_some()),
        });
    }
    for ware in &market.wares {
        // A counted ware must fit the carried count too.
        let fits = held(&ware.item).is_none_or(|n| n.checked_add(1).is_some());
        actions.push(Action {
            command: Command::Buy {
                good: ware.item.clone(),
                quantity: 1,
            },
            available: wallet.currency >= ware.price && fits,
        });
    }
    for good in &economy.goods {
        if held(&good.item).is_some() {
            let quote = economy::quote(world, state, &good.item).unwrap();
            actions.push(Action {
                command: Command::Sell {
                    good: good.item.clone(),
                    quantity: 1,
                },
                // Proceeds that would pass the currency bound cannot be taken.
                available: wallet
                    .currency
                    .checked_add(quote.sell)
                    .is_some_and(|total| total <= realmkit_spec::CURRENCY_BOUND),
            });
        }
    }
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
    actions.extend(consumables(world, state));
    if !encounter::group(world, encounter).is_some_and(|g| g.no_flee) {
        actions.push(available(Command::Flee));
    }
    actions.extend(panels);
    actions
}
