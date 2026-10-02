//! Mass battles between the player's side and an authored army, fought round
//! by round. Every rule here mirrors `scripts/combat_sim/battle.py`; tests
//! hold the two to the same numbers.

use super::*;
use realmkit_spec::TroopClass;

/// One fighting unit as a round sees it: a stack, or the player as a stack
/// of one, with the stats it fights and is hit with.
struct Unit<'w> {
    stats: Stats,
    count: u64,
    /// Troops' class; the player has none.
    class: Option<&'w TroopClass>,
    channel: Channel,
    /// HP for strength: a soldier's full HP, the player's current HP.
    hp: u64,
}

impl Unit<'_> {
    fn ranged(&self) -> bool {
        self.class.is_some_and(|c| c.ranged)
    }
    fn mounted(&self) -> bool {
        self.class.is_some_and(|c| c.mounted)
    }
    fn attack(&self) -> u64 {
        u64::from(match self.channel {
            Channel::Physical => self.stats.patk,
            Channel::Special => self.stats.satk,
        })
    }
}

fn troop<'w>(world: &'w WorldSpec, stack: &BattleStack) -> Unit<'w> {
    let troops = world.troops().unwrap();
    let line = troops.line(&stack.line).unwrap();
    let stats = line.levels[stack.level - 1].stats;
    Unit {
        stats,
        count: stack.count,
        class: troops.class(&line.class),
        channel: line.channel,
        hp: u64::from(stats.hp),
    }
}

/// Each side's units: on side 0 the player comes first.
fn units<'w>(
    world: &'w WorldSpec,
    combat: &CombatState,
    battle: &BattleState,
) -> [Vec<Unit<'w>>; 2] {
    let stats = rules::player_stats(world, combat);
    let channel = gear::basic(world, combat)
        .0
        .unwrap_or(world.combat().unwrap().player_basic_channel);
    let player = Unit {
        stats,
        count: u64::from(battle.hp > 0),
        class: None,
        channel,
        hp: u64::from(battle.hp),
    };
    let side = |s: usize| battle.sides[s].stacks.iter().map(|st| troop(world, st));
    [
        std::iter::once(player).chain(side(0)).collect(),
        side(1).collect(),
    ]
}

/// n attackers across targets in proportion to their counts: floors, then
/// the leftover one each to the largest remainders, ties in target order.
fn split(n: u64, counts: &[u64]) -> Vec<u64> {
    let total: u64 = counts.iter().sum();
    let mut shares: Vec<u64> = counts.iter().map(|c| n * c / total).collect();
    let left = n - shares.iter().sum::<u64>();
    let mut order: Vec<usize> = (0..counts.len()).collect();
    order.sort_by_key(|i| (std::cmp::Reverse(n * counts[*i] % total), *i));
    for i in order.into_iter().take(left as usize) {
        shares[i] += 1;
    }
    shares
}

/// (unit, fighting count, flanking) for a side under its order.
fn fighters(
    own: &[Unit],
    enemy: &[Unit],
    order: BattleOrder,
    frontage: u64,
) -> Vec<(usize, u64, bool)> {
    let flanking = order == BattleOrder::Flank && enemy.iter().any(|u| u.ranged() && u.count > 0);
    let mut room = frontage;
    let mut out = Vec::new();
    for (i, unit) in own.iter().enumerate().filter(|(_, u)| u.count > 0) {
        if unit.ranged() {
            out.push((i, unit.count, false));
        } else if flanking && unit.mounted() {
            out.push((i, unit.count, true));
        } else {
            let take = room.min(unit.count);
            room -= take;
            if take > 0 {
                out.push((i, take, false));
            }
        }
    }
    out
}

/// The units a side can be hit in: its melee, else its archers; a flank
/// goes for the archers.
fn exposed(units: &[Unit], flank: bool) -> Vec<usize> {
    let standing = |ranged: bool| -> Vec<usize> {
        (0..units.len())
            .filter(|i| units[*i].count > 0 && units[*i].ranged() == ranged)
            .collect()
    };
    if flank {
        return standing(true);
    }
    let melee = standing(false);
    if melee.is_empty() {
        standing(true)
    } else {
        melee
    }
}

