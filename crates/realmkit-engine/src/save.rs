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
    let stats = player
        .level
        .checked_sub(1)
        .and_then(|i| world.combat().unwrap().levels.get(i))
        .ok_or("invalid player level")?;
    let next = world.combat().unwrap().levels.get(player.level);
    ensure(
        player.xp >= stats.xp && next.is_none_or(|next| player.xp < next.xp),
        "experience does not match level",
    )?;
    ensure(
        player.max_hp == stats.hp && player.attack == stats.attack && player.hp <= player.max_hp,
        "player stats do not match level",
    )?;
    ensure(
        state.monster_hp.keys().eq(fresh.monster_hp.keys())
            && state
                .monster_hp
                .iter()
                .all(|(id, hp)| *hp <= fresh.monster_hp[id]),
        "invalid monster state",
    )?;
    ensure(
        state.quests.keys().eq(fresh.quests.keys())
            && world.quests.iter().all(|quest| {
                let QuestObjective::Defeat { character } = &quest.objective else {
                    return true;
                };
                let defeated = state.monster_hp.get(character) == Some(&0);
                match state.quests[&quest.id] {
                    QuestStatus::Available => true,
                    QuestStatus::Active => !defeated,
                    QuestStatus::Ready | QuestStatus::Completed => defeated,
                }
            }),
        "invalid quest state",
    )?;
    // Format 1 grants XP, items and quest flags exactly once, from defeated
    // monsters and completed quests, so progress fixes their exact values.
    // Anything else could not have been played, and could overflow later grants.
    let no_flags: &[Id] = &[];
    let defeated = world
        .characters
        .iter()
        .filter(|c| state.monster_hp.get(&c.id) == Some(&0))
        .map(|c| c.combat.as_ref().unwrap())
        .map(|m| (m.xp, &m.loot, no_flags));
    let completed = world
        .quests
        .iter()
        .filter(|q| state.quests[&q.id] == QuestStatus::Completed)
        .map(|q| (q.reward_xp, &q.reward_items, &q.completion_flags[..]));
    let (mut xp, mut inventory, mut quest_flags) = (0_u64, BTreeMap::new(), BTreeSet::new());
    for (reward, stacks, flags) in defeated.chain(completed) {
        xp = xp.checked_add(reward).ok_or("impossible experience")?;
        for stack in stacks {
            let count: &mut u64 = inventory.entry(stack.item.clone()).or_default();
            *count = count
                .checked_add(stack.quantity)
                .ok_or("impossible inventory")?;
        }
        quest_flags.extend(flags.iter().cloned());
    }
    ensure(player.xp == xp, "experience does not match progress")?;
    ensure(
        player.inventory == inventory,
        "inventory does not match progress",
    )?;
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
            && state
                .flags
                .iter()
                .all(|f| quest_flags.contains(f) || dialogue_flags.contains(f)),
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
