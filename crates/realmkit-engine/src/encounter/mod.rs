//! Encounters on the paused initiative timeline. Every rule here mirrors
//! `scripts/combat_sim/encounter.py`; tests hold the two to the same numbers.

use super::*;
use realmkit_spec::{Group, Timeline};

mod action;
mod timeline;

pub(super) use action::check_skill;
use action::{act, choose};
pub(super) use timeline::{baseline_turn, delay, turn_order};
use timeline::{next_actor, tick};

/// The player during an encounter: level (for skill unlocks) and effective
/// stats, which cannot change until the encounter ends.
#[derive(Clone, Copy)]
pub(super) struct Me {
    pub level: usize,
    pub stats: Stats,
    /// A worn weapon's basic-attack channel and time, if any.
    pub basic: (Option<Channel>, Option<u32>),
    /// Worn damage modifiers for physical and special hits on the player.
    pub guard: [(u64, u64); 2],
}

impl Me {
    fn guard(&self, channel: Channel) -> (u64, u64) {
        self.guard[match channel {
            Channel::Physical => 0,
            Channel::Special => 1,
        }]
    }
}

/// A participant's stats: the player's effective stats, anyone else's
/// from their combat profile.
pub(super) fn stats(world: &WorldSpec, player: Stats, p: &Participant) -> Stats {
    match p.control {
        Control::Player => player,
        Control::Policy => profile(world, &p.character).stats,
    }
}

fn profile<'w>(world: &'w WorldSpec, id: &str) -> &'w realmkit_spec::CombatProfile {
    world.character(id).unwrap().combat.as_ref().unwrap()
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
    let player = rules::player_stats(world, combat);
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

/// A participant yields at or below `max(1, ⌊max HP × share / 100⌋)`.
pub(super) fn yield_threshold(max_hp: u32, share: u32) -> u32 {
    (u64::from(max_hp) * u64::from(share) / 100).max(1) as u32
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
    let (encounter, me) = fighting(world, &mut state.combat)?;
    if group(world, encounter).is_some_and(|g| g.no_flee) {
        return Err(EngineError::NoFlee);
    }
    let speed = me.stats.speed;
    let step = delay(&world.combat().unwrap().timeline, speed, 100)?;
    let player = &mut encounter.participants[0];
    player.next_time = player
        .next_time
        .checked_add(step)
        .ok_or(EngineError::NumericLimit)?;
    events.push(Event::FleeStarted);
    advance(world, state, events, true)
}

/// Spends the player's turn on a consumable: `hp`/`mp` (already capped) are
/// restored now, the turn takes one basic action's time, and opponents act
/// until the player's next turn. No rage or technique XP is earned.
pub(super) fn consume_turn(
    world: &WorldSpec,
    state: &mut GameState,
    hp: u32,
    mp: u32,
    events: &mut Vec<Event>,
) -> Result<(), EngineError> {
    let (encounter, me) = fighting(world, &mut state.combat)?;
    let step = delay(&world.combat().unwrap().timeline, me.stats.speed, 100)?;
    let player = &mut encounter.participants[0];
    player.hp += hp;
    player.mp += mp;
    player.next_time = player
        .next_time
        .checked_add(step)
        .ok_or(EngineError::NumericLimit)?;
    advance(world, state, events, false)
}

fn fighting<'s>(
    world: &WorldSpec,
    combat: &'s mut Option<CombatState>,
) -> Result<(&'s mut Encounter, Me), EngineError> {
    let combat = combat.as_mut().ok_or(EngineError::NotFighting)?;
    let me = Me {
        level: combat.level,
        stats: rules::player_stats(world, combat),
        basic: gear::basic(world, combat),
        guard: [
            gear::modifier(world, combat, Channel::Physical),
            gear::modifier(world, combat, Channel::Special),
        ],
    };
    match &mut combat.stance {
        Stance::Fighting(encounter) => Ok((encounter, me)),
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
    let (encounter, me) = fighting(world, &mut state.combat)?;
    let index = encounter
        .participants
        .iter()
        .position(|p| p.character == target && p.side != 0 && p.fighting())
        .ok_or_else(|| EngineError::NotHere(target.clone()))?;
    // Advancing stops at the player's turn, already regenerated, so the
    // player acts now with the MP they see.
    if let Some(skill) = skill {
        check_skill(&encounter.participants[0], me.level, skill)?;
    }
    act(
        world,
        encounter,
        me,
        turn,
        &mut state.rng,
        0,
        index,
        skill,
        events,
    )?;
    // Using a technique trains it, less so against much weaker opponents.
    let combat = state.combat.as_ref().unwrap();
    if let Some(technique) = skill.and_then(|s| techniques::of_skill(world, combat, &s.id)) {
        let per_use = u64::from(world.combat().unwrap().technique_xp_per_use);
        let gain = xp_for_defeat(per_use, me.level, profile(world, &target).level);
        techniques::add_xp(world, state, technique, gain, events)?;
    }
    advance(world, state, events, false)
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
        let (encounter, me) = fighting(world, &mut state.combat)?;
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
        tick(world, encounter, me, actor)?;
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
        act(
            world,
            encounter,
            me,
            turn,
            &mut state.rng,
            actor,
            target,
            skill,
            events,
        )?;
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