fn morale_mod(world: &WorldSpec, side: &BattleSide) -> u128 {
    world.battle().unwrap().morale.map_or(100, |m| {
        u128::from(m.floor) + u128::from(100 - m.floor) * u128::from(side.morale) / 100
    })
}

fn roll(world: &WorldSpec, rng: &mut Option<RngState>) -> u128 {
    let [lo, hi] = world.battle().unwrap().roll;
    let stream = rng.as_mut().unwrap().battle.as_mut().unwrap();
    u128::from(lo) + u128::from(rng::below(stream, u64::from(hi - lo + 1)))
}

/// Side `s`'s damage on the other side, added into `dealt` by target unit.
#[allow(clippy::too_many_arguments)]
fn strike(
    world: &WorldSpec,
    units: &[Vec<Unit>; 2],
    battle: &BattleState,
    rng: &mut Option<RngState>,
    s: usize,
    order: BattleOrder,
    holding: bool,
    pursuit: bool,
    dealt: &mut BTreeMap<usize, u128>,
) -> Result<(), EngineError> {
    let rules = world.battle().unwrap();
    let share = world.combat().unwrap().cross_share;
    let (own, enemy) = (&units[s], &units[1 - s]);
    let roll = roll(world, rng);
    let morale = morale_mod(world, &battle.sides[s]);
    for (i, n, flank) in fighters(own, enemy, order, rules.frontage) {
        let attacker = &own[i];
        let targets = exposed(enemy, flank);
        if targets.is_empty() {
            continue;
        }
        let counts: Vec<u64> = targets.iter().map(|t| enemy[*t].count).collect();
        for (t, k) in targets.into_iter().zip(split(n, &counts)) {
            if k == 0 {
                continue;
            }
            let target = &enemy[t];
            let hit = damage(&attacker.stats, &target.stats, attacker.channel, 100, share)?;
            let matchup = match (attacker.class, target.class) {
                (Some(a), Some(b)) => rules
                    .matchups
                    .get(&a.id)
                    .and_then(|m| m.get(&b.id))
                    .copied()
                    .unwrap_or(100),
                _ => 100,
            };
            let hold = if holding && !attacker.ranged() {
                rules.hold_percent
            } else {
                100
            };
            let flanked = if flank { rules.flank_percent } else { 100 };
            let chase = attacker
                .class
                .filter(|_| pursuit)
                .is_some_and(|c| rules.pursuit_class.as_ref() == Some(&c.id));
            let total = u128::from(k)
                * u128::from(hit)
                * u128::from(matchup)
                * morale
                * roll
                * u128::from(hold)
                * u128::from(flanked)
                * if chase { 200 } else { 100 }
                / 100u128.pow(6);
            *dealt.entry(t).or_default() += total;
        }
    }
    Ok(())
}

/// Losses from the damage dealt to side `s`; returns how many fell.
fn apply(
    world: &WorldSpec,
    battle: &mut BattleState,
    s: usize,
    dealt: &BTreeMap<usize, u128>,
) -> u64 {
    let mut losses = 0;
    for (t, amount) in dealt {
        // Side 0's unit 0 is the player.
        if s == 0 && *t == 0 {
            if battle.hp > 0 && *amount >= u128::from(battle.hp) {
                battle.hp = 0;
                losses += 1;
            } else if battle.hp > 0 {
                battle.hp -= *amount as u32;
            }
            continue;
        }
        let stack = &mut battle.sides[s].stacks[if s == 0 { t - 1 } else { *t }];
        let hp = u128::from(troop(world, stack).stats.hp);
        let pending = u128::from(stack.remainder) + amount;
        let lost = u64::try_from(pending / hp)
            .unwrap_or(u64::MAX)
            .min(stack.count);
        stack.remainder = (pending - u128::from(lost) * hp).min(u128::from(u64::MAX)) as u64;
        stack.count -= lost;
        if stack.count == 0 {
            stack.remainder = 0;
        }
        losses += lost;
    }
    losses
}

