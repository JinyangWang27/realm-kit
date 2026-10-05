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
        .filter(|c| !offers_locked_quest(world, state, c))
        .collect()
}

/// Whether taking the choice would be refused because a quest it accepts
/// is not yet open. Earlier effects in its list count: completing one quest
/// may open the next. Only choices accepting a gated quest are tried.
// ponytail: tries the effects on a copy of the state each time choices are
// listed; cache per node if worlds grow large.
fn offers_locked_quest(world: &WorldSpec, state: &GameState, choice: &DialogueChoice) -> bool {
    let gated = choice.effects.iter().any(|e| {
        matches!(e, Effect::AcceptQuest { quest } if world.quest(quest).unwrap().requires.is_some())
    });
    gated
        && matches!(
            apply(world, &mut state.clone(), &choice.effects, &mut Vec::new()),
            Err(EngineError::QuestLocked(_))
        )
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
    if !complete && !allowed(state, quest.requires.as_ref()) {
        return Err(EngineError::QuestLocked(id.into()));
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

/// Moves the story on to `phase`, passing any phases between; a phase
/// already reached changes nothing.
fn enter_phase(world: &WorldSpec, state: &mut GameState, phase: &str, events: &mut Vec<Event>) {
    let index = world.phase_index(phase).unwrap();
    if index < state.phases.len() {
        return;
    }
    let passed = world.world.phases[state.phases.len()..=index].iter();
    state.phases.extend(passed.map(|p| p.id.clone()));
    events.push(Event::PhaseEntered {
        phase: phase.into(),
    });
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
            Effect::GrantCurrency { amount } => economy::grant(state, *amount, events)?,
            Effect::PayCurrency { amount } => economy::pay(state, *amount, events)?,
            Effect::BuyWorkshop { workshop } => {
                economy::buy_workshop(world, state, workshop, events)?
            }
            Effect::SellWorkshop { workshop } => {
                economy::sell_workshop(world, state, workshop, events)?
            }
            Effect::RaiseProficiency { proficiency, ranks } => {
                proficiency::raise(world, state, *proficiency, *ranks, events)
            }
            Effect::EnterPhase { phase } => enter_phase(world, state, phase, events),
            Effect::DiscoverEvidence { evidence } => {
                if state.evidence.insert(evidence.clone()) {
                    events.push(Event::EvidenceDiscovered {
                        evidence: evidence.clone(),
                    });
                }
            }
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

/// The current interpretation of known evidence: the last whose `when`
/// holds, or none when the evidence is unknown or has no interpretations.
pub(crate) fn reading(world: &WorldSpec, state: &GameState, evidence: &str) -> Option<usize> {
    if !state.evidence.contains(evidence) {
        return None;
    }
    world
        .evidence(evidence)?
        .interpretations
        .iter()
        .rposition(|i| allowed(state, i.when.as_ref()))
}

/// Each known evidence, in authored order, with its current reading.
pub(crate) fn readings<'w>(
    world: &'w WorldSpec,
    state: &GameState,
) -> Vec<(&'w Id, Option<usize>)> {
    world
        .world
        .evidence
        .iter()
        .filter(|e| state.evidence.contains(&e.id))
        .map(|e| (&e.id, reading(world, state, &e.id)))
        .collect()
}

pub(crate) fn journal(world: &WorldSpec, state: &GameState) -> Journal {
    let known = |q: &&Quest| {
        state.quests[&q.id] != QuestStatus::Available || allowed(state, q.requires.as_ref())
    };
    let main = world.quests.iter().filter(|q| q.main);
    let side = world.quests.iter().filter(|q| !q.main);
    Journal {
        phase: state.phases.last().cloned(),
        quests: main
            .chain(side)
            .filter(known)
            .map(|q| JournalQuest {
                quest: q.id.clone(),
                main: q.main,
                status: state.quests[&q.id],
            })
            .collect(),
        evidence: world
            .world
            .evidence
            .iter()
            .filter(|e| state.evidence.contains(&e.id))
            .map(|e| e.id.clone())
            .collect(),
        outcome: state.outcome.clone(),
    }
}
