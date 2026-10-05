use crate::{
    input,
    render::Paint,
    render::{direction_name, duration, gear_name, money, piece_name, soldier, stat_name},
};
use realmkit_engine::{BattleOrder, Command, Engine};
use realmkit_spec::{CharacterKind, Resource, StartQuestion, Stat};
use std::io::{self, Write};

// Fixed interface words live here, apart from authored world text, so a locale
// table can replace them later without touching menu logic.
const TALK: &str = "Talk to";
const EXAMINE: &str = "Examine";
const ATTACK: &str = "Attack";
const TRAVEL: &str = "Travel";
const TRAVEL_TO: &str = "Travel to";
const WAIT: &str = "Wait";
const LOCKED: &str = "[locked]";
const ON: &str = "on";
const ENGAGE: &str = "Engage";
const MP: &str = "MP";
const RAGE: &str = "rage";
const REST: &str = "Rest";
const FLEE: &str = "Flee";
const TRAIN: &str = "Train";
const RESPEC: &str = "Refund stat points";
const TRAIN_GROUP: &str = "Train stats";
const PROFICIENCY_GROUP: &str = "Proficiencies";
const SOLD_OUT: &str = "[sold out]";
const EQUIPMENT_GROUP: &str = "Equipment";
const SMITHING_GROUP: &str = "Smithing";
const MARKET_GROUP: &str = "Market";
const CONSUME_GROUP: &str = "Use item";
const RECRUIT_GROUP: &str = "Recruit";
const UPGRADE_GROUP: &str = "Upgrade";
const RECRUIT: &str = "Recruit";
const LEFT: &str = "left";
const NONE_LEFT: &str = "[none left]";
const FULL: &str = "[retinue full]";
const WITH: &str = "with";
const RETINUE: &str = "Retinue";
const CHARGE: &str = "Charge";
const HOLD: &str = "Hold the line";
const FLANK: &str = "Flank with riders";
const RETREAT: &str = "Retreat";
const AUTORESOLVE: &str = "Autoresolve the rest";
const YOURS: &str = "Yours";
const THEIRS: &str = "Theirs";
const KNOCKED_OUT: &str = "knocked out";
const NOTHING_TO_RESTORE: &str = "[nothing to restore]";
const PRICES: &str = "Prices";
const BUY: &str = "Buy";
const SELL: &str = "Sell";
const NEXT_PRICE: &str = "next";
const AFFORD: &str = "[cannot afford]";
const ENCHANTING_GROUP: &str = "Enchanting";
const ENCHANT: &str = "Enchant";
const FORGE: &str = "Forge";
const IMPROVE: &str = "Improve";
const NEEDS: &str = "needs";
const OPENS: &str = "›";
const BACK: &str = "Back";
const EQUIP: &str = "Equip";
const YIELDED: &str = "yielded";
const INVENTORY: &str = "Inventory";
const CHARACTER: &str = "Character";
const QUESTS: &str = "Quests";
const TECHNIQUES: &str = "Techniques";
const MAP: &str = "Map";
const KEYS_HINT: &str = "↑/↓ select · Enter confirm · number choose · : command";
const ESC_HINT: &str = " · Esc back";
pub const LINE_HINT: &str = "Enter a number, or type help for commands.";
pub const NOT_LISTED: &str = "Choose one of the listed numbers.";
pub const CHOOSE_ANSWER: &str = "Choose one of the listed answers.";
pub const NOT_ON_MAP: &str = "That place is not on the map.";
pub const MAP_AFTER_FIGHT: &str = "The map can wait until the fight is over.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Up,
    Down,
    Left,
    Right,
    Tab,
    Enter,
    Esc,
    Backspace,
    Char(char),
    Quit,
    /// The terminal is now this many columns and rows.
    Resize(u16, u16),
}

#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    Ignore,
    Redraw,
    Run(Command),
    /// Step back from the conversation to the location's actions.
    Back,
    Typed,
    Help,
}

/// Actions gathered behind one entry, so the main menu stays short.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    Train,
    Proficiencies,
    Equipment,
    Smithing,
    Enchanting,
    Market,
    Consume,
    Recruit,
    Upgrade,
}

