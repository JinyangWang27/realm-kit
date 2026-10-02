//! The player's soldiers: recruiting, promotion by shared XP, upgrades into
//! branch lines, and the upkeep that pays, loses and mends them.

use super::*;
use realmkit_spec::{TroopLine, Troops};

fn troops(world: &WorldSpec) -> &Troops {
    world.troops().unwrap()
}

/// Healthy and wounded soldiers in every squad.
pub(crate) fn heads(retinue: &RetinueState) -> u64 {
    retinue
        .roster
        .values()
        .flat_map(|levels| levels.values())
        .map(Squad::heads)
        .sum()
}

/// Removes `count` soldiers who take their shares with them; a squad left
/// empty drops whatever XP remains.
fn leave(squad: &mut Squad, healthy: u64, wounded: u64) {
    let share = squad.share();
    squad.xp -= share * (healthy + wounded);
    squad.healthy -= healthy;
    squad.wounded -= wounded;
    if squad.heads() == 0 {
        squad.xp = 0;
    }
}

fn add(roster: &mut BTreeMap<Id, BTreeMap<usize, Squad>>, line: &str, level: usize, squad: Squad) {
    let into = roster
        .entry(line.into())
        .or_default()
        .entry(level)
        .or_insert(Squad {
            healthy: 0,
            wounded: 0,
            xp: 0,
        });
    into.healthy += squad.healthy;
    into.wounded += squad.wounded;
    into.xp = into.xp.saturating_add(squad.xp);
}

fn prune(retinue: &mut RetinueState) {
    for levels in retinue.roster.values_mut() {
        levels.retain(|_, squad| squad.heads() > 0);
    }
    retinue.roster.retain(|_, levels| !levels.is_empty());
}

/// Squads whose share covers the next level rise together, paying for it and
/// carrying the rest, line by line in authored order and level by level.
pub(crate) fn promote(world: &WorldSpec, state: &mut GameState, events: &mut Vec<Event>) {
    let Some(retinue) = state.retinue.as_mut() else {
        return;
    };
    for line in &troops(world).lines {
        for level in 1..line.levels.len() {
            let Some(squad) = retinue
                .roster
                .get(&line.id)
                .and_then(|l| l.get(&level))
                .copied()
            else {
                continue;
            };
            let next = line.levels[level].xp;
            if squad.share() < next {
                continue;
            }
            retinue.roster.get_mut(&line.id).unwrap().remove(&level);
            let risen = Squad {
                xp: squad.xp - squad.heads() * next,
                ..squad
            };
            add(&mut retinue.roster, &line.id, level + 1, risen);
            events.push(Event::Promoted {
                line: line.id.clone(),
                level: level + 1,
                count: squad.heads(),
            });
        }
    }
    prune(retinue);
}

pub(crate) fn recruit(
    world: &WorldSpec,
    state: &mut GameState,
    line: Id,
    quantity: u64,
    events: &mut Vec<Event>,
) -> Result<(), EngineError> {
    let here = world.location(&state.player.location).unwrap();
    let offer = here
        .recruits
        .as_ref()
        .and_then(|r| r.troops.iter().find(|o| o.line == line))
        .ok_or_else(|| EngineError::NotRecruitedHere(line.clone()))?;
    if quantity == 0 || quantity > realmkit_spec::ROSTER_BOUND {
        return Err(EngineError::InvalidQuantity);
    }
    let retinue = state.retinue.as_mut().unwrap();
    let full = heads(retinue) + quantity > troops(world).limit;
    let pool = retinue
        .pools
        .get_mut(&here.id)
        .and_then(|p| p.get_mut(&line))
        .unwrap();
    if *pool < quantity {
        return Err(EngineError::PoolEmpty(line));
    }
    if full {
        return Err(EngineError::RosterFull);
    }
    *pool -= quantity;
    add(
        &mut retinue.roster,
        &line,
        1,
        Squad {
            healthy: quantity,
            wounded: 0,
            xp: 0,
        },
    );
    let cost = offer
        .price
        .checked_mul(quantity)
        .ok_or(EngineError::NumericLimit)?;
    if cost > 0 {
        economy::pay(state, cost, &mut Vec::new())?;
    }
    state.dialogue = None;
    events.push(Event::Recruited {
        line,
        quantity,
        cost,
    });
    Ok(())
}