fn demoralize(world: &WorldSpec, side: &mut BattleSide, losses: u64) {
    let Some(morale) = world.battle().unwrap().morale else {
        return;
    };
    let num = u128::from(losses) * u128::from(morale.factor) + u128::from(side.morale_remainder);
    let drop = num / u128::from(side.start_size);
    side.morale -= side.morale.min(u32::try_from(drop).unwrap_or(u32::MAX));
    side.morale_remainder = (num % u128::from(side.start_size)) as u64;
}

fn broke(world: &WorldSpec, side: &BattleSide) -> bool {
    world
        .battle()
        .unwrap()
        .morale
        .is_some_and(|m| side.morale < m.rout)
}

fn standing(units: &[Unit]) -> bool {
    units.iter().any(|u| u.count > 0)
}

fn strength(world: &WorldSpec, units: &[Unit], side: &BattleSide) -> u64 {
    let total: u128 = units
        .iter()
        .map(|u| u128::from(u.count) * u128::from(u.hp) * u128::from(u.attack()))
        .sum();
    u64::try_from(total * morale_mod(world, side) / 10_000).unwrap_or(u64::MAX)
}

fn in_battle(combat: &Option<CombatState>) -> Result<(&CombatState, BattleState), EngineError> {
    let combat = combat.as_ref().ok_or(EngineError::NotInBattle)?;
    match &combat.stance {
        Stance::Battle(battle) => Ok((combat, battle.clone())),
        _ => Err(EngineError::NotInBattle),
    }
}

fn store(state: &mut GameState, battle: BattleState) {
    state.combat.as_mut().unwrap().stance = Stance::Battle(battle);
}

/// The winner cuts down a routed or retreating side, which deals nothing.
fn pursue(
    world: &WorldSpec,
    state: &mut GameState,
    battle: &mut BattleState,
    winner: usize,
    events: &mut Vec<Event>,
) -> Result<(), EngineError> {
    let units = units(world, state.combat.as_ref().unwrap(), battle);
    let mut dealt = BTreeMap::new();
    let order = BattleOrder::Charge;
    strike(
        world,
        &units,
        battle,
        &mut state.rng,
        winner,
        order,
        false,
        true,
        &mut dealt,
    )?;
    let losses = apply(world, battle, 1 - winner, &dealt);
    events.push(Event::Pursuit { by: winner, losses });
    Ok(())
}

/// One round under the player's order; the outcome once the battle ends.
fn round(
    world: &WorldSpec,
    state: &mut GameState,
    order: BattleOrder,
    events: &mut Vec<Event>,
) -> Result<Option<BattleOutcome>, EngineError> {
    let (combat, mut battle) = in_battle(&state.combat)?;
    let rules = world.battle().unwrap();
    let units = units(world, combat, &battle);
    if order == BattleOrder::Flank && !units[0].iter().any(|u| u.mounted() && u.count > 0) {
        return Err(EngineError::NoFlank);
    }
    if order == BattleOrder::Retreat {
        pursue(world, state, &mut battle, 1, events)?;
        store(state, battle);
        return Ok(Some(BattleOutcome::Defeat));
    }
    let holding = order == BattleOrder::Hold;
    let mut dealt = [BTreeMap::new(), BTreeMap::new()];
    strike(
        world,
        &units,
        &battle,
        &mut state.rng,
        0,
        order,
        holding,
        false,
        &mut dealt[1],
    )?;
    let charge = BattleOrder::Charge;
    strike(
        world,
        &units,
        &battle,
        &mut state.rng,
        1,
        charge,
        holding,
        false,
        &mut dealt[0],
    )?;
    let losses = [0, 1].map(|s| apply(world, &mut battle, s, &dealt[s]));
    for (side, lost) in battle.sides.iter_mut().zip(losses) {
        demoralize(world, side, lost);
    }
    battle.round += 1;
    let after = self::units(world, state.combat.as_ref().unwrap(), &battle);
    let strengths = [0, 1].map(|s| strength(world, &after[s], &battle.sides[s]));
    events.push(Event::BattleRound {
        round: battle.round,
        strengths,
        losses,
        morale: [battle.sides[0].morale, battle.sides[1].morale],
    });
    let gone = [0, 1].map(|s| broke(world, &battle.sides[s]) || !standing(&after[s]));
    let outcome = match gone {
        [true, true] => Some(BattleOutcome::Draw),
        [false, true] | [true, false] => {
            let winner = usize::from(gone[0]);
            pursue(world, state, &mut battle, winner, events)?;
            Some(if winner == 0 {
                BattleOutcome::Victory
            } else {
                BattleOutcome::Defeat
            })
        }
        [false, false] if battle.round == rules.rounds => Some(if strengths[0] > strengths[1] {
            BattleOutcome::Victory
        } else {
            BattleOutcome::Defeat
        }),
        [false, false] => None,
    };
    store(state, battle);
    Ok(outcome)
}

