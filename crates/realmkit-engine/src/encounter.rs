//! Encounters on the paused initiative timeline. Every rule here mirrors
//! `scripts/combat_sim/encounter.py`; tests hold the two to the same numbers.

use super::*;
use realmkit_spec::{Group, Timeline};

/// Ticks until an actor's next turn after an action taking `time` percent of
/// a basic action: `max(1, ceil(action_cost × time / (100 × speed)))`, with
/// speed clamped to `[1, speed_cap]`.
pub(super) fn delay(timeline: &Timeline, speed: u32, time: u32) -> Result<u64, EngineError> {
    let speed = u128::from(speed.clamp(1, timeline.speed_cap.max(1)));
    let ticks = u128::from(timeline.action_cost) * u128::from(time);
    u64::try_from(ticks.div_ceil(100 * speed).max(1)).map_err(|_| EngineError::NumericLimit)
}

/// One basic action at baseline speed 100; MP regeneration is measured in it.
pub(super) fn baseline_turn(timeline: &Timeline) -> Result<u64, EngineError> {
    delay(timeline, 100, 100)
}

/// A participant's stats: the player's from the level table, anyone else's
/// from their combat profile.
pub(super) fn stats(world: &WorldSpec, level: usize, p: &Participant) -> Stats {
    match p.control {
        Control::Player => rules::player_stats(world, level),
        Control::Policy => profile(world, &p.character).stats,
    }
}

fn profile<'w>(world: &'w WorldSpec, id: &str) -> &'w realmkit_spec::CombatProfile {
    world.character(id).unwrap().combat.as_ref().unwrap()
}

fn basic_channel(world: &WorldSpec, p: &Participant) -> Channel {
    match p.control {
        Control::Player => world.combat().unwrap().player_basic_channel,
        Control::Policy => profile(world, &p.character).basic_channel,
    }
}

fn affordable(p: &Participant, skill: &Skill) -> bool {
    let pool = match skill.resource {
        Resource::Mp => p.mp,
        Resource::Rage => p.rage,
    };
    pool >= skill.cost
}

/// The player's command, or `Err` without any change when it cannot be used.
pub(super) fn check_skill(p: &Participant, level: usize, skill: &Skill) -> Result<(), EngineError> {
    if skill.level > level {
        return Err(EngineError::SkillLocked(skill.id.clone()));
    }
    if !affordable(p, skill) {
        return Err(match skill.resource {
            Resource::Mp => EngineError::NotEnoughMp(skill.id.clone()),
            Resource::Rage => EngineError::NotEnoughRage(skill.id.clone()),
        });
    }
    Ok(())
}

/// The strongest affordable skill, else the basic attack. Equal power prefers
/// the cheaper skill, then the later tier; the first listed wins a full tie.
fn choose<'w>(world: &'w WorldSpec, p: &Participant) -> Option<&'w Skill> {
    let profile = profile(world, &p.character);
    profile
        .skills
        .iter()
        .filter_map(|id| world.skill(id))
        .filter(|s| s.level <= profile.level && affordable(p, s))
        .rev()
        .max_by_key(|s| (s.power, std::cmp::Reverse(s.cost), s.level))
}

/// Starts an encounter with `id`, then lets opponents act until the player's turn.
pub(super) fn engage(
    world: &WorldSpec,
    state: &mut GameState,
    id: Id,
    events: &mut Vec<Event>,
) -> Result<(), EngineError> {
    let fighter = rules::character_here(world, state, &id).and_then(|c| c.combat.as_ref());
    let (Some(_), Some(rules), Some(combat)) = (fighter, world.combat(), state.combat.as_ref())
    else {
        return Err(EngineError::NotHere(id));
    };
    if combat.defeated.contains(&id) {
        return Err(EngineError::AlreadyDefeated(id));
    }
    let Stance::Exploring(vitals) = combat.stance else {
        return Err(EngineError::InEncounter);
    };
    let player = rules::player_stats(world, combat.level);
    let opponents: Vec<(Id, Stats)> = opponents_for(world, state, &id)
        .into_iter()
        .map(|c| (c.clone(), profile(world, &c).stats))
        .collect();
    let combat = state.combat.as_mut().unwrap();
    // Everyone's first action comes after one opening delay, so speed matters at once.
    let joiner = |character: Id, side, control, hp, mp, speed| -> Result<_, EngineError> {
        Ok(Participant {
            character,
            side,
            control,
            hp,
            mp,
            rage: 0,
            mp_remainder: 0,
            rage_remainder: 0,
            next_time: delay(&rules.timeline, speed, 100)?,
            yielded: false,
        })
    };
    let mut participants = vec![joiner(
        world.world.player.clone(),
        0,
        Control::Player,
        vitals.hp,
        vitals.mp,
        player.speed,
    )?];
    for (character, stats) in &opponents {
        participants.push(joiner(
            character.clone(),
            1,
            Control::Policy,
            stats.hp,
            stats.mp,
            stats.speed,
        )?);
    }
    combat.stance = Stance::Fighting(Encounter {
        now: 0,
        participants,
    });
    state.dialogue = None;
    events.push(Event::EncounterStarted {
        opponents: opponents.into_iter().map(|(id, _)| id).collect(),
    });
    advance(world, state, events, false)
}

