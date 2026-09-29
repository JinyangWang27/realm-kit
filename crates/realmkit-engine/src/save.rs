use super::*;

/// Checks a snapshot against the loaded package and the invariants the rules
/// maintain. `fresh` is the route's starting state, which fixes the key sets.
pub(super) fn check(
    world: &WorldSpec,
    fresh: &GameState,
    snapshot: &SaveSnapshot,
) -> Result<(), String> {
    let ensure = |ok: bool, problem: &str| if ok { Ok(()) } else { Err(problem.to_string()) };
    ensure(
        snapshot.save_format_version == SAVE_FORMAT_VERSION,
        "unsupported save format version",
    )?;
    ensure(
        snapshot.package_id == world.world.id,
        "it belongs to a different world",
    )?;
    ensure(
        snapshot.package_revision == world.revision(),
        "it was made with a different revision of this world",
    )?;
    ensure(
        snapshot.player_route_id == DEFAULT_ROUTE,
        "unknown player route",
    )?;
    let state = &snapshot.state;
    ensure(state.turn < u64::MAX, "turn counter exhausted")?;
    let player = &state.player;
    ensure(
        world.location(&player.location).is_some(),
        "unknown player location",
    )?;
    ensure(
        state.combat.is_some() == world.combat().is_some(),
        "combat state does not match the world",
    )?;
    let defeated = |id: &str| {
        state
            .combat
            .as_ref()
            .is_some_and(|c| c.defeated.contains(id))
    };
    // A repeatable group's members are never recorded and reward every victory.
    let repeatable = |id: &str| {
        world
            .character(id)
            .and_then(|c| c.combat.as_ref()?.group.as_deref())
            .and_then(|g| world.group(g))
            .is_some_and(|g| g.repeatable)
    };
    ensure(
        state.quests.keys().eq(fresh.quests.keys())
            && world.quests.iter().all(|quest| {
                let done = match &quest.objective {
                    // Any status is possible: a repeatable victory leaves no record.
                    QuestObjective::Defeat { character } if repeatable(character) => return true,
                    QuestObjective::Defeat { character } => defeated(character),
                    QuestObjective::Flag { flag } => state.flags.contains(flag),
                };
                match state.quests[&quest.id] {
                    QuestStatus::Available => true,
                    QuestStatus::Active => !done,
                    QuestStatus::Ready | QuestStatus::Completed => done,
                }
            }),
        "invalid quest state",
    )?;
    // Completed quests and recorded defeats granted their items exactly once,
    // so they fix a floor for the inventory; only a repeatable group's loot can
    // add more. XP scales by level difference (at most 140% of a defeat's XP),
    // and repeatable victories are unbounded, so XP gets a floor and, without
    // repeatable groups, a ceiling. Anything else could not have been played.
    let (mut floor, mut ceiling) = (0_u64, 0_u64);
    let (mut inventory, mut quest_flags) = (BTreeMap::new(), BTreeSet::new());
    let completed = world
        .quests
        .iter()
        .filter(|q| state.quests[&q.id] == QuestStatus::Completed);
    let recorded = world
        .characters
        .iter()
        .filter(|c| defeated(&c.id))
        .filter_map(|c| c.combat.as_ref());
    let stacks = completed
        .clone()
        .flat_map(|q| &q.reward_items)
        .chain(recorded.clone().flat_map(|m| &m.loot));
    for stack in stacks {
        let count: &mut u64 = inventory.entry(stack.item.clone()).or_default();
        *count = count
            .checked_add(stack.quantity)
            .ok_or("impossible inventory")?;
    }
    for quest in completed {
        floor = floor
            .checked_add(quest.reward_xp)
            .ok_or("impossible experience")?;
        quest_flags.extend(quest.completion_flags.iter().cloned());
    }
    for fighter in recorded {
        let most = xp_for_defeat(fighter.xp, 1, usize::MAX);
        ceiling = ceiling.checked_add(most).ok_or("impossible experience")?;
    }
    let ceiling = floor.checked_add(ceiling).ok_or("impossible experience")?;
    let any_repeatable = world.characters.iter().any(|c| repeatable(&c.id));
    let repeatable_loot: BTreeSet<_> = world
        .characters
        .iter()
        .filter(|c| repeatable(&c.id))
        .filter_map(|c| c.combat.as_ref())
        .flat_map(|m| &m.loot)
        .map(|s| &s.item)
        .collect();
    if let (Some(combat), Some(rules)) = (&state.combat, world.combat()) {
        let stats = combat
            .level
            .checked_sub(1)
            .and_then(|i| rules.levels.get(i))
            .ok_or("invalid player level")?;
        let next = rules.levels.get(combat.level);
        ensure(
            combat.xp >= stats.xp && next.is_none_or(|next| combat.xp < next.xp),
            "experience does not match level",
        )?;
        ensure(
            combat.defeated.iter().all(|id| {
                !repeatable(id)
                    && world.character(id).is_some_and(|c| c.combat.is_some())
                    && world.locations.iter().any(|l| l.characters.contains(id))
            }),
            "invalid defeated characters",
        )?;
        match &combat.stance {
            Stance::Exploring(v) => ensure(
                v.hp <= stats.stats.hp && v.mp <= stats.stats.mp,
                "player vitals exceed their maximums",
            )?,
            Stance::Fighting(encounter) => encounter_state(world, state, combat, rules, encounter)?,
        }
        ensure(
            combat.xp >= floor && (any_repeatable || combat.xp <= ceiling),
            "experience does not match progress",
        )?;
    }
    ensure(
        inventory
            .iter()
            .all(|(item, count)| player.inventory.get(item).is_some_and(|held| held >= count))
            && player.inventory.iter().all(|(item, held)| {
                inventory.get(item) == Some(held) || repeatable_loot.contains(item)
            }),
        "inventory does not match progress",
    )?;
    let group_flags: BTreeSet<_> = world
        .combat()
        .into_iter()
        .flat_map(|c| &c.groups)
        .flat_map(|g| g.victory_flags.iter().chain(&g.defeat_flags))
        .cloned()
        .collect();
    let dialogue_flags: BTreeSet<_> = world
        .dialogues
        .iter()
        .flat_map(|d| &d.nodes)
        .flat_map(|n| &n.choices)
        .filter_map(|c| match &c.effect {
            Some(DialogueEffect::SetFlag { flag }) => Some(flag.clone()),
            _ => None,
        })
        .collect();
    ensure(
        quest_flags.is_subset(&state.flags)
            && state.flags.iter().all(|f| {
                quest_flags.contains(f) || dialogue_flags.contains(f) || group_flags.contains(f)
            }),
        "story flags do not match progress",
    )?;
    if let Some(dialogue) = &state.dialogue {
        let node_exists = world
            .character(&dialogue.npc)
            .and_then(|npc| world.dialogue(npc.dialogue.as_ref()?))
            .is_some_and(|d| d.nodes.iter().any(|n| n.id == dialogue.node));
        ensure(
            node_exists
                && rules::npc_here(world, state, &dialogue.npc)
                && !rules::choices(world, state, &dialogue.npc, &dialogue.node).is_empty(),
            "invalid conversation state",
        )?;
    }
    Ok(())
}

