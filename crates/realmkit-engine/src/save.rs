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
    ensure(
        state.quests.keys().eq(fresh.quests.keys())
            && world.quests.iter().all(|quest| {
                let done = match &quest.objective {
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
    // XP, items and quest flags are granted exactly once, from defeated
    // characters and completed quests, so progress fixes their exact values.
    // Anything else could not have been played, and could overflow later grants.
    let no_flags: &[Id] = &[];
    let defeated = world
        .characters
        .iter()
        .filter(|c| defeated(&c.id))
        .filter_map(|c| c.combat.as_ref())
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
                world.character(id).is_some_and(|c| c.combat.is_some())
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
        ensure(combat.xp == xp, "experience does not match progress")?;
    }
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
    // Engage brings in exactly one opponent until authored groups exist.
    let opponents_ok = opponents.len() == 1
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
    let vitals_ok = encounter.participants.iter().all(|p| {
        let max = encounter::stats(world, combat.level, p);
        p.hp <= max.hp
            && p.mp <= max.mp
            && u128::from(p.mp_remainder) < per_point
            && p.rage_remainder < u64::from(max.hp)
            && (p.hp == 0 || p.next_time >= encounter.now)
    });
    // A live encounter always rests at the player's turn: the player acts now,
    // and every opponent later or, on a tie, after the player's side.
    let paused = player.hp == 0 || player.next_time == encounter.now;
    // A finished fight never stays open: the player is alive with an opponent, or dead.
    let open = player.hp == 0 || opponents.iter().any(|p| p.hp > 0);
    if vitals_ok && paused && open && state.dialogue.is_none() {
        Ok(())
    } else {
        Err(invalid())
    }
}
