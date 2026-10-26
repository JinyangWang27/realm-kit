//! The optional real-time layer: fights on a fixed-step clock of their own,
//! between teams of fighters that can grow while the fight runs. It owns
//! when things happen (attack phases, guards, retreats, engine-controlled
//! fighters' policies), which inputs are legal at each step and when the
//! fight ends; what a landed attack does comes from the character core, as
//! for the turn-based layer. The clock moves only by `Intent::Advance`,
//! never by a host's frames or a wall clock, so the same state and intents
//! give the same fight.
//!
//! Named companions fight here with their own persistent vitals, which
//! stay in their `CompanionState` like the player's in `CombatState`, and
//! aim by their standing tactical order.
//!
//! Contact and arrivals are the host's to report: the engine has no
//! geometry. It accepts a contact only while the attacker's attack is active
//! and has not landed, on a fighter it may hit, and decides the damage,
//! guard and outcome itself. It cannot tell whether the blow really touched;
//! see `docs/combat.md`.

use super::*;
use crate::character::{profile, resources, strike, Pools};
use realmkit_spec::{Group, Realtime, Targeting, ADVANCE_BOUND};

mod clock;

/// The world's real-time rules; a world without them has no real-time fights.
pub(crate) fn rules(world: &WorldSpec) -> Result<&Realtime, EngineError> {
    world
        .combat()
        .and_then(|c| c.realtime.as_ref())
        .ok_or(EngineError::NoDuels)
}

/// A fighter's numbers, which cannot change while the fight lasts: nothing
/// that changes stats or gear can be done during one.
#[derive(Clone, Copy)]
pub(crate) struct Side {
    pub stats: Stats,
    pub pools: Pools,
    pub level: usize,
    /// The basic attack's channel and time.
    pub basic: (Channel, u32),
    /// Worn damage modifiers for physical and special hits on this fighter.
    pub guard: [(u64, u64); 2],
}

impl Side {
    fn guard(&self, channel: Channel) -> (u64, u64) {
        self.guard[match channel {
            Channel::Physical => 0,
            Channel::Special => 1,
        }]
    }
}

/// A fighter's numbers: the player's from the character core, anyone
/// else's from its combat profile. Only the player wears gear.
pub(crate) fn side(world: &WorldSpec, combat: &CombatState, character: &str) -> Side {
    let rules = world.combat().unwrap();
    if character == world.world.player {
        let stats = character::player_stats(world, combat);
        let (channel, time) = gear::basic(world, combat);
        return Side {
            stats,
            pools: resources::player_pools(world, &stats),
            level: combat.level,
            basic: (
                channel.unwrap_or(rules.player_basic_channel),
                time.unwrap_or(100),
            ),
            guard: [
                gear::modifier(world, combat, Channel::Physical),
                gear::modifier(world, combat, Channel::Special),
            ],
        };
    }
    let profile = profile(world, character);
    Side {
        stats: profile.stats,
        pools: resources::profile_pools(world, profile),
        level: profile.level,
        basic: (profile.basic_channel, 100),
        guard: [(1, 1); 2],
    }
}

/// Every fighter's numbers, in join order.
pub(crate) fn sides(
    world: &WorldSpec,
    combat: &CombatState,
    fight: &RealtimeEncounter,
) -> Vec<Side> {
    let side = |f: &Fighter| side(world, combat, &f.character);
    fight.fighters.iter().map(side).collect()
}

/// Steps a phase of `length` steps at speed 100 lasts for a fighter at
/// `speed` doing an action of `time`%: `max(1, ceil(length × time / speed))`.
pub(crate) fn phase_steps(length: u32, speed: u32, time: u32) -> u64 {
    (u64::from(length) * u64::from(time))
        .div_ceil(u64::from(speed.max(1)))
        .max(1)
}

/// The steps `phase` lasts for an attack with `skill` (or a basic attack).
pub(crate) fn attack_steps(
    realtime: &Realtime,
    side: &Side,
    skill: Option<&Skill>,
    phase: AttackPhase,
) -> u64 {
    let length = match phase {
        AttackPhase::Windup => realtime.windup,
        AttackPhase::Active => realtime.active,
        AttackPhase::Recovery => realtime.recovery,
    };
    let time = skill.map_or(side.basic.1, |s| s.time);
    phase_steps(length, side.stats.speed, time)
}