impl Group {
    fn of(command: &Command) -> Option<Self> {
        match command {
            Command::Allocate { .. } | Command::Respec => Some(Self::Train),
            Command::Train { .. } => Some(Self::Proficiencies),
            Command::Equip(_) => Some(Self::Equipment),
            Command::Forge(_) | Command::Improve(_) => Some(Self::Smithing),
            Command::Enchant { .. } => Some(Self::Enchanting),
            Command::Market | Command::Buy { .. } | Command::Sell { .. } => Some(Self::Market),
            Command::Use(_) => Some(Self::Consume),
            Command::Recruit { .. } => Some(Self::Recruit),
            Command::Upgrade { .. } => Some(Self::Upgrade),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pick {
    Run(Command),
    Open(Group),
    Back,
}

pub struct Entry {
    pub pick: Pick,
    pub label: String,
}

/// A menu keeps the entries it was built with, so numbers always refer to what
/// the player saw; the engine rechecks legality when a command runs.
pub struct Menu {
    top: Vec<Entry>,
    groups: Vec<(Group, Vec<Entry>)>,
    /// The submenu being shown, if any.
    pub open: Option<Group>,
    pub cursor: usize,
    dialogue: bool,
    /// In a fight: everyone's vitals and the projected turn order.
    header: Vec<String>,
    /// What the world offers, for letter shortcuts.
    context: input::Context,
}

/// The engine projects turns as if every action took a basic action's time.
const NEXT: &str = "Projected turns";
const TURNS_SHOWN: usize = 5;

/// "You HP 36/40 · rage 2 | The Ash Wolf HP 13/20" and the next few turns.
fn encounter_lines(engine: &Engine<'_>) -> Vec<String> {
    let Some(encounter) = engine.encounter() else {
        return Vec::new();
    };
    let world = engine.world();
    let name = |id: &str| world.character(id).unwrap().name.as_str();
    let resources = world.combat().unwrap().resources;
    let rage = resources.rage_per_action > 0 || resources.rage_per_max_hp > 0;
    let vitals: Vec<_> = encounter
        .participants
        .iter()
        .map(|p| {
            let max = match &world.character(&p.character).unwrap().combat {
                Some(profile) => profile.stats,
                None => engine.player_stats().unwrap(),
            };
            let mut line = format!("{} HP {}/{}", name(&p.character), p.hp, max.hp);
            if max.mp > 0 {
                line += &format!(" · {MP} {}/{}", p.mp, max.mp);
            }
            if rage {
                line += &format!(" · {RAGE} {}", p.rage);
            }
            if p.yielded {
                line += &format!(" · {YIELDED}");
            }
            line
        })
        .collect();
    let order: Vec<_> = engine
        .turn_order(TURNS_SHOWN)
        .iter()
        .map(|id| name(id))
        .collect();
    vec![vitals.join(" | "), format!("{NEXT}: {}", order.join(", "))]
}

/// "Yours: You HP 40/40 · Levy 6 | Theirs: Outlaw 8 · Poacher 4": who still
/// stands on each side of a battle.
fn battle_lines(engine: &Engine<'_>) -> Vec<String> {
    let Some(battle) = engine.battle() else {
        return Vec::new();
    };
    let world = engine.world();
    let player = &world.character(&world.world.player).unwrap().name;
    let max = engine.player_stats().unwrap().hp;
    let you = if battle.hp > 0 {
        format!("{player} HP {}/{max}", battle.hp)
    } else {
        format!("{player} {KNOCKED_OUT}")
    };
    let stacks = |side: usize| -> Vec<String> {
        battle.sides[side]
            .stacks
            .iter()
            .filter(|s| s.count > 0)
            .map(|s| format!("{} {}", soldier(world, &s.line, s.level), s.count))
            .collect()
    };
    let ours = std::iter::once(you).chain(stacks(0)).collect::<Vec<_>>();
    vec![format!(
        "{YOURS}: {} | {THEIRS}: {}",
        ours.join(" · "),
        stacks(1).join(" · ")
    )]
}

/// The location's or fight's actions: grouped ones go into submenus, each
/// entered where its first action would have been.
fn actions(engine: &Engine<'_>) -> (Vec<Entry>, Vec<(Group, Vec<Entry>)>) {
    let mut top = Vec::new();
    let mut groups: Vec<(Group, Vec<Entry>)> = Vec::new();
    for action in engine.actions() {
        let Some(label) = label(engine, &action) else {
            continue;
        };
        let entry = Entry {
            pick: Pick::Run(action.command.clone()),
            label,
        };
        let Some(group) = Group::of(&action.command) else {
            top.push(entry);
            continue;
        };
        match groups.iter_mut().find(|(g, _)| *g == group) {
            Some((_, entries)) => entries.push(entry),
            None => {
                top.push(Entry {
                    pick: Pick::Open(group),
                    label: group_label(engine, group),
                });
                groups.push((group, vec![entry]));
            }
        }
    }
    for (_, entries) in &mut groups {
        entries.push(Entry {
            pick: Pick::Back,
            label: BACK.into(),
        });
    }
    (top, groups)
}

/// "Train stats — 3 points ›", "Equipment ›".
fn group_label(engine: &Engine<'_>, group: Group) -> String {
    match group {
        Group::Train => match engine.unspent_points().unwrap_or(0) {
            0 => format!("{TRAIN_GROUP} {OPENS}"),
            1 => format!("{TRAIN_GROUP} — 1 point {OPENS}"),
            n => format!("{TRAIN_GROUP} — {n} points {OPENS}"),
        },
        Group::Proficiencies => match engine.unspent_proficiency_points() {
            1 => format!("{PROFICIENCY_GROUP} — 1 point {OPENS}"),
            n => format!("{PROFICIENCY_GROUP} — {n} points {OPENS}"),
        },
        Group::Equipment => format!("{EQUIPMENT_GROUP} {OPENS}"),
        Group::Smithing => format!("{SMITHING_GROUP} {OPENS}"),
        Group::Enchanting => format!("{ENCHANTING_GROUP} {OPENS}"),
        Group::Market => format!("{MARKET_GROUP} {OPENS}"),
        Group::Consume => format!("{CONSUME_GROUP} {OPENS}"),
        Group::Recruit => format!("{RECRUIT_GROUP} {OPENS}"),
        Group::Upgrade => format!("{UPGRADE_GROUP} {OPENS}"),
    }
}

/// What the player sees for an action; `None` hides it.
fn label(engine: &Engine<'_>, action: &realmkit_engine::Action) -> Option<String> {
    let world = engine.world();
    let here = world.location(&engine.state().player.location).unwrap();
    Some(match &action.command {
        Command::Move(direction) => format!(
            "{TRAVEL} {} — {}{}",
            direction_name(*direction),
            world
                .location(&here.exits[direction].destination)
                .unwrap()
                .name,
            if action.available {
                String::new()
            } else {
                format!(" {LOCKED}")
            }
        ),
        // "Travel to Ashmere — 2 h", or "[locked]" like an exit.
        Command::Travel(to) => {
            let road = world.road(&here.id, to)?;
            format!(
                "{TRAVEL_TO} {}{}{}",
                world.location(to)?.name,
                if road.minutes > 0 {
                    format!(" — {}", duration(road.minutes))
                } else {
                    String::new()
                },
                if action.available {
                    String::new()
                } else {
                    format!(" {LOCKED}")
                }
            )
        }
        // Like a locked exit, choosing it explains why it cannot be used.
        Command::UseSkill { skill, target } => {
            let skill = world.skill(skill).unwrap();
            // A technique's skill carries the name of the rank it belongs to.
            let rank = engine.state().combat.as_ref().and_then(|c| {
                c.techniques.iter().find_map(|(id, learned)| {
                    let rank = &world.technique(id)?.ranks[learned.rank - 1];
                    (rank.skill.as_ref() == Some(&skill.id)).then_some(&rank.name)
                })
            });
            let name = match rank {
                Some(rank) => format!("{} · {rank}", skill.name),
                None => skill.name.clone(),
            };
            let resource = match skill.resource {
                Resource::Mp => MP,
                Resource::Rage => RAGE,
            };
            format!(
                "{} {ON} {}{}{}",
                name,
                world.character(target).unwrap().name,
                if skill.cost > 0 {
                    format!(" — {} {resource}", skill.cost)
                } else {
                    String::new()
                },
                if action.available {
                    String::new()
                } else {
                    format!(" [not enough {resource}]")
                }
            )
        }
        // Other unavailable actions (e.g. after death) are not offered.
        // "Forge Iron mail — 3 Iron ingot [needs Journeyman Smithing]".
        Command::Forge(id) => {
            let recipe = world.recipe(id).unwrap();
            let output = &world.item(&recipe.output).unwrap().name;
            let cost = materials(world, &recipe.inputs);
            let why = missing(engine, recipe.requires.as_ref(), &recipe.inputs);
            format!("{FORGE} {output} — {cost}{why}")
        }
        // "Improve #1 Iron sword → Fine Iron sword (Attack +4 → +6) — 1 Iron
        // ingot": what the piece itself gains, shown before it is affordable.
        Command::Improve(piece) => {
            let gear = &engine.state().combat.as_ref()?.gear[piece];
            let equipment = world.item(&gear.item)?.equipment.as_ref()?;
            let tier = equipment.tiers.get(gear.tier)?;
            let mut next = gear.clone();
            next.tier += 1;
            let changes = tier_changes(world, equipment, gear.tier);
            let cost = materials(world, &tier.cost);
            let why = missing(engine, tier.requires.as_ref(), &tier.cost);
            format!(
                "{IMPROVE} {} → {}{changes} — {cost}{why}",
                gear_name(engine, *piece),
                piece_name(world, &next)
            )
        }
        // "Enchant #1 Iron sword → Iron sword of Keenness (Attack +2) — 1 Ember shard".
        Command::Enchant { piece, enchantment } => {
            let gear = &engine.state().combat.as_ref()?.gear[piece];
            let enchantment = world.enchantment(enchantment)?;
            let mut next = gear.clone();
            next.enchantment = Some(enchantment.id.clone());
            let bonuses: Vec<String> = Stat::ALL
                .into_iter()
                .filter_map(|s| {
                    // A zero bonus changes nothing, so it is not shown.
                    let bonus = enchantment.bonuses.get(&s).filter(|b| **b > 0)?;
                    Some(format!("{} +{bonus}", stat_name(world, s)))
                })
                .collect();
            let cost = materials(world, &enchantment.catalyst);
            let why = missing(engine, enchantment.requires.as_ref(), &enchantment.catalyst);
            format!(
                "{ENCHANT} {} → {} ({}) — {cost}{why}",
                gear_name(engine, *piece),
                piece_name(world, &next),
                bonuses.join(", ")
            )
        }
        // "Recruit Levy — 10 silver (8 left)", or why not now.
        Command::Recruit { line, .. } => {
            let offer = here
                .recruits
                .as_ref()?
                .troops
                .iter()
                .find(|o| &o.line == line)?;
            let state = engine.state();
            let retinue = state.retinue.as_ref()?;
            let left = retinue.pools.get(&here.id)?.get(line).copied()?;
            let heads = retinue.heads();
            let why = if action.available {
                String::new()
            } else if left == 0 {
                format!(" {NONE_LEFT}")
            } else if heads >= world.troops()?.limit {
                format!(" {FULL}")
            } else {
                format!(" {AFFORD}")
            };
            format!(
                "{RECRUIT} {} — {} ({left} {LEFT}){why}",
                soldier(world, line, 1),
                money(world, offer.price)
            )
        }
        // "Spearman → Bowman — 20 silver".
        Command::Upgrade { line, to, .. } => {
            let from = world.troops()?.line(line)?;
            let cost = from.upgrades.iter().find(|u| &u.to == to)?.cost;
            let why = if action.available {
                String::new()
            } else {
                format!(" {AFFORD}")
            };
            format!(
                "{} → {} — {}{why}",
                soldier(world, line, from.levels.len()),
                soldier(world, to, 1),
                money(world, cost)
            )
        }
        Command::Retinue => RETINUE.into(),
        Command::Order(order) => match order {
            BattleOrder::Charge => CHARGE,
            BattleOrder::Hold => HOLD,
            BattleOrder::Flank => FLANK,
            BattleOrder::Retreat => RETREAT,
        }
        .into(),
        Command::Autoresolve => AUTORESOLVE.into(),
        // "Healing draught ×2 — +30 HP", or why it would do nothing now.
        Command::Use(item) => {
            let consumable = world.item(item)?.consumable?;
            let held = engine
                .state()
                .player
                .inventory
                .get(item)
                .copied()
                .unwrap_or(0);
            let mut gains = Vec::new();
            if consumable.hp > 0 {
                gains.push(format!("+{} HP", consumable.hp));
            }
            if consumable.mp > 0 {
                gains.push(format!("+{} {MP}", consumable.mp));
            }
            let why = if action.available {
                String::new()
            } else {
                format!(" {NOTHING_TO_RESTORE}")
            };
            format!(
                "{} ×{held} — {}{why}",
                world.item(item)?.name,
                gains.join(", ")
            )
        }
        // "Buy Cloth — 129 silver (next 133)": this unit's price and the
        // next one's after the index moves; unaffordable goods stay listed.
        Command::Buy { good, .. } => {
            let name = &world.item(good)?.name;
            // A ware: one fixed price, so no "next".
            if let Some(price) = engine.ware_price(good) {
                let why = if action.available {
                    String::new()
                } else {
                    format!(" {AFFORD}")
                };
                return Some(format!("{BUY} {name} — {}{why}", money(world, price)));
            }
            let quote = engine.quote(good)?;
            if !action.available {
                let currency = engine.state().economy.as_ref()?.currency;
                let why = if quote.stock == Some(0) {
                    SOLD_OUT
                } else if currency < quote.buy {
                    AFFORD
                } else {
                    LOCKED
                };
                return Some(format!("{BUY} {name} — {} {why}", money(world, quote.buy)));
            }
            format!(
                "{BUY} {name} — {} ({NEXT_PRICE} {})",
                money(world, quote.buy),
                money(world, quote.next_buy)
            )
        }
        Command::Sell { .. } if !action.available => return None,
        Command::Sell { good, .. } => {
            let quote = engine.quote(good)?;
            let held = engine
                .state()
                .player
                .inventory
                .get(good)
                .copied()
                .unwrap_or(0);
            format!(
                "{SELL} {} ({held}) — {} ({NEXT_PRICE} {})",
                world.item(good)?.name,
                money(world, quote.sell),
                money(world, quote.next_sell)
            )
        }
        Command::Market => PRICES.into(),
        _ if !action.available => return None,
        Command::Rest => REST.into(),
        Command::Wait(minutes) => format!("{WAIT} — {}", duration(*minutes)),
        Command::Flee => FLEE.into(),
        // "Train Attack: 12 → 13", from the same effective stats the engine uses.
        Command::Allocate { stat, .. } => {
            let now = engine.player_stats().unwrap().get(*stat);
            let value = world.combat().unwrap().stat_points.as_ref().unwrap().values[stat];
            format!(
                "{TRAIN} {}: {now} → {}",
                stat_name(world, *stat),
                now + value
            )
        }
        Command::Respec => RESPEC.into(),
        // "Train Trade: 0 → 1"
        Command::Train { proficiency, .. } => {
            let rank = engine.proficiency_rank(*proficiency);
            format!(
                "{TRAIN} {}: {rank} → {}",
                world.proficiency_name(*proficiency)?,
                rank + 1
            )
        }
        // "Equip #5 Greatsword: Attack 13 → 21, Defence 11 → 9", from
        // the engine's own calculation on a copy.
        Command::Equip(piece) => {
            let mut probe = engine.clone();
            probe.execute(Command::Equip(*piece)).ok()?;
            let changes = stat_changes(engine, &probe);
            format!("{EQUIP} {}{changes}", gear_name(engine, *piece))
        }
        // An army names its size and who will stand with the player.
        Command::Engage(id) => {
            let character = world.character(id).unwrap();
            let Some(army) = &character.army else {
                return Some(format!("{ENGAGE} {}", character.name));
            };
            let size: u64 = army.troops.iter().map(|s| s.count).sum();
            let mut label = format!("{ENGAGE} {} ({size})", character.name);
            let allies: Vec<_> = engine
                .present_here()
                .into_iter()
                .filter(|c| {
                    c.army
                        .as_ref()
                        .and_then(|a| a.joins.as_ref())
                        .is_some_and(|j| engine.holds(j))
                })
                .map(|c| c.name.as_str())
                .collect();
            if !allies.is_empty() {
                label += &format!(" — {WITH} {}", allies.join(", "));
            }
            label
        }
        Command::Talk(id) => {
            let character = world.character(id).unwrap();
            let verb = match character.kind {
                CharacterKind::Person => TALK,
                CharacterKind::Feature => EXAMINE,
            };
            format!("{verb} {}", character.name)
        }
        Command::Attack(id) => {
            format!("{ATTACK} {}", world.character(id).unwrap().name)
        }
        Command::Inventory => INVENTORY.into(),
        Command::Status => CHARACTER.into(),
        Command::Quests => QUESTS.into(),
        Command::Techniques => TECHNIQUES.into(),
        Command::Map => MAP.into(),
        _ => return None,
    })
}

/// "2 Iron ingot, 1 Leather strip".
fn materials(world: &realmkit_spec::WorldSpec, stacks: &[realmkit_spec::ItemStack]) -> String {
    let parts: Vec<String> = stacks
        .iter()
        .map(|s| format!("{} {}", s.quantity, world.item(&s.item).unwrap().name))
        .collect();
    parts.join(", ")
}

/// Why crafting cannot happen yet, in the order the engine checks:
/// " [needs Journeyman Smithing]", " [needs 2 Iron ingot]", or nothing.
fn missing(
    engine: &Engine<'_>,
    requires: Option<&realmkit_spec::Condition>,
    stacks: &[realmkit_spec::ItemStack],
) -> String {
    let world = engine.world();
    if let Some(condition) = requires.filter(|c| !engine.holds(c)) {
        return match unmet(engine, condition) {
            Some(realmkit_spec::Condition::Technique { technique, rank }) => {
                let technique = world.technique(technique).unwrap();
                let rank = &technique.ranks[rank - 1].name;
                format!(" [{NEEDS} {rank} {}]", technique.name)
            }
            _ => format!(" {LOCKED}"),
        };
    }
    let inventory = &engine.state().player.inventory;
    let short = stacks
        .iter()
        .find(|s| inventory.get(&s.item).copied().unwrap_or(0) < s.quantity);
    match short {
        Some(s) => format!(
            " [{NEEDS} {} {}]",
            s.quantity,
            world.item(&s.item).unwrap().name
        ),
        None => String::new(),
    }
}

/// The first unmet leaf that a failed condition needs, if it can be named:
/// through `all` only, since `any`, `at_least` and `not` have no single reason.
fn unmet<'c>(
    engine: &Engine<'_>,
    condition: &'c realmkit_spec::Condition,
) -> Option<&'c realmkit_spec::Condition> {
    use realmkit_spec::Condition;
    match condition {
        Condition::All { of } => unmet(engine, of.iter().find(|c| !engine.holds(c))?),
        Condition::Any { .. } | Condition::AtLeast { .. } | Condition::Not { .. } => None,
        leaf => Some(leaf),
    }
}

/// " (Attack +4 → +6, Speed −10 → −5)": how the piece changes a tier up.
fn tier_changes(
    world: &realmkit_spec::WorldSpec,
    equipment: &realmkit_spec::Equipment,
    tier: usize,
) -> String {
    let bonus = |t: usize, stat| equipment.bonuses_at(t).get(&stat).copied().unwrap_or(0);
    let mut changes: Vec<String> = Stat::ALL
        .into_iter()
        .filter(|s| bonus(tier, *s) != bonus(tier + 1, *s))
        .map(|s| {
            format!(
                "{} +{} → +{}",
                stat_name(world, s),
                bonus(tier, s),
                bonus(tier + 1, s)
            )
        })
        .collect();
    let (before, after) = (
        equipment.speed_penalty_at(tier),
        equipment.speed_penalty_at(tier + 1),
    );
    if before != after {
        changes.push(format!(
            "{} −{before} → −{after}",
            stat_name(world, Stat::Speed)
        ));
    }
    if changes.is_empty() {
        String::new()
    } else {
        format!(" ({})", changes.join(", "))
    }
}

/// ": Attack 15 → 17, Speed 90 → 95" between two engines' effective stats.
fn stat_changes(before: &Engine<'_>, after: &Engine<'_>) -> String {
    let (now, then) = (
        before.player_stats().unwrap(),
        after.player_stats().unwrap(),
    );
    let changes: Vec<String> = Stat::ALL
        .into_iter()
        .filter(|s| now.get(*s) != then.get(*s))
        .map(|s| {
            format!(
                "{} {} → {}",
                stat_name(before.world(), s),
                now.get(s),
                then.get(s)
            )
        })
        .collect();
    if changes.is_empty() {
        String::new()
    } else {
        format!(": {}", changes.join(", "))
    }
}

impl Menu {
    /// Dialogue choices take focus unless the player stepped back with Esc.
    /// `open` keeps a submenu open across commands while it has entries.
    pub fn new(engine: &Engine<'_>, leave_dialogue: bool, open: Option<Group>) -> Self {
        let choices = engine.dialogue_choices();
        let dialogue = !leave_dialogue && !choices.is_empty();
        let (top, groups) = if dialogue {
            let top = choices
                .into_iter()
                .enumerate()
                .map(|(i, text)| Entry {
                    pick: Pick::Run(Command::ChooseDialogue(i + 1)),
                    label: text.into(),
                })
                .collect();
            (top, Vec::new())
        } else {
            actions(engine)
        };
        let open = open.filter(|g| groups.iter().any(|(group, _)| group == g));
        Self {
            top,
            groups,
            open,
            cursor: 0,
            dialogue,
            header: if engine.battle().is_some() {
                battle_lines(engine)
            } else {
                encounter_lines(engine)
            },
            context: input::Context::of(engine.world()),
        }
    }

