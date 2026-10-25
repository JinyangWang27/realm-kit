//! The optional real-time layer: one-on-one duels on a fixed-step clock of
//! their own. It owns when things happen (attack phases, guards, retreats,
//! the opponent's policy) and which inputs are legal at each step; what a
//! landed attack does comes from the character core, as for the turn-based
//! layer. The clock moves only by `Intent::Advance`, never by a host's
//! frames or a wall clock, so the same state and intents give the same duel.
//!
//! Contact is the host's to report: the engine has no geometry. It accepts
//! a report only while the attacker's attack is active and has not landed,
//! and decides the damage, guard and outcome itself. It cannot tell whether
//! the blow really touched; see `docs/combat.md`.

use super::*;
use crate::character::{profile, resources, strike, Pools};
use realmkit_spec::{Group, Realtime, ADVANCE_BOUND};

mod clock;

/// The world's real-time rules; a world without them has no duels.
pub(crate) fn rules(world: &WorldSpec) -> Result<&Realtime, EngineError> {
    world
        .combat()
        .and_then(|c| c.realtime.as_ref())
        .ok_or(EngineError::NoDuels)
}

/// A fighter's numbers, which cannot change while the duel lasts: nothing
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

/// Both fighters' numbers: the player's from the character core, the
/// opponent's from its combat profile. Only the player wears gear.
pub(crate) fn sides(world: &WorldSpec, combat: &CombatState, opponent: &str) -> [Side; 2] {
    let rules = world.combat().unwrap();
    let stats = character::player_stats(world, combat);
    let (channel, time) = gear::basic(world, combat);
    let player = Side {
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
    let profile = profile(world, opponent);
    let opponent = Side {
        stats: profile.stats,
        pools: resources::profile_pools(world, profile),
        level: profile.level,
        basic: (profile.basic_channel, 100),
        guard: [(1, 1); 2],
    };
    [player, opponent]
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

/// The group the opponent fights in, if any.
pub(crate) fn group<'w>(world: &'w WorldSpec, duel: &Duel) -> Option<&'w Group> {
    world.group(
        profile(world, &duel.fighters[1].character)
            .group
            .as_deref()?,
    )
}

/// The duel beside the player's vitals, and both sides' numbers.
struct Ring<'s> {
    duel: &'s mut Duel,
    player: &'s mut Vitals,
    sides: [Side; 2],
}

impl Ring<'_> {
    fn vitals_mut(&mut self, i: usize) -> &mut Vitals {
        match self.duel.fighters[i].vitals.as_mut() {
            Some(vitals) => vitals,
            None => self.player,
        }
    }
    fn fighting(&self, i: usize) -> bool {
        self.duel.fighting(i, self.player)
    }
}

fn dueling<'s>(
    world: &WorldSpec,
    combat: &'s mut Option<CombatState>,
) -> Result<Ring<'s>, EngineError> {
    let combat = combat.as_mut().ok_or(EngineError::NotFighting)?;
    let Stance::Dueling(duel) = &combat.stance else {
        return Err(EngineError::NotFighting);
    };
    let sides = sides(world, combat, &duel.fighters[1].character);
    let Stance::Dueling(duel) = &mut combat.stance else {
        unreachable!("checked above");
    };
    Ok(Ring {
        duel,
        player: &mut combat.vitals,
        sides,
    })
}

/// Starts a duel with `id`, who must fight alone. The player brings their
/// own vitals; Rage that belongs to one fight starts from 0.
pub(crate) fn start(
    world: &WorldSpec,
    state: &mut GameState,
    id: Id,
    events: &mut Vec<Event>,
) -> Result<(), EngineError> {
    let fighter = rules::character_here(world, state, &id).and_then(|c| c.combat.as_ref());
    let (Some(_), Some(combat)) = (fighter, state.combat.as_ref()) else {
        return Err(EngineError::NotHere(id));
    };
    if combat.defeated.contains(&id) {
        return Err(EngineError::AlreadyDefeated(id));
    }
    let Stance::Exploring = combat.stance else {
        return Err(EngineError::InEncounter);
    };
    rules(world)?;
    if encounter::opponents_for(world, state, &id) != [id.clone()] {
        return Err(EngineError::NotOneOnOne(id));
    }
    let profile = profile(world, &id);
    let pools = resources::profile_pools(world, profile);
    let fighter = |character, vitals| Fighter {
        character,
        vitals,
        mp_remainder: 0,
        action: DuelAction::Ready,
        yielded: false,
    };
    let fighters = [
        fighter(world.world.player.clone(), None),
        fighter(id.clone(), Some(resources::full(&profile.stats, pools))),
    ];
    let combat = state.combat.as_mut().unwrap();
    resources::after_fight(world, &mut combat.vitals);
    combat.stance = Stance::Dueling(Duel { step: 0, fighters });
    state.dialogue = None;
    events.push(Event::EncounterStarted {
        opponents: vec![id],
    });
    Ok(())
}