/// The group the fight started with, if any: its rules (yielding, flight,
/// flags) hold for everyone in the fight, reinforcements included.
pub(crate) fn group<'w>(world: &'w WorldSpec, fight: &RealtimeEncounter) -> Option<&'w Group> {
    world.group(
        profile(world, &fight.fighters[1].character)
            .group
            .as_deref()?,
    )
}

/// The fight beside the vitals held outside it, the player's and the
/// named companions', and every fighter's numbers.
struct Ring<'s> {
    fight: &'s mut RealtimeEncounter,
    player: &'s mut Vitals,
    companions: &'s mut BTreeMap<Id, CompanionState>,
    sides: Vec<Side>,
}

impl Ring<'_> {
    fn vitals(&self, i: usize) -> &Vitals {
        let fighter = &self.fight.fighters[i];
        match (&fighter.vitals, i) {
            (Some(vitals), _) => vitals,
            (None, 0) => self.player,
            (None, _) => &self.companions[&fighter.character].vitals,
        }
    }
    fn vitals_mut(&mut self, i: usize) -> &mut Vitals {
        let fighter = &mut self.fight.fighters[i];
        match (fighter.vitals.as_mut(), i) {
            (Some(vitals), _) => vitals,
            (None, 0) => self.player,
            (None, _) => {
                let companion = self.companions.get_mut(&fighter.character);
                &mut companion.unwrap().vitals
            }
        }
    }
    fn fighting(&self, i: usize) -> bool {
        self.vitals(i).hp > 0 && !self.fight.fighters[i].yielded
    }
    /// Whether anyone off the player's team still fights.
    fn contested(&self) -> bool {
        (1..self.fight.fighters.len()).any(|i| self.fight.fighters[i].team != 0 && self.fighting(i))
    }
    /// The fighters `i` may aim at: on another team and still fighting.
    fn foes(&self, i: usize) -> impl Iterator<Item = usize> + '_ {
        let team = self.fight.fighters[i].team;
        (0..self.fight.fighters.len())
            .filter(move |&t| self.fight.fighters[t].team != team && self.fighting(t))
    }
    /// The standing order of fighter `i`, if it is a named companion.
    fn order(&self, i: usize) -> Option<&TacticalOrder> {
        let fighter = &self.fight.fighters[i];
        match (&fighter.vitals, i) {
            (None, 1..) => Some(&self.companions[&fighter.character].order),
            _ => None,
        }
    }
}

fn ring<'s>(world: &WorldSpec, state: &'s mut GameState) -> Result<Ring<'s>, EngineError> {
    let GameState {
        combat, companions, ..
    } = state;
    let combat = combat.as_mut().ok_or(EngineError::NotFighting)?;
    let Stance::Realtime(fight) = &combat.stance else {
        return Err(EngineError::NotFighting);
    };
    let sides = sides(world, combat, fight);
    let Stance::Realtime(fight) = &mut combat.stance else {
        unreachable!("checked above");
    };
    Ok(Ring {
        fight,
        player: &mut combat.vitals,
        companions,
        sides,
    })
}

/// A character entering the fight now, as the engine controls it: whole,
/// with vitals that last the fight, or with `held` (`None`) for a named
/// companion, whose vitals stay its own.
fn newcomer(world: &WorldSpec, character: Id, team: u32, joined: u64, held: bool) -> Fighter {
    let profile = profile(world, &character);
    let pools = resources::profile_pools(world, profile);
    Fighter {
        character,
        team,
        control: Control::Policy,
        joined,
        vitals: (!held).then(|| resources::full(&profile.stats, pools)),
        mp_remainder: 0,
        action: RealtimeAction::Ready,
        yielded: false,
    }
}

