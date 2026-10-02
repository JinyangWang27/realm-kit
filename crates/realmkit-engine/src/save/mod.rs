//! Save checks: a snapshot loads only if this package and its rules could
//! have produced it.

use super::*;

mod battle;
mod combat;
mod retinue;

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
    // The roster first: a battle's check sums its bounded squads.
    retinue::check(world, state)?;
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
    // Each stream exists exactly when content draws from it.
    ensure(
        match state.rng {
            None => !world.stochastic(),
            Some(r) => {
                r.version == RNG_VERSION
                    && r.combat.is_some() == world.random_combat()
                    && r.world.is_some() == world.random_world()
                    && r.market.is_some() == world.random_market()
                    && r.battle.is_some() == world.random_battle()
                    && r.stock.is_some() == world.random_stock()
            }
        },
        "random state does not match the world",
    )?;
    time(world, state)?;
    economy(world, state)?;
    proficiencies(world, state)
}

/// Only authored proficiencies, each gained and within its top rank, with
/// no more points trained than the saved level has granted.
fn proficiencies(world: &WorldSpec, state: &GameState) -> Result<(), String> {
    let level = state.combat.as_ref().map_or(0, |c| c.level);
    // An unknown level is reported by the combat check.
    let reached = world.combat().is_none_or(|c| level <= c.levels.len());
    let granted = if reached {
        rules::granted_proficiency_points(world, level)
    } else {
        0
    };
    let trained: u64 = state
        .proficiencies
        .values()
        .map(|p| u64::from(p.trained))
        .sum();
    ensure(
        trained <= granted
            && state.proficiencies.iter().all(|(proficiency, held)| {
                let rank = u64::from(held.trained) + u64::from(held.taught);
                world
                    .proficiency_max(*proficiency)
                    .is_some_and(|max| rank > 0 && rank <= u64::from(max))
            }),
        "proficiencies do not match the world",
    )
}

/// Currency within its bound, and an index for every good at every market
/// within the index bounds.
fn economy(world: &WorldSpec, state: &GameState) -> Result<(), String> {
    let valid = match (world.economy(), &state.economy) {
        (None, None) => true,
        (Some(economy), Some(wallet)) => {
            let [low, high] = economy.index_bounds;
            wallet.currency <= realmkit_spec::CURRENCY_BOUND
                && wallet.prices.len() == economy.markets.len()
                && economy.markets.iter().all(|market| {
                    wallet.prices.get(&market.location).is_some_and(|prices| {
                        prices.len() == economy.goods.len()
                            && economy.goods.iter().all(|good| {
                                prices
                                    .get(&good.item)
                                    .is_some_and(|i| (low..=high).contains(i))
                            })
                    })
                })
        }
        _ => false,
    };
    ensure(valid, "currency or prices do not match the world")?;
    let (Some(economy), Some(wallet)) = (world.economy(), &state.economy) else {
        return Ok(());
    };
    let every_market = |keys: Vec<&Id>| {
        keys.len() == economy.markets.len()
            && economy.markets.iter().all(|m| keys.contains(&&m.location))
    };
    ensure(
        match economy.prosperity {
            Some(_) => {
                every_market(wallet.prosperity.keys().collect())
                    && wallet
                        .prosperity
                        .values()
                        .all(|p| *p <= realmkit_spec::PROSPERITY_BOUND)
            }
            None => wallet.prosperity.is_empty(),
        },
        "prosperity does not match the world",
    )?;
    ensure(
        match economy.stock {
            Some(_) => {
                every_market(wallet.stock.keys().collect())
                    && wallet.stock.values().all(|stock| {
                        stock.currency <= realmkit_spec::CURRENCY_BOUND
                            && stock.goods.len() == economy.goods.len()
                            && economy.goods.iter().all(|g| {
                                stock
                                    .goods
                                    .get(&g.item)
                                    .is_some_and(|n| *n <= realmkit_spec::STOCK_BOUND)
                            })
                    })
            }
            None => wallet.stock.is_empty(),
        },
        "merchants' stock does not match the world",
    )?;
    // Workshops stand only in towns, within each town's limit.
    let limit = economy.workshops.as_ref().map_or(0, |w| u64::from(w.limit));
    ensure(
        wallet.workshops.iter().all(|(town, kinds)| {
            let in_town = economy
                .market(town)
                .is_some_and(|m| m.kind == realmkit_spec::MarketKind::Town);
            let total: u64 = kinds.values().map(|n| u64::from(*n)).sum();
            in_town
                && !kinds.is_empty()
                && total <= limit
                && kinds
                    .iter()
                    .all(|(kind, n)| *n > 0 && economy.workshop(kind).is_some())
        }),
        "workshops do not match the world",
    )
}