    /// A start question at New Game, its options numbered in order. Choosing
    /// one runs `ChooseDialogue(n)`; the caller takes it as the answer.
    pub fn question(question: &StartQuestion) -> Self {
        let top = question
            .options
            .iter()
            .enumerate()
            .map(|(i, option)| Entry {
                pick: Pick::Run(Command::ChooseDialogue(i + 1)),
                label: option.text.clone(),
            })
            .collect();
        Self {
            top,
            groups: Vec::new(),
            open: None,
            cursor: 0,
            dialogue: false,
            header: vec![question.text.clone()],
            context: input::Context::default(),
        }
    }

    /// The submenu to reopen after `command`: the open one, if the command came from it.
    pub fn stays_open(&self, command: &Command) -> Option<Group> {
        self.open.filter(|g| Group::of(command) == Some(*g))
    }

    /// The entries on screen: the open submenu's, or the main menu's.
    pub fn entries(&self) -> &[Entry] {
        self.groups
            .iter()
            .find(|(group, _)| Some(*group) == self.open)
            .map_or(&self.top, |(_, entries)| entries)
    }

    /// Drops the fight's vitals and turn order, for a screen that shows its own.
    pub fn without_header(mut self) -> Self {
        self.header.clear();
        self
    }

    /// One-based selection, as shown to the player.
    pub fn select(&self, number: usize) -> Option<&Entry> {
        number.checked_sub(1).and_then(|i| self.entries().get(i))
    }