/// Starts a real-time fight with `id` and whoever engaging it would bring
/// in, all on team 1. The player brings their own vitals; Rage that belongs
/// to one fight starts from 0.
pub(crate) fn start(
    world: &WorldSpec,
    state: &mut GameState,
    id: Id,
    events: &mut Vec<Event>,
) -> Result<(), EngineError> {
    let fighter = rules::character_here(world, state, &id).and_then(|c| c.combat.as_ref());
    let (Some(fighter), Some(combat)) = (fighter, state.combat.as_ref()) else {
        return Err(EngineError::NotHere(id));
    };
    if combat.defeated.contains(&id) {
        return Err(EngineError::AlreadyDefeated(id));
    }
    let Stance::Exploring = combat.stance else {
        return Err(EngineError::InEncounter);
    };
    rules(world)?;
    if fighter.ally {
        return Err(EngineError::NotHostile(id));
    }
    let opponents = encounter::opponents_for(world, state, &id);
    let player = Fighter {
        character: world.world.player.clone(),
        team: 0,
        control: Control::Player,
        joined: 0,
        vitals: None,
        mp_remainder: 0,
        action: RealtimeAction::Ready,
        yielded: false,
    };
    let fighters = std::iter::once(player)
        .chain(
            opponents
                .iter()
                .map(|c| newcomer(world, c.clone(), 1, 0, false)),
        )
        .collect();
    let combat = state.combat.as_mut().unwrap();
    resources::after_fight(world, &mut combat.vitals);
    combat.stance = Stance::Realtime(RealtimeEncounter { step: 0, fighters });
    state.dialogue = None;
    events.push(Event::EncounterStarted { opponents });
    Ok(())
}

/// One intent at the fight's current step. Every check comes before any
/// change, so a refused intent changes nothing.
pub(crate) fn intent(
    world: &WorldSpec,
    state: &mut GameState,
    intent: Intent,
    events: &mut Vec<Event>,
) -> Result<(), EngineError> {
    let realtime = *rules(world)?;
    let known = state
        .combat
        .as_ref()
        .map(|c| techniques::player_skills(world, c))
        .unwrap_or_default();
    let mut ring = ring(world, state)?;
    let now = ring.fight.step;
    let free = matches!(
        ring.fight.fighters[0].action,
        RealtimeAction::Ready | RealtimeAction::Guard { .. }
    );
    let player = ring.fight.fighters[0].character.clone();
    match intent {
        Intent::Advance(steps) => {
            if !(1..=ADVANCE_BOUND).contains(&steps) {
                return Err(EngineError::InvalidSteps);
            }
            clock::advance(world, state, steps, events)
        }
        Intent::Attack { .. } | Intent::Skill { .. } => {
            let (skill, target) = match intent {
                Intent::Skill { skill: id, target } => (
                    Some(
                        known
                            .into_iter()
                            .find(|s| s.id == id)
                            .ok_or(EngineError::UnknownSkill(id))?,
                    ),
                    target,
                ),
                Intent::Attack { target } => (None, target),
                _ => unreachable!("matched above"),
            };
            if !free {
                return Err(EngineError::Committed);
            }
            let target = aim(&ring, target)?;
            if let Some(skill) = skill {
                let side = ring.sides[0];
                encounter::check_skill(ring.player, side.pools, side.level, skill)?;
            }
            begin(&realtime, &mut ring, 0, skill, target, events)
        }
        Intent::Guard => {
            if ring.fight.fighters[0].action != RealtimeAction::Ready {
                return Err(EngineError::Committed);
            }
            raise(&realtime, &mut ring, 0, events)
        }
        Intent::LowerGuard => {
            let RealtimeAction::Guard { .. } = ring.fight.fighters[0].action else {
                return Err(EngineError::NotGuarding);
            };
            ring.fight.fighters[0].action = RealtimeAction::Ready;
            events.push(Event::GuardLowered { character: player });
            Ok(())
        }
        Intent::Retreat => {
            if group(world, ring.fight).is_some_and(|g| g.no_flee) {
                return Err(EngineError::NoFlee);
            }
            if !free {
                return Err(EngineError::Committed);
            }
            let until = now
                .checked_add(u64::from(realtime.retreat))
                .ok_or(EngineError::NumericLimit)?;
            ring.fight.fighters[0].action = RealtimeAction::Retreat { until };
            events.push(Event::RetreatStarted);
            Ok(())
        }
        Intent::Contact { attacker, target } => {
            let i = ring
                .fight
                .index(&attacker)
                .ok_or_else(|| EngineError::NotHere(attacker.clone()))?;
            // A fighter that is out is `Ready`, so it has nothing that can land.
            let RealtimeAction::Attack {
                phase: AttackPhase::Active,
                connected: false,
                target: aimed,
                ..
            } = &ring.fight.fighters[i].action
            else {
                return Err(EngineError::NoContact(attacker));
            };
            let target = target.unwrap_or_else(|| aimed.clone());
            let t = ring
                .fight
                .index(&target)
                .filter(|&t| t != i && ring.fighting(t))
                .ok_or_else(|| EngineError::InvalidTarget(target.clone()))?;
            if ring.fight.fighters[t].team == ring.fight.fighters[i].team && !realtime.friendly_fire
            {
                return Err(EngineError::NotHostile(target));
            }
            land(world, &realtime, state, i, t, events)
        }
        Intent::Join { character, team } => join(world, state, character, team, events),
    }
}