pub(crate) fn upgrade(
    world: &WorldSpec,
    state: &mut GameState,
    line: Id,
    to: Id,
    quantity: u64,
    events: &mut Vec<Event>,
) -> Result<(), EngineError> {
    if state.retinue.is_none() {
        return Err(EngineError::NoRetinue);
    }
    let from = troops(world)
        .line(&line)
        .ok_or_else(|| EngineError::NotEnoughTroops(line.clone()))?;
    let branch = from
        .upgrades
        .iter()
        .find(|u| u.to == to)
        .ok_or_else(|| EngineError::NoSuchUpgrade(line.clone(), to.clone()))?;
    let last = from.levels.len();
    let retinue = state.retinue.as_mut().unwrap();
    let squad = retinue
        .roster
        .get_mut(&line)
        .and_then(|l| l.get_mut(&last))
        .filter(|s| quantity > 0 && s.healthy >= quantity)
        .ok_or_else(|| EngineError::NotEnoughTroops(line.clone()))?;
    let carried = squad.share() * quantity;
    leave(squad, quantity, 0);
    add(
        &mut retinue.roster,
        &to,
        1,
        Squad {
            healthy: quantity,
            wounded: 0,
            xp: carried,
        },
    );
    prune(retinue);
    let cost = branch
        .cost
        .checked_mul(quantity)
        .ok_or(EngineError::NumericLimit)?;
    if cost > 0 {
        economy::pay(state, cost, &mut Vec::new())?;
    }
    state.dialogue = None;
    events.push(Event::Upgraded {
        line,
        to,
        quantity,
        cost,
    });
    promote(world, state, events);
    Ok(())
}

fn line<'w>(world: &'w WorldSpec, id: &str) -> &'w TroopLine {
    troops(world).line(id).unwrap()
}

/// Wages, all or nothing, then recovery. Unpaid wages cost each squad a
/// share of its soldiers, healthy ones first.
pub(crate) fn upkeep(world: &WorldSpec, state: &mut GameState, events: &mut Vec<Event>) {
    let rules = troops(world).upkeep.unwrap();
    let retinue = state.retinue.as_mut().unwrap();
    let wages: u64 = retinue
        .roster
        .iter()
        .flat_map(|(id, levels)| {
            levels
                .iter()
                .map(|(level, squad)| line(world, id).wage_at(*level) * squad.heads())
        })
        .sum();
    if wages > 0 {
        let wallet = state.economy.as_mut().unwrap();
        if wallet.currency >= wages {
            wallet.currency -= wages;
            events.push(Event::WagesPaid { amount: wages });
        } else {
            for (id, levels) in &mut retinue.roster {
                for (level, squad) in levels.iter_mut() {
                    let count = (squad.heads() * u64::from(rules.desert_percent) / 100)
                        .max(1)
                        .min(squad.heads());
                    let healthy = count.min(squad.healthy);
                    leave(squad, healthy, count - healthy);
                    events.push(Event::Deserted {
                        line: id.clone(),
                        level: *level,
                        count,
                    });
                }
            }
        }
    }
    mend(retinue, Some(rules.recover_percent), events);
    prune(retinue);
    // Deserters take their shares rounded down, which can promote the rest.
    promote(world, state, events);
}

/// Wounded soldiers heal: a share of each squad, rounded up, or all of them.
pub(crate) fn mend(retinue: &mut RetinueState, percent: Option<u32>, events: &mut Vec<Event>) {
    for (id, levels) in &mut retinue.roster {
        for (level, squad) in levels.iter_mut() {
            let count = match percent {
                Some(percent) => (squad.wounded * u64::from(percent)).div_ceil(100),
                None => squad.wounded,
            };
            if count == 0 {
                continue;
            }
            squad.wounded -= count;
            squad.healthy += count;
            events.push(Event::Recovered {
                line: id.clone(),
                level: *level,
                count,
            });
        }
    }
}

/// A recruiting pool refills towards its size.
pub(crate) fn refill(world: &WorldSpec, state: &mut GameState, location: &str) {
    let recruits = world.location(location).unwrap().recruits.as_ref().unwrap();
    let amount = recruits.refill.unwrap().amount;
    let pools = state
        .retinue
        .as_mut()
        .unwrap()
        .pools
        .get_mut(location)
        .unwrap();
    for offer in &recruits.troops {
        let pool = pools.get_mut(&offer.line).unwrap();
        *pool = pool.saturating_add(amount).min(offer.size);
    }
}