/// The clock is within its bounds, and every mover is somewhere it may be.
fn time(world: &WorldSpec, state: &GameState) -> Result<(), String> {
    ensure(
        match (&world.world.time, state.time) {
            (Some(time), Some(now)) => now >= time.start && now <= realmkit_spec::WORLD_TIME_BOUND,
            (None, None) => true,
            _ => false,
        },
        "world time does not match the world",
    )?;
    let movers: Vec<_> = world
        .characters
        .iter()
        .filter_map(|c| Some((&c.id, c.moves.as_ref()?)))
        .collect();
    // Before its first move, a mover is still where it was placed.
    let now = state.time.unwrap_or(0);
    let placed = |id: &Id| world.locations.iter().find(|l| l.characters.contains(id));
    ensure(
        state.whereabouts.len() == movers.len()
            && movers.iter().all(|(id, moves)| {
                state.whereabouts.get(*id).is_some_and(|at| {
                    moves.among.contains(at)
                        && (now >= moves.schedule.at || placed(id).is_some_and(|l| &l.id == at))
                })
            }),
        "a character who moves is somewhere it cannot be",
    )
}

/// Effects that could have happened by the saved minute: every dialogue
/// choice's, and every event's whose first occurrence has come.
pub(super) fn fired<'w>(world: &'w WorldSpec, state: &GameState) -> Vec<&'w Effect> {
    let now = state.time.unwrap_or(0);
    let choices = world
        .dialogues
        .iter()
        .flat_map(|d| &d.nodes)
        .flat_map(|n| &n.choices)
        .flat_map(|c| &c.effects);
    let events = world
        .world
        .events
        .iter()
        .filter(|e| e.schedule.at <= now)
        .flat_map(|e| &e.effects);
    choices.chain(events).collect()
}

/// Whether an optional requirement could have held at some earlier moment.
fn lasting(world: &WorldSpec, state: &GameState, requires: Option<&Condition>) -> bool {
    requires.is_none_or(|c| could_have_been(world, state, c, true))
}

/// Whether `condition` could once have evaluated to `value`. Flags are never
/// cleared and ranks never fall, so a flag or rank that was once required
/// still holds. Every flag starts unset, and techniques other than the
/// starting ones start unlearned; quest states and items move both ways, so
/// anything else proves nothing. Branches are judged separately, which can
/// only accept more.
fn could_have_been(
    world: &WorldSpec,
    state: &GameState,
    condition: &Condition,
    value: bool,
) -> bool {
    let could = |c, v| could_have_been(world, state, c, v);
    match condition {
        Condition::All { of } if value => of.iter().all(|c| could(c, true)),
        Condition::All { of } => of.iter().any(|c| could(c, false)),
        Condition::Any { of } if value => of.iter().any(|c| could(c, true)),
        Condition::Any { of } => of.iter().all(|c| could(c, false)),
        Condition::Not { condition } => could(condition, !value),
        Condition::Flag { .. } | Condition::Technique { .. } | Condition::Proficiency { .. }
            if value =>
        {
            rules::holds(state, condition)
        }
        // A starting technique was known at that rank from the first moment.
        Condition::Technique { technique, rank } => !world
            .combat()
            .into_iter()
            .flat_map(|c| &c.player_techniques)
            .any(|g| &g.technique == technique && g.rank.unwrap_or(1) >= *rank),
        _ => true,
    }
}

/// Crafting that trains a technique teaches it, so it must be learned.
fn trained(state: &GameState, grant: Option<&realmkit_spec::TechniqueGrant>) -> bool {
    grant.is_none_or(|g| {
        state
            .combat
            .as_ref()
            .is_some_and(|c| c.techniques.contains_key(&g.technique))
    })
}

/// A recipe the player could have used, as far as a snapshot can tell.
fn qualified(world: &WorldSpec, state: &GameState, recipe: &realmkit_spec::Recipe) -> bool {
    lasting(world, state, recipe.known_when.as_ref())
        && lasting(world, state, recipe.requires.as_ref())
        && trained(state, recipe.trains.as_ref())
}

fn defeated(state: &GameState, id: &str) -> bool {
    state
        .combat
        .as_ref()
        .is_some_and(|c| c.defeated.contains(id))
}

