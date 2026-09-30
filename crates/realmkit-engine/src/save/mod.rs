//! Save checks: a snapshot loads only if this package and its rules could
//! have produced it.

use super::*;

mod combat;

fn ensure(ok: bool, problem: &str) -> Result<(), String> {
    if ok {
        Ok(())
    } else {
        Err(problem.to_string())
    }
}

/// Checks a snapshot against the loaded package and the invariants the rules
/// maintain. `fresh` is the route's starting state, which fixes the key sets.
pub(super) fn check(
    world: &WorldSpec,
    fresh: &GameState,
    snapshot: &SaveSnapshot,
) -> Result<(), String> {
    header(world, snapshot)?;
    let state = &snapshot.state;
    basics(world, state)?;
    quests(world, fresh, state)?;
    let progress = Progress::of(world, state)?;
    if let (Some(combat), Some(rules)) = (&state.combat, world.combat()) {
        combat::check(world, state, combat, rules, &progress)?;
    }
    inventory(world, state, &progress)?;
    flags(world, state, &progress)?;
    dialogue(world, state)
}

/// The snapshot belongs to this format, package, revision and route.
fn header(world: &WorldSpec, snapshot: &SaveSnapshot) -> Result<(), String> {
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
    )
}

/// The turn counter, location, and combat and random state the world implies.
fn basics(world: &WorldSpec, state: &GameState) -> Result<(), String> {
    ensure(state.turn < u64::MAX, "turn counter exhausted")?;
    ensure(
        world.location(&state.player.location).is_some(),
        "unknown player location",
    )?;
    ensure(
        state.combat.is_some() == world.combat().is_some(),
        "combat state does not match the world",
    )?;
    ensure(
        state.rng.is_some() == world.stochastic()
            && state.rng.is_none_or(|r| r.version == RNG_VERSION),
        "random state does not match the world",
    )
}

fn defeated(state: &GameState, id: &str) -> bool {
    state
        .combat
        .as_ref()
        .is_some_and(|c| c.defeated.contains(id))
}

/// A repeatable group's members are never recorded and reward every victory.
fn repeatable(world: &WorldSpec, id: &str) -> bool {
    world
        .character(id)
        .and_then(|c| c.combat.as_ref()?.group.as_deref())
        .and_then(|g| world.group(g))
        .is_some_and(|g| g.repeatable)
}

/// Every quest's status agrees with its objective.
fn quests(world: &WorldSpec, fresh: &GameState, state: &GameState) -> Result<(), String> {
    ensure(
        state.quests.keys().eq(fresh.quests.keys())
            && world.quests.iter().all(|quest| {
                let done = match &quest.objective {
                    // Any status is possible: a repeatable victory leaves no record.
                    QuestObjective::Defeat { character } if repeatable(world, character) => {
                        return true
                    }
                    QuestObjective::Defeat { character } => defeated(state, character),
                    QuestObjective::Flag { flag } => state.flags.contains(flag),
                };
                match state.quests[&quest.id] {
                    QuestStatus::Available => true,
                    QuestStatus::Active => !done,
                    QuestStatus::Ready | QuestStatus::Completed => done,
                }
            }),
        "invalid quest state",
    )
}

/// What recorded progress implies. Completed quests and recorded defeats
/// granted their items exactly once, so they fix a floor for the inventory;
/// only a repeatable group's loot can add more. XP scales by level difference
/// (at most 140% of a defeat's XP), and repeatable victories are unbounded, so
/// XP gets a floor and, without repeatable groups, a ceiling. Anything else
/// could not have been played.
struct Progress<'w> {
    /// Items and counts the recorded progress granted.
    inventory: BTreeMap<Id, u64>,
    quest_flags: BTreeSet<Id>,
    xp_floor: u64,
    xp_ceiling: u64,
    any_repeatable: bool,
    repeatable_loot: BTreeSet<&'w Id>,
    /// Materials crafting spends and items it makes: their counts follow
    /// crafting, not only recorded progress.
    crafted: BTreeSet<&'w Id>,
}

