use super::*;

pub(super) fn conditions_met(state: &GameState, conditions: &[Condition]) -> bool {
    conditions.iter().all(|condition| match condition {
        Condition::Flag { flag } => state.flags.contains(flag),
        Condition::Quest { quest, status } => state.quests.get(quest) == Some(status),
    })
}

pub(super) fn player_vitals(state: &GameState) -> Option<Vitals> {
    Some(match &state.combat.as_ref()?.stance {
        Stance::Exploring(vitals) => *vitals,
        Stance::Fighting(encounter) => {
            let player = &encounter.participants[0];
            Vitals {
                hp: player.hp,
                mp: player.mp,
            }
        }
    })
}

pub(super) fn dead(state: &GameState) -> bool {
    player_vitals(state).is_some_and(|v| v.hp == 0)
}

fn defeated(state: &GameState, id: &str) -> bool {
    state
        .combat
        .as_ref()
        .is_some_and(|c| c.defeated.contains(id))
}

fn fighting(state: &GameState) -> Option<&Encounter> {
    match &state.combat.as_ref()?.stance {
        Stance::Fighting(encounter) => Some(encounter),
        Stance::Exploring(_) => None,
    }
}

/// Placed at the player's location and present under its conditions.
pub(super) fn character_here<'a>(
    world: &'a WorldSpec,
    state: &GameState,
    id: &str,
) -> Option<&'a Character> {
    let placed = world
        .location(&state.player.location)
        .unwrap()
        .characters
        .iter()
        .any(|c| c == id);
    world
        .character(id)
        .filter(|c| placed && conditions_met(state, &c.requires))
}

/// Can be talked to here; a defeated fighter is gone, like its listing.
pub(super) fn npc_here(world: &WorldSpec, state: &GameState, id: &str) -> bool {
    !defeated(state, id) && character_here(world, state, id).is_some_and(|c| c.dialogue.is_some())
}

pub(super) fn choices<'a>(
    world: &'a WorldSpec,
    state: &GameState,
    npc: &str,
    node: &str,
) -> Vec<&'a DialogueChoice> {
    world
        .dialogue(world.character(npc).unwrap().dialogue.as_ref().unwrap())
        .unwrap()
        .nodes
        .iter()
        .find(|n| n.id == node)
        .unwrap()
        .choices
        .iter()
        .filter(|c| conditions_met(state, &c.requires))
        .collect()
}

fn dialogue(world: &WorldSpec, state: &mut GameState, npc: Id, node: Id, events: &mut Vec<Event>) {
    let visible = choices(world, state, &npc, &node);
    state.dialogue = if visible.is_empty() {
        None
    } else {
        Some(DialogueState {
            npc: npc.clone(),
            node: node.clone(),
        })
    };
    events.push(Event::Dialogue {
        npc,
        node,
        choices: visible.iter().map(|c| c.text.clone()).collect(),
    });
    if state.dialogue.is_none() {
        events.push(Event::DialogueEnded);
    }
}

pub(super) fn grant_items(
    state: &mut GameState,
    stacks: &[ItemStack],
    events: &mut Vec<Event>,
) -> Result<(), EngineError> {
    for stack in stacks {
        let count = state
            .player
            .inventory
            .entry(stack.item.clone())
            .or_default();
        *count = count
            .checked_add(stack.quantity)
            .ok_or(EngineError::NumericLimit)?;
        events.push(Event::ItemReceived {
            item: stack.item.clone(),
            quantity: stack.quantity,
        });
    }
    Ok(())
}

/// XP and levels belong to combat; a world without it grants none. Rewards
/// arrive only while exploring, so level-ups restore the exploring vitals.
pub(super) fn grant_xp(
    world: &WorldSpec,
    state: &mut GameState,
    amount: u64,
    events: &mut Vec<Event>,
) -> Result<(), EngineError> {
    let (Some(combat), Some(rules)) = (state.combat.as_mut(), world.combat()) else {
        return Ok(());
    };
    combat.xp = combat
        .xp
        .checked_add(amount)
        .ok_or(EngineError::NumericLimit)?;
    events.push(Event::ExperienceGranted { amount });
    while let Some(level) = rules.levels.get(combat.level) {
        if combat.xp < level.xp {
            break;
        }
        // Levelling up restores HP and MP fully.
        combat.level += 1;
        combat.stance = Stance::Exploring(Vitals {
            hp: level.stats.hp,
            mp: level.stats.mp,
        });
        events.push(Event::LevelUp {
            level: combat.level,
        });
    }
    Ok(())
}