/// An encounter is consistent with the rules that produce it: the player
/// first, opponents present and undefeated, vitals within maxima, remainders
/// below one point and turns not in the past.
fn encounter_state(
    world: &WorldSpec,
    state: &GameState,
    combat: &CombatState,
    rules: &realmkit_spec::Combat,
    encounter: &Encounter,
) -> Result<(), String> {
    let invalid = || "invalid encounter state".to_string();
    let per_point =
        100 * u128::from(encounter::baseline_turn(&rules.timeline).map_err(|_| invalid())?);
    let (player, opponents) = encounter.participants.split_first().ok_or_else(invalid)?;
    let player_ok = player.character == world.world.player
        && player.side == 0
        && player.control == Control::Player;
    // Exactly the opponents engaging the first one would bring in.
    let expected = opponents
        .first()
        .filter(|p| rules::character_here(world, state, &p.character).is_some())
        .map(|p| encounter::opponents_for(world, state, &p.character));
    let listed: Vec<_> = opponents.iter().map(|p| p.character.clone()).collect();
    let opponents_ok = expected.as_ref() == Some(&listed)
        && opponents.iter().all(|p| {
            p.side == 1
                && p.control == Control::Policy
                && !combat.defeated.contains(&p.character)
                && rules::character_here(world, state, &p.character)
                    .is_some_and(|c| c.combat.is_some())
        });
    let ids: BTreeSet<_> = encounter
        .participants
        .iter()
        .map(|p| &p.character)
        .collect();
    // Identities first: stats exist only for the player and characters that can fight.
    if !(player_ok && opponents_ok && ids.len() == encounter.participants.len()) {
        return Err(invalid());
    }
    let yields = encounter::group(world, encounter).is_some_and(|g| g.yield_share.is_some());
    let vitals_ok = encounter.participants.iter().all(|p| {
        let max = encounter::stats(world, combat.level, p);
        // Only a yielding group yields, and a yielder is alive.
        (!p.yielded || (yields && p.hp > 0))
            && p.hp <= max.hp
            && p.mp <= max.mp
            && u128::from(p.mp_remainder) < per_point
            && p.rage_remainder < u64::from(max.hp)
            && (p.hp == 0 || p.next_time >= encounter.now)
    });
    // A live encounter always rests at the player's turn: the player acts now,
    // and every opponent later or, on a tie, after the player's side.
    let paused = player.hp == 0 || player.next_time == encounter.now;
    // A finished fight never stays open: the player still fights an opponent who
    // still fights, or the player is dead.
    let open = player.hp == 0 || (player.fighting() && opponents.iter().any(Participant::fighting));
    if vitals_ok && paused && open && state.dialogue.is_none() {
        Ok(())
    } else {
        Err(invalid())
    }
}