/// Starts a battle against a hostile army here, with every ally here whose
/// condition holds, and waits for the player's first order.
pub(super) fn engage(
    world: &WorldSpec,
    state: &mut GameState,
    id: Id,
    events: &mut Vec<Event>,
) -> Result<(), EngineError> {
    let character = rules::character_here(world, state, &id)
        .filter(|c| !rules::defeated(state, &c.id))
        .ok_or_else(|| EngineError::NotHere(id.clone()))?;
    let army = character.army.as_ref().unwrap();
    if army.joins.is_some() {
        return Err(EngineError::NotHostile(id));
    }
    let combat = state.combat.as_ref().unwrap();
    let Stance::Exploring(vitals) = combat.stance else {
        return Err(EngineError::InEncounter);
    };
    let allies = allies_here(world, state);
    let mut ours = roster_stacks(world, state);
    for ally in &allies {
        ours.extend(army_stacks(
            world.character(ally).unwrap().army.as_ref().unwrap(),
        ));
    }
    let theirs = army_stacks(army);
    let side = |stacks: Vec<BattleStack>, extra: u64| BattleSide {
        start_size: extra + stacks.iter().map(|s| s.count).sum::<u64>(),
        stacks,
        morale: 100,
        morale_remainder: 0,
    };
    let battle = BattleState {
        army: id.clone(),
        allies: allies.clone(),
        round: 0,
        hp: vitals.hp,
        mp: vitals.mp,
        sides: [side(ours, 1), side(theirs, 0)],
    };
    store(state, battle);
    state.dialogue = None;
    events.push(Event::BattleStarted { army: id, allies });
    Ok(())
}

/// Allies present here whose condition to join holds, in presence order.
pub(crate) fn allies_here(world: &WorldSpec, state: &GameState) -> Vec<Id> {
    rules::present_here(world, state)
        .into_iter()
        .filter(|c| {
            c.army
                .as_ref()
                .and_then(|a| a.joins.as_ref())
                .is_some_and(|j| rules::holds(state, j))
        })
        .map(|c| c.id.clone())
        .collect()
}

/// The roster's healthy squads: lines as authored, levels from the highest.
pub(crate) fn roster_stacks(world: &WorldSpec, state: &GameState) -> Vec<BattleStack> {
    let Some(retinue) = &state.retinue else {
        return Vec::new();
    };
    let mut stacks = Vec::new();
    for line in &world.troops().unwrap().lines {
        let Some(levels) = retinue.roster.get(&line.id) else {
            continue;
        };
        for (level, squad) in levels.iter().rev().filter(|(_, s)| s.healthy > 0) {
            stacks.push(BattleStack {
                line: line.id.clone(),
                level: *level,
                count: squad.healthy,
                remainder: 0,
            });
        }
    }
    stacks
}