pub(super) fn progress(state: &mut GameState, quest: &Id, events: &mut Vec<Event>) {
    state.quests.insert(quest.clone(), QuestStatus::Ready);
    events.push(Event::QuestProgressed {
        quest: quest.clone(),
    });
}

pub(super) fn set_flag(
    world: &WorldSpec,
    state: &mut GameState,
    flag: &str,
    events: &mut Vec<Event>,
) {
    if !state.flags.insert(flag.into()) {
        return;
    }
    events.push(Event::StoryFlagSet { flag: flag.into() });
    for q in &world.quests {
        if matches!(&q.objective, QuestObjective::Flag { flag: f } if f == flag)
            && state.quests[&q.id] == QuestStatus::Active
        {
            progress(state, &q.id, events);
        }
    }
}

fn quest(
    world: &WorldSpec,
    state: &mut GameState,
    id: &str,
    complete: bool,
    events: &mut Vec<Event>,
) -> Result<(), EngineError> {
    let quest = world
        .quest(id)
        .ok_or_else(|| EngineError::UnknownQuest(id.into()))?;
    if !npc_here(world, state, &quest.giver) {
        return Err(EngineError::NotHere(quest.giver.clone()));
    }
    let required = if complete {
        QuestStatus::Ready
    } else {
        QuestStatus::Available
    };
    if state.quests[id] != required {
        return Err(EngineError::QuestState(id.into()));
    }
    if complete {
        state.quests.insert(id.into(), QuestStatus::Completed);
        events.push(Event::QuestCompleted { quest: id.into() });
        grant_items(state, &quest.reward_items, events)?;
        grant_xp(world, state, quest.reward_xp, events)?;
        for flag in &quest.completion_flags {
            set_flag(world, state, flag, events);
        }
    } else {
        state.quests.insert(id.into(), QuestStatus::Active);
        events.push(Event::QuestAccepted { quest: id.into() });
        // The world remembers earlier kills and flags, so accepting late
        // cannot strand this quest.
        let done = match &quest.objective {
            QuestObjective::Defeat { character } => defeated(state, character),
            QuestObjective::Flag { flag } => state.flags.contains(flag),
        };
        if done {
            progress(state, &quest.id, events);
        }
    }
    Ok(())
}

pub(super) fn actions(world: &WorldSpec, state: &GameState) -> Vec<Action> {
    let available = |command| Action {
        command,
        available: true,
    };
    let panels = [Command::Inventory, Command::Status, Command::Quests].map(available);
    // Death is not a locked door: offer only what can still be done.
    if dead(state) {
        return panels.into();
    }
    if let Some(encounter) = fighting(state) {
        let level = state.combat.as_ref().unwrap().level;
        let player = &encounter.participants[0];
        let skills: Vec<_> = world
            .combat()
            .unwrap()
            .player_skills
            .iter()
            .filter_map(|id| world.skill(id))
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
        return actions;
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
        available: conditions_met(state, &exit.requires),
    }));
    if state.combat.is_some() && location.safe {
        actions.push(available(Command::Rest));
    }
    actions.extend(panels);
    actions
}