    /// Runs a command, or opens or leaves a submenu.
    pub fn choose(&mut self, number: usize) -> Outcome {
        let Some(entry) = self.select(number) else {
            return Outcome::Ignore;
        };
        match entry.pick.clone() {
            Pick::Run(command) => return Outcome::Run(command),
            Pick::Open(group) => self.open = Some(group),
            Pick::Back => self.open = None,
        }
        self.cursor = 0;
        Outcome::Redraw
    }

    /// Writes the menu and returns how many lines it occupies.
    // ponytail: assumes labels fit one terminal row; measure widths if long labels wrap.
    pub fn write(
        &self,
        output: &mut impl Write,
        interactive: bool,
        paint: Paint,
    ) -> io::Result<u16> {
        writeln!(output)?;
        let mut header = self.header.clone();
        // A submenu is headed by the entry that opened it: "Train stats — 1 point".
        let opener = self
            .top
            .iter()
            .find(|e| Some(&e.pick) == self.open.map(Pick::Open).as_ref());
        if let Some(entry) = opener {
            let title = entry.label.trim_end_matches(OPENS).trim_end();
            header.push(paint.title(title));
        }
        for line in &header {
            writeln!(output, "{line}")?;
        }
        for (i, entry) in self.entries().iter().enumerate() {
            let line = format!("{}. {}", i + 1, entry.label);
            if interactive && i == self.cursor {
                writeln!(output, "> {}", paint.title(&line))?;
            } else {
                writeln!(output, "  {line}")?;
            }
        }
        if interactive {
            let esc = if self.dialogue || self.open.is_some() {
                ESC_HINT
            } else {
                ""
            };
            writeln!(output, "\n{}", paint.dim(&format!("{KEYS_HINT}{esc}")))?;
            Ok((self.entries().len() + header.len()) as u16 + 3)
        } else {
            Ok((self.entries().len() + header.len()) as u16 + 1)
        }
    }