/// Who engaging `id` brings in: `id` alone, or every member of its group
/// placed here, present under its conditions and not defeated, in the
/// location's authored order.
pub(super) fn opponents_for(world: &WorldSpec, state: &GameState, id: &str) -> Vec<Id> {
    let Some(group) = world
        .character(id)
        .and_then(|c| c.combat.as_ref()?.group.as_ref())
    else {
        return vec![id.into()];
    };
    let defeated = |c: &str| {
        state
            .combat
            .as_ref()
            .is_some_and(|s| s.defeated.contains(c))
    };
    world
        .location(&state.player.location)
        .unwrap()
        .characters
        .iter()
        .filter(|c| !defeated(c))
        .filter(|c| {
            rules::character_here(world, state, c)
                .and_then(|m| m.combat.as_ref())
                .is_some_and(|m| m.group.as_ref() == Some(group))
        })
        .cloned()
        .collect()
}

/// The group an encounter's opponents share, if any.
pub(super) fn group<'w>(world: &'w WorldSpec, encounter: &Encounter) -> Option<&'w Group> {
    let first = encounter.participants.get(1)?;
    world.group(profile(world, &first.character).group.as_deref()?)
}

/// Declares a flight: the player's turn is spent, opponents act until the
/// player's next turn, and the escape happens then if the player lives.
pub(super) fn flee(
    world: &WorldSpec,
    state: &mut GameState,
    events: &mut Vec<Event>,
) -> Result<(), EngineError> {
    let (encounter, level) = fighting(state)?;
    if group(world, encounter).is_some_and(|g| g.no_flee) {
        return Err(EngineError::NoFlee);
    }
    let speed = rules::player_stats(world, level).speed;
    let step = delay(&world.combat().unwrap().timeline, speed, 100)?;
    let player = &mut encounter.participants[0];
    player.next_time = player
        .next_time
        .checked_add(step)
        .ok_or(EngineError::NumericLimit)?;
    events.push(Event::FleeStarted);
    advance(world, state, events, true)
}

fn fighting(state: &mut GameState) -> Result<(&mut Encounter, usize), EngineError> {
    let combat = state.combat.as_mut().ok_or(EngineError::NotFighting)?;
    match &mut combat.stance {
        Stance::Fighting(encounter) => Ok((encounter, combat.level)),
        Stance::Exploring(_) => Err(EngineError::NotFighting),
    }
}

/// The player's attack or skill on `target`, then opponents act until the
/// player's next turn or the end.
pub(super) fn player_action(
    world: &WorldSpec,
    state: &mut GameState,
    target: Id,
    skill: Option<&Skill>,
    events: &mut Vec<Event>,
) -> Result<(), EngineError> {
    let turn = state.turn;
    let (encounter, level) = fighting(state)?;
    let index = encounter
        .participants
        .iter()
        .position(|p| p.character == target && p.side != 0 && p.fighting())
        .ok_or(EngineError::NotHere(target))?;
    // Advancing stops at the player's turn, already regenerated, so the
    // player acts now with the MP they see.
    if let Some(skill) = skill {
        check_skill(&encounter.participants[0], level, skill)?;
    }
    act(world, encounter, level, turn, 0, index, skill, events)?;
    advance(world, state, events, false)
}

/// Regenerates everyone for the time until `actor`'s turn, then moves there.
fn tick(
    world: &WorldSpec,
    encounter: &mut Encounter,
    level: usize,
    actor: usize,
) -> Result<(), EngineError> {
    let rules = world.combat().unwrap();
    let at = encounter.participants[actor].next_time;
    let elapsed = at - encounter.now;
    let per_point = 100 * u128::from(baseline_turn(&rules.timeline)?);
    for p in &mut encounter.participants {
        let max = stats(world, level, p).mp;
        // Time spent at full MP banks nothing, so the remainder drops at the cap.
        let progress = u128::from(p.mp_remainder)
            + u128::from(max) * u128::from(rules.resources.mp_regen_percent) * u128::from(elapsed);
        let gained = progress / per_point;
        if u128::from(p.mp) + gained >= u128::from(max) {
            (p.mp, p.mp_remainder) = (max, 0);
        } else {
            p.mp += gained as u32;
            p.mp_remainder = (progress % per_point) as u64;
        }
    }
    encounter.now = at;
    Ok(())
}