/// A repeatable group's members and repeatable hostile armies are never
/// recorded and reward every victory.
fn repeatable(world: &WorldSpec, id: &str) -> bool {
    let army = world
        .character(id)
        .and_then(|c| c.army.as_ref())
        .is_some_and(|a| a.repeatable && a.joins.is_none());
    army || world
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
    /// Items that can come again and again (a repeatable group's loot, a
    /// market's wares): counts may rise above what progress granted, never
    /// fall below it.
    renewable: BTreeSet<&'w Id>,
    /// Items effects grant or take, which a choice can do any number of
    /// times, so their counts follow from nothing.
    loose: BTreeSet<&'w Id>,
    /// Effects that could have happened by the saved minute.
    fired: Vec<&'w Effect>,
    /// Items effects take or players use up, never grant: their counts can
    /// fall below what progress granted, but never rise above it.
    taken: BTreeSet<&'w Id>,
    /// Materials crafting spends: at most what progress granted, maybe less.
    spendable: BTreeSet<&'w Id>,
    /// Items crafting makes: at least what progress granted, maybe more.
    forged: BTreeSet<&'w Id>,
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
        let armies = world
            .characters
            .iter()
            .filter(|c| defeated(state, &c.id))
            .filter_map(|c| c.army.as_ref());
        let stacks = completed
            .clone()
            .flat_map(|q| &q.reward_items)
            .chain(recorded.clone().flat_map(|m| &m.loot))
            .chain(armies.clone().flat_map(|a| &a.loot));
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
        // A won battle gives the player at most the army's whole XP.
        for army in armies {
            ceiling = ceiling
                .checked_add(army.xp)
                .ok_or("impossible experience")?;
        }
        let ceiling = floor.checked_add(ceiling).ok_or("impossible experience")?;
        let repeaters = world.characters.iter().filter(|c| repeatable(world, &c.id));
        let recipes = world.combat().into_iter().flat_map(|c| &c.recipes);
        let tiers = world
            .items
            .iter()
            .filter_map(|i| i.equipment.as_ref())
            .flat_map(|e| &e.tiers);
        let catalysts = world
            .combat()
            .into_iter()
            .flat_map(|c| &c.enchantments)
            .flat_map(|e| &e.catalyst);
        let spendable = recipes
            .clone()
            .flat_map(|r| &r.inputs)
            .chain(tiers.flat_map(|t| &t.cost))
            .chain(catalysts)
            .map(|s| &s.item)
            .collect();
        // Trade goods come and go at markets too; wares only come.
        let goods = world.economy().into_iter().flat_map(|e| &e.goods);
        let wares = world
            .economy()
            .into_iter()
            .flat_map(|e| &e.markets)
            .flat_map(|m| &m.wares);
        let fired = fired(world, state);
        let stacks = |taking: bool| {
            fired.iter().flat_map(move |e| match e {
                Effect::GrantItems { items } if !taking => items.as_slice(),
                Effect::TakeItems { items } if taking => items.as_slice(),
                _ => &[],
            })
        };
        let loose: BTreeSet<&Id> = stacks(false)
            .map(|s| &s.item)
            .chain(goods.map(|g| &g.item))
            .collect();
        let consumables = world
            .items
            .iter()
            .filter(|i| i.consumable.is_some())
            .map(|i| &i.id);
        let taken = stacks(true)
            .map(|s| &s.item)
            .chain(consumables)
            .filter(|item| !loose.contains(item))
            .collect();
        Ok(Self {
            fired,
            loose,
            taken,
            spendable,
            forged: recipes.map(|r| &r.output).collect(),
            inventory,
            quest_flags,
            xp_floor: floor,
            xp_ceiling: ceiling,
            any_repeatable: repeaters.clone().next().is_some(),
            renewable: repeaters
                .flat_map(|c| {
                    let fighter = c.combat.iter().flat_map(|m| &m.loot);
                    fighter.chain(c.army.iter().flat_map(|a| &a.loot))
                })
                .map(|s| &s.item)
                .chain(wares.map(|w| &w.item))
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
            .filter(|(item, _)| {
                !progress.spendable.contains(item)
                    && !progress.loose.contains(item)
                    && !progress.taken.contains(item)
            })
            .all(|(item, count)| held.get(item).is_some_and(|h| h >= count))
            && held.iter().all(|(item, h)| {
                let granted = progress.inventory.get(item).copied().unwrap_or(0);
                *h == granted
                    || progress.renewable.contains(item)
                    || progress.loose.contains(item)
                    || ((progress.spendable.contains(item) || progress.taken.contains(item))
                        && *h < granted)
                    || progress.forged.contains(item)
            }),
        "inventory does not match progress",
    )?;
    // Only crafting takes materials away: what is held plus what crafting
    // spent matches what was granted, between the cheapest and dearest
    // recipes that could have made each forged piece.
    let (least, most) = spent(world, state, progress);
    ensure(
        progress.spendable.iter().all(|item| {
            let held = held.get(*item).copied().unwrap_or(0);
            let granted = progress.inventory.get(*item).copied().unwrap_or(0);
            let at = |spent: &BTreeMap<&Id, u64>| {
                held.saturating_add(spent.get(*item).copied().unwrap_or(0))
            };
            // Something taken may also have been handed over, so then only
            // the upper bound holds.
            progress.renewable.contains(item)
                || progress.loose.contains(item)
                || (at(&least) <= granted
                    && (progress.taken.contains(item) || at(&most) >= granted))
        }),
        "crafting does not match the materials it needed",
    )
}

/// The fewest and the most materials that could have made the pieces held:
/// the cheapest and the dearest recipe for each piece beyond those granted,
/// plus every tier each piece has risen through.
fn spent<'w>(
    world: &'w WorldSpec,
    state: &GameState,
    progress: &Progress,
) -> (BTreeMap<&'w Id, u64>, BTreeMap<&'w Id, u64>) {
    let (mut least, mut most): (BTreeMap<&Id, u64>, BTreeMap<&Id, u64>) = Default::default();
    let (Some(combat), Some(rules)) = (&state.combat, world.combat()) else {
        return (least, most);
    };
    let add = |spent: &mut BTreeMap<&'w Id, u64>, item: &'w Id, quantity: u64| {
        let total = spent.entry(item).or_default();
        *total = total.saturating_add(quantity);
    };
    for output in &progress.forged {
        // Pieces are never lost, so those beyond the grants were forged;
        // with repeatable loot, any number of them may have dropped instead.
        let pieces = combat.gear.values().filter(|g| &&g.item == output).count() as u64;
        let granted = progress.inventory.get(*output).copied().unwrap_or(0);
        let forged = pieces.saturating_sub(granted);
        // Only recipes the player could have used explain a piece.
        let recipes: Vec<_> = rules
            .recipes
            .iter()
            .filter(|r| &&r.output == output && qualified(world, state, r))
            .collect();
        let materials: BTreeSet<&Id> = recipes
            .iter()
            .flat_map(|r| &r.inputs)
            .map(|s| &s.item)
            .collect();
        for material in materials {
            let cost = |r: &&realmkit_spec::Recipe| {
                r.inputs
                    .iter()
                    .find(|s| &s.item == material)
                    .map_or(0, |s| s.quantity)
            };
            // ponytail: per-material bounds are loose when several recipes make
            // one item (a recipe without a material counts as 0 of it); an
            // exact check searches assignments of recipes to pieces.
            let cheapest = recipes.iter().map(cost).min().unwrap_or(0);
            let dearest = recipes.iter().map(cost).max().unwrap_or(0);
            if !progress.renewable.contains(output) && !progress.loose.contains(output) {
                add(&mut least, material, cheapest.saturating_mul(forged));
            }
            add(&mut most, material, dearest.saturating_mul(forged));
        }
    }
    for gear in combat.gear.values() {
        let tiers = &world
            .item(&gear.item)
            .unwrap()
            .equipment
            .as_ref()
            .unwrap()
            .tiers;
        let enchantment = gear.enchantment.as_ref().and_then(|e| world.enchantment(e));
        let catalyst = enchantment.into_iter().flat_map(|e| &e.catalyst);
        for stack in tiers[..gear.tier.min(tiers.len())]
            .iter()
            .flat_map(|t| &t.cost)
            .chain(catalyst)
        {
            add(&mut least, &stack.item, stack.quantity);
            add(&mut most, &stack.item, stack.quantity);
        }
    }
    (least, most)
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
    let effect_flags: BTreeSet<_> = progress
        .fired
        .iter()
        .filter_map(|e| match e {
            Effect::SetFlag { flag } => Some(flag.clone()),
            _ => None,
        })
        .collect();
    // An unconditional event that only sets flags cannot fail, so once its
    // first minute has come, its flags are set.
    let now = state.time.unwrap_or(0);
    let mut required = world
        .world
        .events
        .iter()
        .filter(|e| e.requires.is_none() && e.schedule.at <= now)
        .filter(|e| {
            e.effects
                .iter()
                .all(|f| matches!(f, Effect::SetFlag { .. }))
        })
        .flat_map(|e| &e.effects)
        .filter_map(|e| match e {
            Effect::SetFlag { flag } => Some(flag),
            _ => None,
        });
    ensure(
        required.all(|flag| state.flags.contains(flag)),
        "an event that has happened left no mark",
    )?;
    ensure(
        progress.quest_flags.is_subset(&state.flags)
            && state.flags.iter().all(|f| {
                progress.quest_flags.contains(f)
                    || effect_flags.contains(f)
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