    pub fn handle(&mut self, key: Key) -> Outcome {
        let last = self.entries().len().saturating_sub(1);
        match key {
            Key::Up => {
                self.cursor = if self.cursor == 0 {
                    last
                } else {
                    self.cursor - 1
                }
            }
            Key::Down => {
                self.cursor = if self.cursor == last {
                    0
                } else {
                    self.cursor + 1
                }
            }
            Key::Enter => return self.choose(self.cursor + 1),
            Key::Esc if self.open.is_some() => {
                self.open = None;
                self.cursor = 0;
            }
            Key::Esc if self.dialogue => return Outcome::Back,
            Key::Char(':') => return Outcome::Typed,
            // ponytail: digits 1–9 choose instantly; longer menus need arrows or `:`.
            Key::Char(c @ '1'..='9') => return self.choose(c as usize - '0' as usize),
            // Letter shortcuts (movement, panels) apply only outside dialogue focus.
            Key::Char(c) if !self.dialogue => {
                return match input::shortcut(c, self.context) {
                    Ok(input::Input::Command(command)) => Outcome::Run(command),
                    Ok(input::Input::Help) => Outcome::Help,
                    _ => Outcome::Ignore,
                }
            }
            _ => return Outcome::Ignore,
        }
        Outcome::Redraw
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use realmkit_spec::{Direction, WorldSpec};

    fn demo() -> WorldSpec {
        WorldSpec::load(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../examples/demo-world"
        ))
        .unwrap()
    }