/// Resolves one action. Rage from acting is credited after the action, so an
/// actor one point short cannot spend what its own action earns; rage from
/// being hit follows the damage dealt, before capping at the target's HP.
#[allow(clippy::too_many_arguments)]
fn act(
    world: &WorldSpec,
    encounter: &mut Encounter,
    level: usize,
    turn: u64,
    actor: usize,
    target: usize,
    skill: Option<&Skill>,
    events: &mut Vec<Event>,
) -> Result<(), EngineError> {
    let rules = world.combat().unwrap();
    let (a, t) = (
        &encounter.participants[actor],
        &encounter.participants[target],
    );
    let (attacker, defender) = (stats(world, level, a), stats(world, level, t));
    let (channel, power, share) = match skill {
        Some(s) => (
            s.channel,
            s.power,
            s.cross_share.unwrap_or(rules.cross_share),
        ),
        None => (basic_channel(world, a), BASIC_POWER, rules.cross_share),
    };
    let dealt = damage(&attacker, &defender, channel, power, share)?;
    // In a yielding group nobody dies: a hit stops at 1 HP.
    let yield_share = group(world, encounter).and_then(|g| g.yield_share);
    let time = skill.map_or(100, |s| s.time);
    let next = delay(&rules.timeline, attacker.speed, time)?;
    let a = &mut encounter.participants[actor];
    if let Some(skill) = skill.filter(|s| s.cost > 0) {
        match skill.resource {
            Resource::Mp => a.mp -= skill.cost,
            Resource::Rage => a.rage -= skill.cost,
        }
        events.push(Event::ResourceSpent {
            character: a.character.clone(),
            resource: skill.resource,
            amount: skill.cost,
        });
    }
    // Rage beyond u32 buys nothing more, so it saturates rather than failing.
    a.rage = a.rage.saturating_add(rules.resources.rage_per_action);
    a.next_time = a
        .next_time
        .checked_add(next)
        .ok_or(EngineError::NumericLimit)?;
    let source = a.character.clone();
    let t = &mut encounter.participants[target];
    let floor = u32::from(yield_share.is_some());
    let taken = dealt.min(t.hp - floor);
    t.hp -= taken;
    let progress = u128::from(t.rage_remainder)
        + u128::from(rules.resources.rage_per_max_hp) * u128::from(dealt);
    let max_hp = u128::from(defender.hp);
    t.rage = u32::try_from(u128::from(t.rage) + progress / max_hp).unwrap_or(u32::MAX);
    t.rage_remainder = (progress % max_hp) as u64;
    let skill = skill.map(|s| s.id.clone());
    let threshold =
        yield_share.map(|share| (u64::from(defender.hp) * u64::from(share) / 100).max(1) as u32);
    let yields = threshold.is_some_and(|limit| t.hp <= limit);
    if actor == 0 {
        events.push(Event::DamageDealt {
            target: t.character.clone(),
            amount: taken,
            variant: (turn % rules.narrative.attack.len() as u64) as usize,
            skill,
        });
        if t.hp == 0 {
            events.push(Event::EnemyDefeated {
                monster: t.character.clone(),
            });
        }
        if yields {
            t.yielded = true;
            events.push(Event::Yielded {
                character: t.character.clone(),
            });
        }
    } else {
        events.push(Event::DamageReceived {
            source,
            amount: taken,
            variant: (turn % rules.narrative.hurt.len() as u64) as usize,
            skill,
        });
        if t.hp == 0 && t.control == Control::Player {
            events.push(Event::PlayerDied);
        }
        if yields {
            t.yielded = true;
            events.push(Event::Yielded {
                character: t.character.clone(),
            });
        }
    }
    Ok(())
}

/// The living participant who acts next: `(next_time, side, position)`.
fn next_actor(encounter: &Encounter) -> Option<usize> {
    (0..encounter.participants.len())
        .filter(|&i| encounter.participants[i].fighting())
        .min_by_key(|&i| {
            let p = &encounter.participants[i];
            (p.next_time, p.side, i)
        })
}