/// One of the player's intents at the duel's current step. Every check
/// comes before any change, so a refused intent changes nothing.
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
    let mut ring = dueling(world, &mut state.combat)?;
    let now = ring.duel.step;
    let free = matches!(
        ring.duel.fighters[0].action,
        DuelAction::Ready | DuelAction::Guard { .. }
    );
    let player = ring.duel.fighters[0].character.clone();
    match intent {
        Intent::Advance(steps) => {
            if !(1..=ADVANCE_BOUND).contains(&steps) {
                return Err(EngineError::InvalidSteps);
            }
            clock::advance(world, state, steps, events)
        }
        Intent::Attack | Intent::Skill(_) => {
            let skill = match intent {
                Intent::Skill(id) => Some(
                    known
                        .into_iter()
                        .find(|s| s.id == id)
                        .ok_or(EngineError::UnknownSkill(id))?,
                ),
                _ => None,
            };
            if !free {
                return Err(EngineError::Committed);
            }
            if let Some(skill) = skill {
                let side = ring.sides[0];
                encounter::check_skill(ring.player, side.pools, side.level, skill)?;
            }
            begin(&realtime, &mut ring, 0, skill, events)
        }
        Intent::Guard => {
            if ring.duel.fighters[0].action != DuelAction::Ready {
                return Err(EngineError::Committed);
            }
            let ready_at = now
                .checked_add(u64::from(realtime.guard_raise))
                .ok_or(EngineError::NumericLimit)?;
            ring.duel.fighters[0].action = DuelAction::Guard { ready_at };
            events.push(if ready_at == now {
                Event::GuardUp { character: player }
            } else {
                Event::GuardRaising { character: player }
            });
            Ok(())
        }
        Intent::LowerGuard => {
            let DuelAction::Guard { .. } = ring.duel.fighters[0].action else {
                return Err(EngineError::NotGuarding);
            };
            ring.duel.fighters[0].action = DuelAction::Ready;
            events.push(Event::GuardLowered { character: player });
            Ok(())
        }
        Intent::Retreat => {
            if group(world, ring.duel).is_some_and(|g| g.no_flee) {
                return Err(EngineError::NoFlee);
            }
            if !free {
                return Err(EngineError::Committed);
            }
            let until = now
                .checked_add(u64::from(realtime.retreat))
                .ok_or(EngineError::NumericLimit)?;
            ring.duel.fighters[0].action = DuelAction::Retreat { until };
            events.push(Event::RetreatStarted);
            Ok(())
        }
        Intent::Contact(attacker) => {
            let i = (0..2)
                .find(|&i| ring.duel.fighters[i].character == attacker)
                .ok_or_else(|| EngineError::NotHere(attacker.clone()))?;
            let DuelAction::Attack {
                phase: AttackPhase::Active,
                connected: false,
                ..
            } = ring.duel.fighters[i].action
            else {
                return Err(EngineError::NoContact(attacker));
            };
            land(world, &realtime, state, i, events)
        }
    }
}