    #[test]
    fn an_exit_to_an_unknown_place_is_neither_offered_nor_listed() {
        let mut world = demo();
        for (i, l) in world.locations.iter_mut().enumerate() {
            l.map = Some(realmkit_spec::MapPoint {
                x: i as u32,
                y: 0,
                kind: realmkit_spec::PlaceKind::Waypoint,
            });
        }
        // The chapel is unheard of until its gate is opened.
        world.locations[2].known_when = Some(realmkit_spec::Condition::Flag {
            flag: "ruins_open".into(),
        });
        let mut engine = Engine::new(&world).unwrap();
        let menu = Menu::new(&engine, false, None);
        let labels: Vec<_> = menu.entries().iter().map(|e| e.label.clone()).collect();
        assert!(labels.contains(&"Travel north — The Pine Track".to_string()));
        assert!(!labels.iter().any(|l| l.contains("Chapel")), "{labels:?}");
        let mut panel = Vec::new();
        crate::panels::location(
            &mut panel,
            &engine,
            "village",
            crate::render::Paint::default(),
        )
        .unwrap();
        let panel = String::from_utf8(panel).unwrap();
        assert!(panel.contains("Exits: north\n"), "{panel}");
        assert!(matches!(
            engine.execute(Command::Move(Direction::East)),
            Err(realmkit_engine::EngineError::NoExit)
        ));
    }