impl<'w> Progress<'w> {
    fn of(world: &'w WorldSpec, state: &GameState) -> Result<Self, String> {
        let (mut floor, mut ceiling) = (0_u64, 0_u64);
        let (mut inventory, mut quest_flags) = (BTreeMap::new(), BTreeSet::new());
        let completed = world
            .quests
            .iter()
            .filter(|q| state.quests[&q.id] == QuestStatus::Completed);
        let recorded = world
            .characters
            .iter()
            .filter(|c| defeated(state, &c.id))
            .filter_map(|c| c.combat.as_ref());
        let stacks = completed
            .clone()
            .flat_map(|q| &q.reward_items)
            .chain(recorded.clone().flat_map(|m| &m.loot));
        // Starting gear counts as held from the start.
        let starting = world
            .combat()
            .into_iter()
            .flat_map(|c| &c.player_equipment)
            .map(|item| (item, 1_u64));
        for (item, quantity) in stacks.map(|s| (&s.item, s.quantity)).chain(starting) {
            let count: &mut u64 = inventory.entry(item.clone()).or_default();
            *count = count.checked_add(quantity).ok_or("impossible inventory")?;
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
        let repeaters = world.characters.iter().filter(|c| repeatable(world, &c.id));
        let recipes = world.combat().into_iter().flat_map(|c| &c.recipes);
        let tiers = world
            .items
            .iter()
            .filter_map(|i| i.equipment.as_ref())
            .flat_map(|e| &e.tiers);
        let crafted = recipes
            .clone()
            .flat_map(|r| &r.inputs)
            .chain(tiers.flat_map(|t| &t.cost))
            .map(|s| &s.item)
            .chain(recipes.map(|r| &r.output))
            .collect();
        Ok(Self {
            crafted,
            inventory,
            quest_flags,
            xp_floor: floor,
            xp_ceiling: ceiling,
            any_repeatable: repeaters.clone().next().is_some(),
            repeatable_loot: repeaters
                .filter_map(|c| c.combat.as_ref())
                .flat_map(|m| &m.loot)
                .map(|s| &s.item)
                .collect(),
        })
    }
}

/// Held items: counts in the inventory, plus every piece of equipment.
/// Equipment never sits in the counts, since it arrives as pieces.
fn inventory(world: &WorldSpec, state: &GameState, progress: &Progress) -> Result<(), String> {
    let mut held = state.player.inventory.clone();
    let wearable = |item: &str| world.item(item).is_some_and(|i| i.equipment.is_some());
    ensure(
        !held.keys().any(|item| wearable(item)),
        "equipment cannot be held as a count",
    )?;
    if let Some(combat) = &state.combat {
        for piece in combat.gear.values() {
            *held.entry(piece.item.clone()).or_default() += 1;
        }
    }
    ensure(
        progress
            .inventory
            .iter()
            .filter(|(item, _)| !progress.crafted.contains(item))
            .all(|(item, count)| held.get(item).is_some_and(|h| h >= count))
            && held.iter().all(|(item, h)| {
                progress.inventory.get(item) == Some(h)
                    || progress.repeatable_loot.contains(item)
                    || progress.crafted.contains(item)
            }),
        "inventory does not match progress",
    )
}

/// Completed quests' flags are set, and every flag has something that sets it.
fn flags(world: &WorldSpec, state: &GameState, progress: &Progress) -> Result<(), String> {
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
        progress.quest_flags.is_subset(&state.flags)
            && state.flags.iter().all(|f| {
                progress.quest_flags.contains(f)
                    || dialogue_flags.contains(f)
                    || group_flags.contains(f)
            }),
        "story flags do not match progress",
    )
}

/// An open conversation is at an existing node, with its speaker here and
/// choices to offer.
fn dialogue(world: &WorldSpec, state: &GameState) -> Result<(), String> {
    let Some(dialogue) = &state.dialogue else {
        return Ok(());
    };
    let node_exists = world
        .character(&dialogue.npc)
        .and_then(|npc| world.dialogue(npc.dialogue.as_ref()?))
        .is_some_and(|d| d.nodes.iter().any(|n| n.id == dialogue.node));
    ensure(
        node_exists
            && rules::npc_here(world, state, &dialogue.npc)
            && !rules::choices(world, state, &dialogue.npc, &dialogue.node).is_empty(),
        "invalid conversation state",
    )
}