/// Begins fighter `i`'s attack now, paying an affordable skill's costs.
fn begin(
    realtime: &Realtime,
    ring: &mut Ring,
    i: usize,
    skill: Option<&Skill>,
    events: &mut Vec<Event>,
) -> Result<(), EngineError> {
    let side = ring.sides[i];
    let character = ring.duel.fighters[i].character.clone();
    let until = ring
        .duel
        .step
        .checked_add(attack_steps(realtime, &side, skill, AttackPhase::Windup))
        .ok_or(EngineError::NumericLimit)?;
    if let Some(skill) = skill {
        resources::spend(ring.vitals_mut(i), &character, skill, events);
    }
    ring.duel.fighters[i].action = DuelAction::Attack {
        skill: skill.map(|s| s.id.clone()),
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

/// Fighter `i`'s active attack lands on the other fighter. The character
/// core decides the damage, with no random crit: a real-time hit's force
/// depends only on the state. A raised guard lets `guard_percent` through.
/// Rage for acting is credited only to an attack that lands, so swinging at
/// the air earns nothing.
fn land(
    world: &WorldSpec,
    realtime: &Realtime,
    state: &mut GameState,
    i: usize,
    events: &mut Vec<Event>,
) -> Result<(), EngineError> {
    let rules = world.combat().unwrap();
    let mut ring = dueling(world, &mut state.combat)?;
    let t = 1 - i;
    let now = ring.duel.step;
    let DuelAction::Attack { skill, .. } = &ring.duel.fighters[i].action else {
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
    let target = ring.duel.fighters[t].character.clone();
    if matches!(ring.duel.fighters[t].action, DuelAction::Guard { ready_at } if ready_at <= now) {
        let through = (u64::from(dealt) * u64::from(realtime.guard_percent) / 100) as u32;
        events.push(Event::Blocked {
            character: target.clone(),
            prevented: dealt - through,
        });
        dealt = through;
    }
    let source = ring.duel.fighters[i].character.clone();
    resources::on_action(world, ring.vitals_mut(i), attacker.pools);
    if let DuelAction::Attack { connected, .. } = &mut ring.duel.fighters[i].action {
        *connected = true;
    }
    // In a yielding group nobody dies: a hit stops at 1 HP.
    let yield_share = group(world, ring.duel).and_then(|g| g.yield_share);
    let floor = u32::from(yield_share.is_some());
    let v = ring.vitals_mut(t);
    let taken = strike::take(v, dealt, floor);
    resources::on_damage(world, v, defender.pools, dealt, defender.stats.hp);
    let hp = v.hp;
    let skill_id = skill.map(|s| s.id.clone());
    if i == 0 {
        events.push(Event::DamageDealt {
            target: target.clone(),
            amount: taken,
            variant: (now % rules.narrative.attack.len() as u64) as usize,
            skill: skill_id,
            critical: false,
        });
        if hp == 0 {
            events.push(Event::EnemyDefeated {
                monster: target.clone(),
            });
        }
    } else {
        events.push(Event::DamageReceived {
            source,
            amount: taken,
            variant: (now % rules.narrative.hurt.len() as u64) as usize,
            skill: skill_id,
            critical: false,
        });
        if hp == 0 {
            events.push(Event::PlayerDied);
        }
    }
    if yield_share.is_some_and(|share| hp <= encounter::yield_threshold(defender.stats.hp, share)) {
        ring.duel.fighters[t].yielded = true;
        events.push(Event::Yielded { character: target });
    }
    let (opponent_out, player_yielded) = (!ring.fighting(1), ring.duel.fighters[0].yielded);
    let opponent = ring.duel.fighters[1].character.clone();
    // A technique that lands trains, less so against much weaker opponents.
    let combat = state.combat.as_ref().unwrap();
    if let Some(technique) = skill
        .filter(|_| i == 0)
        .and_then(|s| techniques::of_skill(world, combat, &s.id))
    {
        let per_use = u64::from(rules.technique_xp_per_use);
        let gain = xp_for_defeat(per_use, attacker.level, profile(world, &opponent).level);
        techniques::add_xp(world, state, technique, gain, events)?;
    }
    if opponent_out {
        end(world, state, Outcome::Victory, events)
    } else if player_yielded {
        end(world, state, Outcome::Yielded, events)
    } else {
        Ok(())
    }
}

/// Ends the duel and settles its outcome, once. The player's death leaves
/// it open instead, as it does an encounter.
fn end(
    world: &WorldSpec,
    state: &mut GameState,
    outcome: Outcome,
    events: &mut Vec<Event>,
) -> Result<(), EngineError> {
    let Some(Stance::Dueling(duel)) = state.combat.as_ref().map(|c| &c.stance) else {
        unreachable!("only a running duel ends");
    };
    let group = group(world, duel);
    let opponent = &duel.fighters[1];
    let fallen = opponent
        .vitals
        .filter(|v| v.hp == 0)
        .map(|_| opponent.character.clone())
        .into_iter()
        .collect();
    settle::settle(world, state, outcome, group, fallen, events)
}
