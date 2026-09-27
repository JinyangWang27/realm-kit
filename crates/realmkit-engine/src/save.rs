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
    let player = &state.player;
    ensure(
        world.location(&player.location).is_some(),
        "unknown player location",
    )?;
    let stats = player
        .level
        .checked_sub(1)
        .and_then(|i| world.world.levels.get(i))
        .ok_or("invalid player level")?;
    let next = world.world.levels.get(player.level);
    ensure(
        player.xp >= stats.xp && next.is_none_or(|next| player.xp < next.xp),
        "experience does not match level",
    )?;
    ensure(
        player.max_hp == stats.hp && player.attack == stats.attack && player.hp <= player.max_hp,
        "player stats do not match level",
    )?;
    ensure(
        player.inventory.keys().all(|id| world.item(id).is_some()),
        "unknown inventory item",
    )?;
    // Format 1 grants each loot and reward stack at most once, so no quantity
    // can exceed the world's total; this also keeps later grants from overflowing.
    let mut grantable: BTreeMap<&str, u64> = BTreeMap::new();
    let stacks = world.monsters.iter().flat_map(|m| &m.loot);
    for stack in stacks.chain(world.quests.iter().flat_map(|q| &q.reward_items)) {
        let total = grantable.entry(&stack.item).or_default();
        *total = total.saturating_add(stack.quantity);
    }
    ensure(
        player
            .inventory
            .iter()
            .all(|(id, count)| *count <= grantable.get(id.as_str()).copied().unwrap_or(0)),
        "inventory holds more than this world can grant",
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
                let QuestObjective::Defeat { monster } = &quest.objective;
                let defeated = state.monster_hp.get(monster) == Some(&0);
                match state.quests[&quest.id] {
                    QuestStatus::Available => true,
                    QuestStatus::Active => !defeated,
                    QuestStatus::Ready | QuestStatus::Completed => defeated,
                }
            }),
        "invalid quest state",
    )?;
    ensure(
        state
            .flags
            .iter()
            .all(|flag| world.world.flags.contains(flag)),
        "unknown story flag",
    )?;
    if let Some(dialogue) = &state.dialogue {
        let node_exists = world
            .npc(&dialogue.npc)
            .and_then(|npc| world.dialogue(&npc.dialogue))
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