/// Whom the player aims at: `target` if it is a foe still fighting, or
/// the only such foe when none is named.
fn aim(ring: &Ring, target: Option<Id>) -> Result<Id, EngineError> {
    let fighters = &ring.fight.fighters;
    let Some(target) = target else {
        let mut foes = ring.foes(0);
        return match (foes.next(), foes.next()) {
            (Some(t), None) => Ok(fighters[t].character.clone()),
            _ => Err(EngineError::NeedTarget),
        };
    };
    match ring.fight.index(&target) {
        Some(t) if t != 0 && ring.fighting(t) && fighters[t].team == 0 => {
            Err(EngineError::NotHostile(target))
        }
        Some(t) if ring.foes(0).any(|f| f == t) => Ok(target),
        _ => Err(EngineError::InvalidTarget(target)),
    }
}

/// The foe an engine-controlled fighter `i` aims its next attack at: by its
/// authored targeting, or, for a named companion, by its standing order
/// first. `None` when it has nobody it would strike now, which only a
/// companion holding position or fighting warily can have while foes stand.
fn pick(world: &WorldSpec, ring: &Ring, i: usize) -> Option<usize> {
    let character = &ring.fight.fighters[i].character;
    let targeting = profile(world, character).targeting;
    let by_targeting = |foes: Vec<usize>| match targeting {
        Targeting::First => foes.first().copied(),
        // `min_by_key` keeps the first of equals: the earliest to join.
        Targeting::Weakest => foes.into_iter().min_by_key(|&t| ring.vitals(t).hp),
    };
    let foes: Vec<usize> = ring.foes(i).collect();
    let Some(order) = ring.order(i) else {
        return by_targeting(foes);
    };
    // Foes whose attack, still to land or miss, is aimed at `who`.
    let threats = |who: &str| -> Vec<usize> {
        let aimed = |t: &usize| match &ring.fight.fighters[*t].action {
            RealtimeAction::Attack { target, phase, .. } => {
                target == who && *phase != AttackPhase::Recovery
            }
            _ => false,
        };
        foes.iter().copied().filter(aimed).collect()
    };
    let caution = world.character(character).unwrap().companion.as_ref();
    let max_hp = ring.sides[i].stats.hp;
    let wary = caution
        .and_then(|c| c.caution)
        .is_some_and(|share| ring.vitals(i).hp <= encounter::yield_threshold(max_hp, share));
    match order {
        _ if wary => by_targeting(threats(character)),
        TacticalOrder::Hold => by_targeting(threats(character)),
        TacticalOrder::Follow => by_targeting(foes),
        TacticalOrder::Defend { ally } => {
            by_targeting(threats(ally)).or_else(|| by_targeting(foes.clone()))
        }
        TacticalOrder::Attack { target } => foes
            .iter()
            .copied()
            .find(|&t| ring.fight.fighters[t].character == *target)
            .or_else(|| by_targeting(foes.clone())),
    }
}

/// An authored join: `character`, here, enters whole on `team` at the
/// current step. An ally joins only the player's team, anyone else an
/// opponent's team or the next unused number; nobody joins twice, and a
/// defeated opponent stays defeated. A named companion joins only from the
/// party, never downed, and brings its own vitals instead of whole ones.
fn join(
    world: &WorldSpec,
    state: &mut GameState,
    character: Id,
    team: u32,
    events: &mut Vec<Event>,
) -> Result<(), EngineError> {
    let combat = state.combat.as_ref().unwrap();
    let Stance::Realtime(fight) = &combat.stance else {
        unreachable!("only a running fight takes intents");
    };
    if fight.index(&character).is_some() {
        return Err(EngineError::AlreadyJoined(character));
    }
    let Some(profile) =
        rules::character_here(world, state, &character).and_then(|c| c.combat.as_ref())
    else {
        return Err(EngineError::NotHere(character));
    };
    if combat.defeated.contains(&character) {
        return Err(EngineError::AlreadyDefeated(character));
    }
    let held = world
        .character(&character)
        .is_some_and(|c| c.companion.is_some());
    if held {
        match state.companions.get(&character) {
            Some(c) if c.in_party() && c.downed() => {
                return Err(EngineError::Downed(character));
            }
            Some(c) if c.in_party() => {}
            _ => return Err(EngineError::NotInParty(character)),
        }
    }
    let unused = fight.fighters.iter().map(|f| f.team).max().unwrap() + 1;
    let allowed = match profile.ally {
        true => team == 0,
        false => (1..=unused).contains(&team),
    };
    if !allowed {
        return Err(EngineError::InvalidTeam { character, team });
    }
    let combat = state.combat.as_mut().unwrap();
    let Stance::Realtime(fight) = &mut combat.stance else {
        unreachable!("checked above");
    };
    let fighter = newcomer(world, character.clone(), team, fight.step, held);
    fight.fighters.push(fighter);
    events.push(Event::Joined { character, team });
    Ok(())
}