    #[test]
    fn labels_use_authored_names_and_mark_locked_exits() {
        let world = demo();
        let engine = Engine::new(&world).unwrap();
        let menu = Menu::new(&engine, false, None);
        let labels: Vec<_> = menu.entries().iter().map(|e| e.label.as_str()).collect();
        assert_eq!(
            labels,
            [
                "Talk to Elder Mara",
                "Travel north — The Pine Track",
                "Travel east — The Roofless Chapel [locked]",
                "Rest",
                "Inventory",
                "Character",
                "Quests"
            ]
        );
    }

    #[test]
    fn keys_move_the_cursor_choose_and_leave_dialogue() {
        let world = demo();
        let mut engine = Engine::new(&world).unwrap();
        let mut menu = Menu::new(&engine, false, None);
        assert_eq!(menu.handle(Key::Up), Outcome::Redraw);
        assert_eq!(menu.cursor, 6);
        assert_eq!(menu.handle(Key::Down), Outcome::Redraw);
        assert_eq!(menu.handle(Key::Down), Outcome::Redraw);
        assert_eq!(
            menu.handle(Key::Enter),
            Outcome::Run(Command::Move(Direction::North))
        );
        assert_eq!(menu.handle(Key::Char('9')), Outcome::Ignore);
        assert_eq!(menu.handle(Key::Esc), Outcome::Ignore);
        assert_eq!(menu.handle(Key::Char(':')), Outcome::Typed);
        assert_eq!(
            menu.handle(Key::Char('n')),
            Outcome::Run(Command::Move(Direction::North))
        );
        assert_eq!(menu.handle(Key::Char('c')), Outcome::Run(Command::Status));

        engine.execute(Command::Talk("elder".into())).unwrap();
        let mut menu = Menu::new(&engine, false, None);
        assert_eq!(menu.entries()[0].label, "What troubles the village?");
        assert_eq!(menu.handle(Key::Char('n')), Outcome::Ignore);
        assert_eq!(
            menu.handle(Key::Char('2')),
            Outcome::Run(Command::ChooseDialogue(2))
        );
        assert_eq!(menu.handle(Key::Esc), Outcome::Back);
        assert_eq!(
            Menu::new(&engine, true, None).entries()[0].label,
            "Talk to Elder Mara"
        );
    }

    #[test]
    fn proficiencies_wait_behind_their_own_submenu() {
        let world = WorldSpec::load(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../examples/marches"
        ))
        .unwrap();
        let mut engine = Engine::new(&world).unwrap();
        let mut menu = Menu::new(&engine, false, None);
        let labels = |menu: &Menu| -> Vec<String> {
            menu.entries().iter().map(|e| e.label.clone()).collect()
        };
        assert_eq!(labels(&menu)[5], "Proficiencies — 1 point ›");
        menu.choose(6);
        assert_eq!(labels(&menu), ["Train Trade: 0 → 1", "Back"]);
        let Outcome::Run(train) = menu.handle(Key::Enter) else {
            panic!("the first entry trains trading");
        };
        engine.execute(train).unwrap();
        // With no points left, the group is gone.
        let menu = Menu::new(&engine, false, None);
        assert!(!labels(&menu).iter().any(|l| l.starts_with("Proficiencies")));
    }

    #[test]
    fn training_waits_behind_a_submenu_that_stays_open_while_it_is_used() {
        let world =
            WorldSpec::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/arena")).unwrap();
        let mut engine = Engine::new(&world).unwrap();
        let mut menu = Menu::new(&engine, false, None);
        let labels = |menu: &Menu| -> Vec<String> {
            menu.entries().iter().map(|e| e.label.clone()).collect()
        };
        assert_eq!(
            labels(&menu)[4..],
            [
                "Rest",
                "Train stats — 3 points ›",
                "Market ›",
                "Inventory",
                "Character",
                "Quests"
            ]
        );
        assert_eq!(menu.handle(Key::Char('6')), Outcome::Redraw);
        assert_eq!(
            labels(&menu),
            [
                "Train HP: 60 → 65",
                "Train Attack: 14 → 15",
                "Train Defence: 12 → 13",
                "Train Speed: 100 → 102",
                "Back"
            ]
        );
        // Esc and Back both return to the main menu.
        assert_eq!(menu.handle(Key::Esc), Outcome::Redraw);
        assert_eq!(menu.open, None);
        menu.choose(6);
        assert_eq!(menu.choose(5), Outcome::Redraw);
        assert_eq!(menu.open, None);

        menu.choose(6);
        let Outcome::Run(train) = menu.handle(Key::Enter) else {
            panic!("the first entry trains HP");
        };
        engine.execute(train.clone()).unwrap();
        let menu = Menu::new(&engine, false, menu.stays_open(&train));
        assert_eq!(labels(&menu)[0], "Train HP: 65 → 70");
        // A command from elsewhere closes it.
        assert_eq!(menu.stays_open(&Command::Rest), None);
    }

