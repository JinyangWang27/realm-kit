//! Conversations, quests, story flags and the rewards they grant.

use super::*;

pub(crate) fn choices<'a>(
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
        .filter(|c| allowed(state, c.requires.as_ref()))
        .collect()
}

pub(crate) fn dialogue(
    world: &WorldSpec,
    state: &mut GameState,
    npc: Id,
    node: Id,
    events: &mut Vec<Event>,
) {
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

/// Equipment arrives as individual pieces in the pack; other items as counts.
pub(crate) fn grant_items(
    world: &WorldSpec,
    state: &mut GameState,
    stacks: &[ItemStack],
    events: &mut Vec<Event>,
) -> Result<(), EngineError> {
    for stack in stacks {
        let wearable = world
            .item(&stack.item)
            .is_some_and(|i| i.equipment.is_some());
        if let (true, Some(combat)) = (wearable, state.combat.as_mut()) {
            gear::receive(combat, &stack.item, stack.quantity)?;
            events.push(Event::ItemReceived {
                item: stack.item.clone(),
                quantity: stack.quantity,
            });
            continue;
        }
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
pub(crate) fn grant_xp(
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
        // Levelling up restores HP and MP fully, to the effective maxima.
        combat.level += 1;
        let max = player_stats(world, combat);
        combat.stance = Stance::Exploring(Vitals {
            hp: max.hp,
            mp: max.mp,
        });
        events.push(Event::LevelUp {
            level: combat.level,
            mp_restored: max.mp > 0,
        });
    }
    Ok(())
}

pub(crate) fn progress(state: &mut GameState, quest: &Id, events: &mut Vec<Event>) {
    state.quests.insert(quest.clone(), QuestStatus::Ready);
    events.push(Event::QuestProgressed {
        quest: quest.clone(),
    });
}

pub(crate) fn set_flag(
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

pub(crate) fn quest(
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
        grant_items(world, state, &quest.reward_items, events)?;
        for grant in &quest.reward_techniques {
            techniques::grant(world, state, grant, events)?;
        }
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

/// Applies effects in authored order to the staged state. An error refuses
/// the whole command, so nothing they did is kept.
pub(crate) fn apply(
    world: &WorldSpec,
    state: &mut GameState,
    effects: &[Effect],
    events: &mut Vec<Event>,
) -> Result<(), EngineError> {
    for effect in effects {
        match effect {
            Effect::AcceptQuest { quest: id } => quest(world, state, id, false, events)?,
            Effect::CompleteQuest { quest: id } => quest(world, state, id, true, events)?,
            Effect::SetFlag { flag } => set_flag(world, state, flag, events),
            Effect::GrantTechnique(grant) => techniques::grant(world, state, grant, events)?,
            Effect::GrantItems { items } => grant_items(world, state, items, events)?,
            Effect::TakeItems { items } => take_items(state, items, events)?,
        }
    }
    Ok(())
}

/// Hands over counted items, all or nothing.
pub(crate) fn take_items(
    state: &mut GameState,
    stacks: &[ItemStack],
    events: &mut Vec<Event>,
) -> Result<(), EngineError> {
    for stack in stacks {
        let held = state
            .player
            .inventory
            .get(&stack.item)
            .copied()
            .unwrap_or(0);
        let left = held
            .checked_sub(stack.quantity)
            .ok_or_else(|| EngineError::NotEnoughMaterials(stack.item.clone()))?;
        if left == 0 {
            state.player.inventory.remove(&stack.item);
        } else {
            state.player.inventory.insert(stack.item.clone(), left);
        }
        events.push(Event::ItemsSpent {
            item: stack.item.clone(),
            quantity: stack.quantity,
        });
    }
    Ok(())
}

pub(crate) fn talk(
    world: &WorldSpec,
    state: &mut GameState,
    id: Id,
    events: &mut Vec<Event>,
) -> Result<(), EngineError> {
    if !npc_here(world, state, &id) {
        return Err(EngineError::NotHere(id));
    }
    let npc = world.character(&id).unwrap();
    let start = world
        .dialogue(npc.dialogue.as_ref().unwrap())
        .unwrap()
        .start
        .clone();
    dialogue(world, state, id, start, events);
    Ok(())
}

/// Takes a visible choice: its effect, then its next line, if the speaker is
/// still here to say it.
pub(crate) fn choose(
    world: &WorldSpec,
    state: &mut GameState,
    number: usize,
    events: &mut Vec<Event>,
) -> Result<(), EngineError> {
    let active = state.dialogue.clone().ok_or(EngineError::NoDialogue)?;
    if !npc_here(world, state, &active.npc) {
        return Err(EngineError::NotHere(active.npc));
    }
    let visible = choices(world, state, &active.npc, &active.node);
    let choice = number
        .checked_sub(1)
        .and_then(|i| visible.get(i))
        .ok_or(EngineError::InvalidChoice)?;
    apply(world, state, &choice.effects, events)?;
    // An effect can make the speaker unavailable; the conversation ends then.
    match &choice.next {
        Some(next) if npc_here(world, state, &active.npc) => {
            dialogue(world, state, active.npc, next.clone(), events)
        }
        _ => {
            state.dialogue = None;
            events.push(Event::DialogueEnded);
        }
    }
    Ok(())
}