/// Begins fighter `i`'s attack on `target` now, paying an affordable skill's costs.
fn begin(
    realtime: &Realtime,
    ring: &mut Ring,
    i: usize,
    skill: Option<&Skill>,
    target: Id,
    events: &mut Vec<Event>,
) -> Result<(), EngineError> {
    let side = ring.sides[i];
    let character = ring.fight.fighters[i].character.clone();
    let until = ring
        .fight
        .step
        .checked_add(attack_steps(realtime, &side, skill, AttackPhase::Windup))
        .ok_or(EngineError::NumericLimit)?;
    if let Some(skill) = skill {
        resources::spend(ring.vitals_mut(i), &character, skill, events);
    }
    ring.fight.fighters[i].action = RealtimeAction::Attack {
        skill: skill.map(|s| s.id.clone()),
        target,
        phase: AttackPhase::Windup,
        until,
        connected: false,
    };
    events.push(Event::Swing {
        character,
        skill: skill.map(|s| s.id.clone()),
        phase: AttackPhase::Windup,
    });
    Ok(())
}

/// Fighter `i` raises its guard now; it stops hits `guard_raise` steps later.
fn raise(
    realtime: &Realtime,
    ring: &mut Ring,
    i: usize,
    events: &mut Vec<Event>,
) -> Result<(), EngineError> {
    let now = ring.fight.step;
    let ready_at = now
        .checked_add(u64::from(realtime.guard_raise))
        .ok_or(EngineError::NumericLimit)?;
    ring.fight.fighters[i].action = RealtimeAction::Guard { ready_at };
    let character = ring.fight.fighters[i].character.clone();
    events.push(if ready_at == now {
        Event::GuardUp { character }
    } else {
        Event::GuardRaising { character }
    });
    Ok(())
}