    #[test]
    fn an_improvement_previews_the_piece_even_before_it_is_affordable() {
        let world = WorldSpec::load(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../examples/smithy"
        ))
        .unwrap();
        let mut engine = Engine::new(&world).unwrap();
        for command in [
            Command::Talk("bran".into()),
            Command::ChooseDialogue(1),
            Command::ChooseDialogue(1),
            Command::Move(Direction::Down),
        ] {
            engine.execute(command).unwrap();
        }
        for _ in 0..2 {
            engine.execute(Command::Engage("beetle".into())).unwrap();
            while engine.encounter().is_some() {
                engine.execute(Command::Attack("beetle".into())).unwrap();
            }
        }
        engine.execute(Command::Move(Direction::Up)).unwrap();
        engine.execute(Command::Move(Direction::East)).unwrap();
        engine.execute(Command::Forge("iron_sword".into())).unwrap();
        let mut menu = Menu::new(&engine, false, None);
        let smithing = menu
            .entries()
            .iter()
            .position(|e| e.label == "Smithing ›")
            .unwrap();
        menu.choose(smithing + 1);
        let labels: Vec<_> = menu.entries().iter().map(|e| e.label.as_str()).collect();
        // The piece's own change, shown although an Apprentice with no ingot
        // cannot make it yet.
        assert!(labels.contains(
            &"Improve #1 Iron sword → Fine Iron sword (Attack +4 → +6) — 1 Iron ingot [needs Journeyman Smithing]"
        ), "{labels:?}");
    }

    #[test]
    fn consumables_and_wares_are_labelled_with_what_they_do_and_cost() {
        let world =
            WorldSpec::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/arena")).unwrap();
        let mut engine = Engine::new(&world).unwrap();
        let open = |engine: &Engine<'_>, group: &str| -> Vec<String> {
            let mut menu = Menu::new(engine, false, None);
            let at = menu
                .entries()
                .iter()
                .position(|e| e.label == group)
                .unwrap_or_else(|| panic!("no {group}"));
            menu.choose(at + 1);
            menu.entries().iter().map(|e| e.label.clone()).collect()
        };
        let market = open(&engine, "Market ›");
        assert!(
            market.contains(&"Buy Healing draught — 8 marks".into()),
            "{market:?}"
        );
        assert!(
            market.contains(&"Buy Iron mail — 60 marks [cannot afford]".into()),
            "{market:?}"
        );
        engine
            .execute(Command::Buy {
                good: "healing_draught".into(),
                quantity: 1,
            })
            .unwrap();
        // At full health a draught would do nothing, and says so.
        let usable = open(&engine, "Use item ›");
        assert_eq!(
            usable,
            ["Healing draught ×1 — +30 HP [nothing to restore]", "Back"]
        );
    }

    #[test]
    fn soldiers_are_recruited_and_upgraded_from_submenus() {
        let world = WorldSpec::load(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../examples/marches"
        ))
        .unwrap();
        let mut engine = Engine::new(&world).unwrap();
        engine.execute(Command::Travel("ashmere".into())).unwrap();
        let open = |engine: &Engine<'_>, group: &str| -> Vec<String> {
            let mut menu = Menu::new(engine, false, None);
            let at = menu
                .entries()
                .iter()
                .position(|e| e.label == group)
                .unwrap_or_else(|| panic!("no {group}"));
            menu.choose(at + 1);
            menu.entries().iter().map(|e| e.label.clone()).collect()
        };
        assert_eq!(
            open(&engine, "Recruit ›"),
            ["Recruit Levy — 10 silver (8 left)", "Back"]
        );
        engine
            .execute(Command::Recruit {
                line: "levy".into(),
                quantity: 8,
            })
            .unwrap();
        assert_eq!(
            open(&engine, "Recruit ›"),
            ["Recruit Levy — 10 silver (0 left) [none left]", "Back"]
        );
        // Spearmen can turn bowmen or, for more than is left, riders.
        let mut snapshot = engine.snapshot();
        let roster = &mut snapshot.state.retinue.as_mut().unwrap().roster;
        let levies = roster.remove("levy").unwrap()[&1];
        roster.entry("levy".into()).or_default().insert(3, levies);
        let engine = Engine::restore(&world, snapshot).unwrap();
        assert_eq!(
            open(&engine, "Upgrade ›"),
            [
                "Spearman → Bowman — 20 silver",
                "Spearman → Rider — 40 silver [cannot afford]",
                "Back"
            ]
        );
    }

    #[test]
    fn an_enchantment_label_leaves_out_zero_bonuses() {
        let mut world = WorldSpec::load(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../examples/smithy"
        ))
        .unwrap();
        let combat = world.world.combat.as_mut().unwrap();
        combat.enchantments[0].bonuses.insert(Stat::Pdef, 0);
        let mut engine = Engine::new(&world).unwrap();
        let steps = [
            Command::Talk("bran".into()),
            Command::ChooseDialogue(1),
            Command::ChooseDialogue(1),
            Command::Move(Direction::Down),
        ];
        for command in steps {
            engine.execute(command).unwrap();
        }
        for _ in 0..2 {
            engine.execute(Command::Engage("beetle".into())).unwrap();
            while engine.encounter().is_some() {
                engine.execute(Command::Attack("beetle".into())).unwrap();
            }
        }
        for command in [
            Command::Move(Direction::Up),
            Command::Move(Direction::East),
            Command::Forge("iron_sword".into()),
            Command::Move(Direction::West),
            Command::Move(Direction::North),
            Command::Talk("maud".into()),
            Command::ChooseDialogue(1),
            Command::ChooseDialogue(1),
        ] {
            engine.execute(command).unwrap();
        }
        let mut menu = Menu::new(&engine, false, None);
        let enchanting = menu
            .entries()
            .iter()
            .position(|e| e.label == "Enchanting ›")
            .unwrap();
        menu.choose(enchanting + 1);
        let label = &menu.entries()[0].label;
        assert!(label.contains("(Attack +2)"), "{label}");
    }
}