/// Resolves policy actions until the player's turn, the player's death or
/// yield, or victory. A declared flight escapes at the player's turn.
fn advance(
    world: &WorldSpec,
    state: &mut GameState,
    events: &mut Vec<Event>,
    fleeing: bool,
) -> Result<(), EngineError> {
    let turn = state.turn;
    loop {
        let (encounter, level) = fighting(state)?;
        let player = &encounter.participants[0];
        if player.hp == 0 {
            return Ok(());
        }
        if player.yielded {
            return end(world, state, Outcome::Yielded, events);
        }
        if !encounter.participants[1..]
            .iter()
            .any(Participant::fighting)
        {
            return end(world, state, Outcome::Victory, events);
        }
        let actor = next_actor(encounter).unwrap();
        // Everyone regenerates up to the next turn before it is taken; the
        // player's turn then waits for a command.
        tick(world, encounter, level, actor)?;
        if encounter.participants[actor].control == Control::Player {
            return if fleeing {
                end(world, state, Outcome::Fled, events)
            } else {
                Ok(())
            };
        }
        let side = encounter.participants[actor].side;
        let target = (0..encounter.participants.len())
            .find(|&i| {
                let p = &encounter.participants[i];
                p.side != side && p.fighting()
            })
            .unwrap();
        let skill = choose(world, &encounter.participants[actor]);
        act(world, encounter, level, turn, actor, target, skill, events)?;
    }
}

/// The player's vitals return to exploring. A victory then grants each
/// opponent that died its loot and XP (scaled by level difference), records it
/// unless its group is repeatable, advances defeat objectives and sets the
/// group's victory flags; the player's yield sets its defeat flags; a flight
/// grants and records nothing.
fn end(
    world: &WorldSpec,
    state: &mut GameState,
    outcome: Outcome,
    events: &mut Vec<Event>,
) -> Result<(), EngineError> {
    let combat = state.combat.as_mut().unwrap();
    let Stance::Fighting(encounter) = &combat.stance else {
        unreachable!("only a running encounter ends");
    };
    let group = group(world, encounter);
    let player = &encounter.participants[0];
    let vitals = Vitals {
        hp: player.hp,
        mp: player.mp,
    };
    let fallen: Vec<Id> = encounter.participants[1..]
        .iter()
        .filter(|p| p.hp == 0)
        .map(|p| p.character.clone())
        .collect();
    combat.stance = Stance::Exploring(vitals);
    events.push(Event::EncounterEnded { outcome });
    let flags = match outcome {
        Outcome::Victory => group.map_or(&[][..], |g| &g.victory_flags[..]),
        Outcome::Yielded => group.map_or(&[][..], |g| &g.defeat_flags[..]),
        Outcome::Fled => return Ok(()),
    };
    if outcome == Outcome::Victory {
        let repeatable = group.is_some_and(|g| g.repeatable);
        // Every reward scales from the level the player fought at, so a
        // level-up from one opponent does not change the next one's XP.
        let level = state.combat.as_ref().unwrap().level;
        for id in fallen {
            let profile = profile(world, &id);
            let combat = state.combat.as_mut().unwrap();
            if !repeatable {
                combat.defeated.insert(id.clone());
            }
            let xp = xp_for_defeat(profile.xp, level, profile.level);
            rules::grant_items(state, &profile.loot, events)?;
            rules::grant_xp(world, state, xp, events)?;
            let defeat = QuestObjective::Defeat { character: id };
            for q in &world.quests {
                if q.objective == defeat && state.quests[&q.id] == QuestStatus::Active {
                    rules::progress(state, &q.id, events);
                }
            }
        }
    }
    for flag in flags {
        rules::set_flag(world, state, flag, events);
    }
    Ok(())
}

/// The next `n` actors if every action took a basic action's time.
pub(super) fn turn_order(world: &WorldSpec, state: &GameState, n: usize) -> Vec<Id> {
    let Some(combat) = &state.combat else {
        return Vec::new();
    };
    let Stance::Fighting(encounter) = &combat.stance else {
        return Vec::new();
    };
    let timeline = world.combat().unwrap().timeline;
    let mut projected = encounter.clone();
    let mut order = Vec::new();
    while order.len() < n {
        let Some(i) = next_actor(&projected) else {
            break;
        };
        let p = &projected.participants[i];
        let Ok(step) = delay(&timeline, stats(world, combat.level, p).speed, 100) else {
            break;
        };
        order.push(p.character.clone());
        projected.participants[i].next_time = p.next_time.saturating_add(step);
    }
    order
}