pub(super) fn execute(
    world: &WorldSpec,
    state: &mut GameState,
    command: Command,
) -> Result<Vec<Event>, EngineError> {
    if dead(state)
        && !matches!(
            command,
            Command::Look | Command::Status | Command::Inventory | Command::Quests
        )
    {
        return Err(EngineError::PlayerDead);
    }
    let panel = matches!(
        command,
        Command::Look | Command::Status | Command::Inventory | Command::Quests
    );
    let combat_action = matches!(
        command,
        Command::Attack(_) | Command::UseSkill { .. } | Command::Flee
    );
    // During an encounter only combat actions and panels are possible.
    if fighting(state).is_some() && !panel && !combat_action {
        return Err(EngineError::InEncounter);
    }
    let mut events = Vec::new();
    match command {
        Command::Look => events.push(Event::LocationViewed {
            location: state.player.location.clone(),
        }),
        Command::Inventory => events.push(Event::InventoryViewed),
        Command::Status => events.push(Event::StatusViewed),
        Command::Quests => events.push(Event::QuestsViewed),
        Command::Move(direction) => {
            let location = world.location(&state.player.location).unwrap();
            let exit = location.exits.get(&direction).ok_or(EngineError::NoExit)?;
            if !conditions_met(state, &exit.requires) {
                return Err(EngineError::ExitLocked {
                    location: location.id.clone(),
                    direction,
                });
            }
            let from = std::mem::replace(&mut state.player.location, exit.destination.clone());
            state.dialogue = None;
            events.push(Event::Moved {
                from,
                to: exit.destination.clone(),
            });
            events.push(Event::LocationViewed {
                location: exit.destination.clone(),
            });
        }
        Command::Talk(id) => {
            if !npc_here(world, state, &id) {
                return Err(EngineError::NotHere(id));
            }
            let npc = world.character(&id).unwrap();
            let start = world
                .dialogue(npc.dialogue.as_ref().unwrap())
                .unwrap()
                .start
                .clone();
            dialogue(world, state, id, start, &mut events);
        }
        Command::ChooseDialogue(number) => {
            let active = state.dialogue.clone().ok_or(EngineError::NoDialogue)?;
            if !npc_here(world, state, &active.npc) {
                return Err(EngineError::NotHere(active.npc));
            }
            let visible = choices(world, state, &active.npc, &active.node);
            let choice = number
                .checked_sub(1)
                .and_then(|i| visible.get(i))
                .ok_or(EngineError::InvalidChoice)?;
            match &choice.effect {
                Some(DialogueEffect::AcceptQuest { quest: id }) => {
                    quest(world, state, id, false, &mut events)?
                }
                Some(DialogueEffect::CompleteQuest { quest: id }) => {
                    quest(world, state, id, true, &mut events)?
                }
                Some(DialogueEffect::SetFlag { flag }) => set_flag(world, state, flag, &mut events),
                None => {}
            }
            // An effect can make the speaker unavailable; the conversation ends then.
            match &choice.next {
                Some(next) if npc_here(world, state, &active.npc) => {
                    dialogue(world, state, active.npc, next.clone(), &mut events)
                }
                _ => {
                    state.dialogue = None;
                    events.push(Event::DialogueEnded);
                }
            }
        }
        Command::AcceptQuest(id) => {
            quest(world, state, &id, false, &mut events)?;
            state.dialogue = None;
        }
        Command::CompleteQuest(id) => {
            quest(world, state, &id, true, &mut events)?;
            state.dialogue = None;
        }
        Command::Engage(id) => encounter::engage(world, state, id, &mut events)?,
        Command::Flee => encounter::flee(world, state, &mut events)?,
        Command::Attack(id) => encounter::player_action(world, state, id, None, &mut events)?,
        Command::UseSkill { skill, target } => {
            let known = world
                .combat()
                .is_some_and(|c| c.player_skills.contains(&skill));
            let skill = world
                .skill(&skill)
                .filter(|_| known)
                .ok_or(EngineError::UnknownSkill(skill))?;
            encounter::player_action(world, state, target, Some(skill), &mut events)?
        }
        Command::Rest => {
            let safe = world.location(&state.player.location).unwrap().safe;
            let Some(combat) = state.combat.as_mut().filter(|_| safe) else {
                return Err(EngineError::NotSafe);
            };
            let stats = player_stats(world, combat.level);
            combat.stance = Stance::Exploring(Vitals {
                hp: stats.hp,
                mp: stats.mp,
            });
            state.dialogue = None;
            events.push(Event::Rested);
        }
    }
    Ok(events)
}

pub(super) fn player_stats(world: &WorldSpec, level: usize) -> Stats {
    world.combat().unwrap().levels[level - 1].stats
}