pub(crate) fn army_stacks(army: &realmkit_spec::Army) -> Vec<BattleStack> {
    army.troops
        .iter()
        .map(|s| BattleStack {
            line: s.line.clone(),
            level: s.level,
            count: s.count,
            remainder: 0,
        })
        .collect()
}

/// Fights one round under `order`, ending the battle if it is decided.
pub(super) fn command(
    world: &WorldSpec,
    state: &mut GameState,
    order: BattleOrder,
    events: &mut Vec<Event>,
) -> Result<(), EngineError> {
    if let Some(outcome) = round(world, state, order, events)? {
        finish(world, state, outcome, events)?;
    }
    Ok(())
}

/// Charges until the battle is decided.
pub(super) fn autoresolve(
    world: &WorldSpec,
    state: &mut GameState,
    events: &mut Vec<Event>,
) -> Result<(), EngineError> {
    loop {
        if let Some(outcome) = round(world, state, BattleOrder::Charge, events)? {
            return finish(world, state, outcome, events);
        }
    }
}

/// Applies the roster's losses, returns the player to exploring on at least
/// 1 HP, and on victory shares the XP, grants the loot and records the army.
fn finish(
    world: &WorldSpec,
    state: &mut GameState,
    outcome: BattleOutcome,
    events: &mut Vec<Event>,
) -> Result<(), EngineError> {
    let (_, battle) = in_battle(&state.combat)?;
    let rules = world.troops().unwrap();
    let before = roster_stacks(world, state);
    let (mut wounded, mut killed) = (Vec::new(), Vec::new());
    if let Some(retinue) = state.retinue.as_mut() {
        for (start, now) in before.iter().zip(&battle.sides[0].stacks) {
            let lost = start.count - now.count;
            if lost == 0 {
                continue;
            }
            let hurt = lost * u64::from(rules.wounded_percent) / 100;
            let squad = retinue
                .roster
                .get_mut(&start.line)
                .and_then(|l| l.get_mut(&start.level))
                .unwrap();
            // The killed take their shares; the wounded stay, out of the fight.
            rules::leave(squad, lost - hurt, 0);
            squad.healthy -= hurt;
            squad.wounded += hurt;
            if hurt > 0 {
                wounded.push((start.line.clone(), start.level, hurt));
            }
            if lost > hurt {
                killed.push((start.line.clone(), start.level, lost - hurt));
            }
        }
        rules::prune(retinue);
    }
    state.combat.as_mut().unwrap().stance = Stance::Exploring(Vitals {
        hp: battle.hp.max(1),
        mp: battle.mp,
    });
    events.push(Event::BattleEnded {
        outcome,
        wounded,
        killed,
    });
    if outcome != BattleOutcome::Victory {
        // The dead take their shares rounded down, so what they leave behind
        // can lift the survivors' share past the next level.
        rules::promote(world, state, events);
        return Ok(());
    }
    let army = world
        .character(&battle.army)
        .unwrap()
        .army
        .as_ref()
        .unwrap();
    let share = world.battle().unwrap().player_xp_percent;
    // Army XP is unbounded content: share it in u128, where it cannot overflow.
    let mine = (u128::from(army.xp) * u128::from(share) / 100) as u64;
    rules::grant_xp(world, state, mine, events)?;
    let rest = army.xp - mine;
    if let Some(retinue) = state.retinue.as_mut() {
        let total: u64 = retinue
            .roster
            .values()
            .flat_map(|l| l.values())
            .map(|s| s.healthy)
            .sum();
        // Only squads that fought: every healthy one did.
        for squad in retinue.roster.values_mut().flat_map(|l| l.values_mut()) {
            if total > 0 && squad.healthy > 0 {
                let gain = u128::from(rest) * u128::from(squad.healthy) / u128::from(total);
                squad.xp = squad.xp.saturating_add(gain as u64);
            }
        }
    }
    rules::promote(world, state, events);
    rules::grant_items(world, state, &army.loot, events)?;
    if !army.repeatable {
        let combat = state.combat.as_mut().unwrap();
        combat.defeated.insert(battle.army.clone());
    }
    Ok(())
}