/// Fighter `i`'s active attack lands on fighter `t`. The character core
/// decides the damage, with no random crit: a real-time hit's force depends
/// only on the state. A raised guard lets `guard_percent` through. Rage for
/// acting is credited only to an attack that lands, so swinging at the air
/// earns nothing. A fighter knocked out of the fight stops whatever it was
/// doing; the fight ends when nobody off the player's team still fights.
fn land(
    world: &WorldSpec,
    realtime: &Realtime,
    state: &mut GameState,
    i: usize,
    t: usize,
    events: &mut Vec<Event>,
) -> Result<(), EngineError> {
    let rules = world.combat().unwrap();
    let mut ring = ring(world, state)?;
    let now = ring.fight.step;
    let RealtimeAction::Attack { skill, .. } = &ring.fight.fighters[i].action else {
        unreachable!("only an active attack lands");
    };
    let skill = skill.as_ref().map(|id| world.skill(id).unwrap());
    let (attacker, defender) = (ring.sides[i], ring.sides[t]);
    let attack = strike::Attack::of(world, skill, attacker.basic.0, None);
    let modifier = defender.guard(attack.channel);
    let (mut dealt, _) = strike::damage_of(
        &attacker.stats,
        &defender.stats,
        attack,
        modifier,
        &mut None,
    )?;
    let target = ring.fight.fighters[t].character.clone();
    let guard = &ring.fight.fighters[t].action;
    if matches!(guard, RealtimeAction::Guard { ready_at } if *ready_at <= now) {
        let through = (u64::from(dealt) * u64::from(realtime.guard_percent) / 100) as u32;
        events.push(Event::Blocked {
            character: target.clone(),
            prevented: dealt - through,
        });
        dealt = through;
    }
    let source = ring.fight.fighters[i].character.clone();
    // A stray blow on a teammate earns neither Rage for acting nor training.
    let hostile = ring.fight.fighters[i].team != ring.fight.fighters[t].team;
    if hostile {
        resources::on_action(world, ring.vitals_mut(i), attacker.pools);
    }
    if let RealtimeAction::Attack { connected, .. } = &mut ring.fight.fighters[i].action {
        *connected = true;
    }
    // In a yielding group nobody dies: a hit stops at 1 HP.
    let yield_share = group(world, ring.fight).and_then(|g| g.yield_share);
    let floor = u32::from(yield_share.is_some());
    let v = ring.vitals_mut(t);
    let taken = strike::take(v, dealt, floor);
    resources::on_damage(world, v, defender.pools, dealt, defender.stats.hp);
    let hp = v.hp;
    let skill_id = skill.map(|s| s.id.clone());
    let attack_variant = (now % rules.narrative.attack.len() as u64) as usize;
    events.push(match (i, t) {
        (0, _) => Event::DamageDealt {
            target: target.clone(),
            amount: taken,
            variant: attack_variant,
            skill: skill_id,
            critical: false,
        },
        (_, 0) => Event::DamageReceived {
            source,
            amount: taken,
            variant: (now % rules.narrative.hurt.len() as u64) as usize,
            skill: skill_id,
            critical: false,
        },
        _ => Event::Struck {
            attacker: source,
            target: target.clone(),
            amount: taken,
            variant: attack_variant,
            skill: skill_id,
        },
    });
    if hp == 0 {
        events.push(match (t, ring.fight.fighters[t].team) {
            (0, _) => Event::PlayerDied,
            (_, 0) => Event::AllyDown {
                character: target.clone(),
            },
            _ => Event::EnemyDefeated {
                monster: target.clone(),
            },
        });
    }
    if yield_share.is_some_and(|share| hp <= encounter::yield_threshold(defender.stats.hp, share)) {
        ring.fight.fighters[t].yielded = true;
        events.push(Event::Yielded {
            character: target.clone(),
        });
    }
    // The player's own action stays as it was, dead or not, as it always has.
    if t != 0 && !ring.fighting(t) {
        ring.fight.fighters[t].action = RealtimeAction::Ready;
    }
    let (contested, player_yielded) = (ring.contested(), ring.fight.fighters[0].yielded);
    // A technique that lands trains, less so against much weaker opponents.
    let combat = state.combat.as_ref().unwrap();
    if let Some(technique) = skill
        .filter(|_| i == 0 && hostile)
        .and_then(|s| techniques::of_skill(world, combat, &s.id))
    {
        let per_use = u64::from(rules.technique_xp_per_use);
        let gain = xp_for_defeat(per_use, attacker.level, profile(world, &target).level);
        techniques::add_xp(world, state, technique, gain, events)?;
    }
    if !contested {
        end(world, state, Outcome::Victory, events)
    } else if player_yielded {
        end(world, state, Outcome::Yielded, events)
    } else {
        Ok(())
    }
}

/// Ends the fight and settles its outcome, once. A victory rewards every
/// opponent that fell, in join order, whoever struck it down; allies and
/// the opponents that yielded give nothing. Named companions keep the
/// vitals the fight left them, losing only Rage that belongs to one fight,
/// and orders to attack someone in it lapse to following. The player's
/// death leaves the fight open instead, as it does an encounter.
fn end(
    world: &WorldSpec,
    state: &mut GameState,
    outcome: Outcome,
    events: &mut Vec<Event>,
) -> Result<(), EngineError> {
    let Some(Stance::Realtime(fight)) = state.combat.as_ref().map(|c| &c.stance) else {
        unreachable!("only a running fight ends");
    };
    let group = group(world, fight);
    let fallen = fight
        .fighters
        .iter()
        .filter(|f| f.team != 0 && f.vitals.is_some_and(|v| v.hp == 0))
        .map(|f| f.character.clone())
        .collect();
    let fought: Vec<Id> = fight.fighters[1..]
        .iter()
        .filter(|f| f.vitals.is_none())
        .map(|f| f.character.clone())
        .collect();
    for id in &fought {
        resources::after_fight(world, &mut state.companions.get_mut(id).unwrap().vitals);
    }
    for companion in state.companions.values_mut() {
        if let TacticalOrder::Attack { .. } = companion.order {
            companion.order = TacticalOrder::Follow;
        }
    }
    settle::settle(world, state, outcome, group, fallen, events)
}
