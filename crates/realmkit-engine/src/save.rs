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
    ensure(
        state.monster_hp.keys().eq(fresh.monster_hp.keys())
            && state
                .monster_hp
                .iter()
                .all(|(id, hp)| *hp <= fresh.monster_hp[id]),
        "invalid monster state",
    )?;
    ensure(
        state.quests.keys().eq(fresh.quests.keys()),
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
