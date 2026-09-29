use super::*;

pub(super) fn conditions_met(state: &GameState, conditions: &[Condition]) -> bool {
    conditions.iter().all(|condition| match condition {
        Condition::Flag { flag } => state.flags.contains(flag),
        Condition::Quest { quest, status } => state.quests.get(quest) == Some(status),
    })
}

pub(super) fn npc_here(world: &WorldSpec, state: &GameState, id: &str) -> bool {
    world
        .location(&state.player.location)
        .unwrap()
        .characters
        .iter()
        .any(|n| n == id)
        && world
            .character(id)
            .is_some_and(|n| n.dialogue.is_some() && conditions_met(state, &n.requires))
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

fn grant_items(
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

fn grant_xp(
    world: &WorldSpec,
    state: &mut GameState,
    amount: u64,
    events: &mut Vec<Event>,
) -> Result<(), EngineError> {
    state.player.xp = state
        .player
        .xp
        .checked_add(amount)
        .ok_or(EngineError::NumericLimit)?;
    events.push(Event::ExperienceGranted { amount });
    while let Some(level) = world.combat().unwrap().levels.get(state.player.level) {
        if state.player.xp < level.xp {
            break;
        }
        state.player.level += 1;
        state.player.hp = level.hp;
        state.player.max_hp = level.hp;
        state.player.attack = level.attack;
        events.push(Event::LevelUp {
            level: state.player.level,
        });
    }
    Ok(())
}

fn set_flag(state: &mut GameState, flag: &str, events: &mut Vec<Event>) {
    if state.flags.insert(flag.into()) {
        events.push(Event::StoryFlagSet { flag: flag.into() });
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
            set_flag(state, flag, events);
        }
    } else {
        state.quests.insert(id.into(), QuestStatus::Active);
        events.push(Event::QuestAccepted { quest: id.into() });
        let QuestObjective::Defeat { character } = &quest.objective else {
            return Ok(());
        };
        // The world remembers earlier kills so accepting late cannot strand this quest.
        if state.monster_hp[character] == 0 {
            state.quests.insert(id.into(), QuestStatus::Ready);
            events.push(Event::QuestProgressed { quest: id.into() });
        }
    }
    Ok(())
}

pub(super) fn actions(world: &WorldSpec, state: &GameState) -> Vec<Action> {
    let location = world.location(&state.player.location).unwrap();
    let available = |command| Action {
        command,
        available: true,
    };
    let mut actions: Vec<_> = location
        .characters
        .iter()
        .filter(|id| npc_here(world, state, id))
        .map(|id| available(Command::Talk(id.clone())))
        .collect();
    actions.extend(
        location
            .characters
            .iter()
            .filter(|id| state.monster_hp.get(*id).is_some_and(|hp| *hp > 0))
            .map(|id| available(Command::Attack(id.clone()))),
    );
    actions.extend(location.exits.iter().map(|(direction, exit)| Action {
        command: Command::Move(*direction),
        available: conditions_met(state, &exit.requires),
    }));
    actions.extend([Command::Inventory, Command::Status, Command::Quests].map(available));
    // Death is not a locked door: offer only what can still be done.
    if state.player.hp == 0 {
        actions.retain(|a| {
            matches!(
                a.command,
                Command::Inventory | Command::Status | Command::Quests
            )
        });
    }
    actions
}

pub(super) fn execute(
    world: &WorldSpec,
    state: &mut GameState,
    command: Command,
) -> Result<Vec<Event>, EngineError> {
    if state.player.hp == 0
        && !matches!(
            command,
            Command::Look | Command::Status | Command::Inventory | Command::Quests
        )
    {
        return Err(EngineError::PlayerDead);
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
                Some(DialogueEffect::SetFlag { flag }) => set_flag(state, flag, &mut events),
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
        Command::Attack(id) => {
            if !world
                .location(&state.player.location)
                .unwrap()
                .characters
                .contains(&id)
                || !state.monster_hp.contains_key(&id)
            {
                return Err(EngineError::NotHere(id));
            }
            let narrative = &world.combat().unwrap().narrative;
            let hp = state.monster_hp.get_mut(&id).unwrap();
            if *hp == 0 {
                return Err(EngineError::AlreadyDefeated(id));
            }
            state.dialogue = None;
            let damage = state.player.attack.min(*hp);
            *hp -= damage;
            events.push(Event::DamageDealt {
                target: id.clone(),
                amount: damage,
                variant: (state.turn % narrative.attack.len() as u64) as usize,
            });
            let monster = world.character(&id).unwrap().combat.as_ref().unwrap();
            if *hp == 0 {
                events.push(Event::EnemyDefeated {
                    monster: id.clone(),
                });
                grant_items(state, &monster.loot, &mut events)?;
                grant_xp(world, state, monster.xp, &mut events)?;
                for q in &world.quests {
                    if q.objective
                        == (QuestObjective::Defeat {
                            character: id.clone(),
                        })
                        && state.quests[&q.id] == QuestStatus::Active
                    {
                        state.quests.insert(q.id.clone(), QuestStatus::Ready);
                        events.push(Event::QuestProgressed {
                            quest: q.id.clone(),
                        });
                    }
                }
            } else {
                let damage = monster.attack.min(state.player.hp);
                state.player.hp -= damage;
                events.push(Event::DamageReceived {
                    source: id,
                    amount: damage,
                    variant: (state.turn % narrative.hurt.len() as u64) as usize,
                });
                if state.player.hp == 0 {
                    events.push(Event::PlayerDied);
                }
            }
        }
    }
    Ok(events)
}
