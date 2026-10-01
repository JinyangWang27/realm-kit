# Consumables and Wares Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Items that restore HP/MP when used (exploring or as a fight turn),
and markets that sell items at fixed prices, as package and save Format 13.

**Architecture:** Two optional content fields (`Item.consumable` and
`Market.wares`) are validated in the spec crate. They add one new command
(`Use`) and extend `Buy` in the engine, with no new saved state; the save
check only learns which item counts may now move. The CLI adds a `Use item ›`
submenu, ware entries in `Market ›` and a wares section in the market panel.
`examples/arena` demonstrates both.

**Tech Stack:** Rust workspace (`realmkit-spec`, `realmkit-engine`,
`realmkit-cli`), serde/serde_json, crossterm in the CLI only.

**Spec:** `docs/superpowers/specs/2026-10-01-consumables-and-wares-design.md`

**Deviations from the spec, decided while planning:**

- One event, `Event::Consumed { item, hp, mp }`, replaces the spec's
  `ItemUsed` and `Restored` pair.
- Missing units reuse `EngineError::NotEnoughMaterials`; there is no new
  `NotCarried`.
- The fixture is `examples/arena`, not marches: marches has no fighters, so
  nothing there could be healed. PR 2 adds combat to marches.
- Equipment wares in a world without combat need no new check: equipment
  items are already rejected there (`combat_disabled`).

Task 1 updates the spec file to match.

## Global Constraints

- `FORMAT_VERSION` and `SAVE_FORMAT_VERSION` both become `13`. Every
  `examples/*/world.json` gets `"format_version": 13`. Older packages and
  saves are rejected, never migrated.
- Determinism: no wall clock or I/O in the engine. Refused commands change
  nothing (`Engine::execute` works on a clone) and draw no randomness.
- Checked arithmetic. Return `EngineError::NumericLimit`; never wrap.
- Every validation check pushes a `Diagnostic` with a stable `code`, and
  tests assert on codes.
- Player-facing text comes from the world package. Fixed English interface
  words live as `const`s in `menu.rs`, `render.rs` and `panels.rs`.
- The CLI must not depend on worldgen or any AI dependency.
- Run before every commit:
  - `cargo fmt --all -- --check`
  - `cargo clippy --workspace --all-targets --locked -- -D warnings`
  - `cargo test --workspace --locked`
  - Add `--offline` once dependencies are cached, and never pipe through
    `| tail`, which hides failures.
- One commit per task. Commit messages end with
  `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- realm-kit names no concrete third-party game anywhere.

## Review Focus

1. **Using a consumable at full vitals.** It is refused with
   `NothingToRestore` and nothing is consumed, while exploring and in a fight
   alike. Pinned in Task 2.
2. **A consumable that only restores MP, used by a player with max MP 0** (a
   world without MP). This is "nothing to restore", never a silent waste.
   Pinned in Task 2.
3. **A save after eating every unit of a granted consumable** (count 0, the
   key removed). It must load: consumable counts may fall below what
   progress granted. Pinned in Task 2.
4. **Buying 2 of a gear ware.** It gives two separate pieces with
   consecutive fresh IDs, the money leaves once, and a save afterwards loads.
   Pinned in Task 3.
5. **A ware at a market whose merchant is absent.** It is not offered and
   cannot be bought (`NotHere`). Pinned in Task 3.

---

### Task 1: Spec types, validation, Format 13 and the arena fixture

**Files:**
- Modify: `crates/realmkit-spec/src/items.rs` (add `Consumable`)
- Modify: `crates/realmkit-spec/src/economy.rs` (add `Ware`, `Market.wares`, `Economy::ware`)
- Modify: `crates/realmkit-spec/src/lib.rs:28` (`FORMAT_VERSION = 13`)
- Modify: `crates/realmkit-spec/src/validation/world.rs` (consumable checks, next to the existing equipment-without-combat check ~line 38)
- Modify: `crates/realmkit-spec/src/validation/economy.rs` (`markets()`: ware checks)
- Modify: `crates/realmkit-engine/src/state.rs:152` (`SAVE_FORMAT_VERSION = 13`)
- Modify: `examples/*/world.json` (all seven: `"format_version": 13`)
- Modify: `examples/arena/items.json`, `examples/arena/world.json` (economy + draught)
- Modify: `docs/superpowers/specs/2026-10-01-consumables-and-wares-design.md` (the deviations above)
- Test: `crates/realmkit-spec/tests/consumables.rs` (new)

**Interfaces:**
- Produces:
  - `pub struct Consumable { pub hp: u32, pub mp: u32 }`
  - `Item.consumable: Option<Consumable>`
  - `pub struct Ware { pub item: Id, pub price: u64 }`
  - `Market.wares: Vec<Ware>`
  - `Economy::ware(&self, market: &Market, item: &str) -> Option<&Ware>`
  - Diagnostic codes `invalid_consumable` and `invalid_ware`; reused codes
    `combat_disabled`, `invalid_price` and `missing_reference`.

- [ ] **Step 1: Write the failing spec tests**

Create `crates/realmkit-spec/tests/consumables.rs`:

```rust
//! Validation of consumable items and of wares sold at fixed prices.

mod common;

use common::*;
use realmkit_spec::*;

fn item<'w>(w: &'w mut WorldSpec, id: &str) -> &'w mut Item {
    w.items.iter_mut().find(|i| i.id == id).unwrap()
}

fn gate(w: &mut WorldSpec) -> &mut Market {
    w.world.economy.as_mut().unwrap().markets.iter_mut().find(|m| m.location == "gate").unwrap()
}

#[test]
fn the_arena_sells_a_draught_that_heals_and_roundtrips() {
    let world = arena();
    assert!(codes(&world).is_empty(), "{:?}", world.diagnostics());
    let draught = world.item("healing_draught").unwrap();
    assert_eq!(draught.consumable, Some(Consumable { hp: 30, mp: 0 }));
    let economy = world.economy().unwrap();
    let market = economy.market("gate").unwrap();
    assert_eq!(economy.ware(market, "healing_draught").unwrap().price, 8);
    assert!(economy.ware(market, "rat_tail").is_none());
    let json = serde_json::to_string(draught).unwrap();
    assert_eq!(&serde_json::from_str::<Item>(&json).unwrap(), draught);
    // A plain item serializes without the field.
    let tail = serde_json::to_string(world.item("rat_tail").unwrap()).unwrap();
    assert!(!tail.contains("consumable"), "{tail}");
}

#[test]
fn consumables_are_checked_with_stable_codes() {
    type Change = fn(&mut WorldSpec);
    let cases: Vec<(Change, &str)> = vec![
        (|w| item(w, "healing_draught").consumable = Some(Consumable { hp: 0, mp: 0 }), "invalid_consumable"),
        (|w| item(w, "healing_draught").consumable = Some(Consumable { hp: STAT_BOUND + 1, mp: 0 }), "invalid_consumable"),
        (|w| item(w, "healing_draught").consumable = Some(Consumable { hp: 0, mp: STAT_BOUND + 1 }), "invalid_consumable"),
        // Wearing and eating the same thing is not a thing.
        (|w| item(w, "buckler").consumable = Some(Consumable { hp: 5, mp: 0 }), "invalid_consumable"),
    ];
    for (change, code) in cases {
        let mut world = arena();
        change(&mut world);
        assert!(codes(&world).contains(&code.to_string()), "{code}: {:?}", world.diagnostics());
    }
    // Restoring HP needs HP: a world without combat has none.
    let mut archive = archive();
    archive.items[0].consumable = Some(Consumable { hp: 5, mp: 0 });
    assert!(codes(&archive).contains(&"combat_disabled".to_string()));
}

#[test]
fn wares_are_checked_with_stable_codes() {
    type Change = fn(&mut WorldSpec);
    let cases: Vec<(Change, &str)> = vec![
        (|w| gate(w).wares[0].price = 0, "invalid_price"),
        (|w| gate(w).wares[0].price = PRICE_BOUND + 1, "invalid_price"),
        (|w| gate(w).wares[0].item = "missing".into(), "missing_reference"),
        // A good already has a moving price; it cannot also have a fixed one.
        (|w| gate(w).wares[0].item = "rat_tail".into(), "invalid_ware"),
        (|w| {
            let first = gate(w).wares[0].clone();
            gate(w).wares.push(first);
        }, "invalid_ware"),
    ];
    for (change, code) in cases {
        let mut world = arena();
        change(&mut world);
        assert!(codes(&world).contains(&code.to_string()), "{code}: {:?}", world.diagnostics());
    }
}
```

- [ ] **Step 2: Run them and see them fail**

Run: `cargo test -p realmkit-spec --test consumables`
Expected: compile errors, because `Consumable`, `Ware`, `wares` and
`Economy::ware` don't exist yet.

- [ ] **Step 3: Add the types**

In `crates/realmkit-spec/src/items.rs`, add the field to `Item` after
`equipment`:

```rust
    /// Makes the item usable: each use spends one and restores vitals.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub consumable: Option<Consumable>,
```

and the type after `Modifier`:

```rust
/// What using one unit restores, each capped at the maximum.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Consumable {
    #[serde(default, skip_serializing_if = "is_zero")]
    pub hp: u32,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub mp: u32,
}
```

(`is_zero` is the crate-root helper in `lib.rs`; `use crate::*` brings it in.)

In `crates/realmkit-spec/src/economy.rs`, add to `Market` after
`spread_percent`:

```rust
    /// Items sold here at a fixed price, buy-only and never running out.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub wares: Vec<Ware>,
```

the type after `TradeLink`:

```rust
/// An item a market sells at a fixed price per unit.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Ware {
    pub item: Id,
    pub price: u64,
}
```

and to `impl Economy`:

```rust
    /// The ware a market sells as `item`, if any.
    pub fn ware<'m>(&self, market: &'m Market, item: &str) -> Option<&'m Ware> {
        market.wares.iter().find(|w| w.item == item)
    }
```

Then set `FORMAT_VERSION` to 13 in `lib.rs` and `SAVE_FORMAT_VERSION` to 13
in `crates/realmkit-engine/src/state.rs`. Run
`sed -i 's/"format_version": 12/"format_version": 13/' examples/*/world.json`.
Fix any struct-literal compile errors in tests or worldgen (`Item { .. }` or
`Market { .. }` literals) by adding `consumable: None` or `wares: vec![]`;
find them with `cargo build --workspace --all-targets`.

- [ ] **Step 4: Validate consumables**

In `crates/realmkit-spec/src/validation/world.rs`, right after the existing
loop that flags equipment items in a world without combat (~lines 38–48),
add:

```rust
    for item in &w.items {
        let Some(consumable) = item.consumable else {
            continue;
        };
        if w.combat().is_none() {
            issue(
                out,
                &item.id,
                "combat_disabled",
                "this world has no combat block, so there are no HP or MP to restore",
            );
        }
        let restores = consumable.hp > 0 || consumable.mp > 0;
        let bounded = consumable.hp <= STAT_BOUND && consumable.mp <= STAT_BOUND;
        if !restores || !bounded || item.equipment.is_some() {
            issue(
                out,
                &item.id,
                "invalid_consumable",
                format!("a consumable restores 1 to {STAT_BOUND} HP or MP and is not equipment"),
            );
        }
    }
```

- [ ] **Step 5: Validate wares**

In `crates/realmkit-spec/src/validation/economy.rs`, inside `markets()`'s
`for market in &economy.markets` loop, after the `market.prices` loop, add:

```rust
        let mut sold = BTreeSet::new();
        for ware in &market.wares {
            reference(out, owner, "item", &ware.item, w.item(&ware.item).is_some());
            if !(1..=PRICE_BOUND).contains(&ware.price) {
                issue(
                    out,
                    owner,
                    "invalid_price",
                    format!("{} costs 1 to {PRICE_BOUND}", ware.item),
                );
            }
            if economy.good(&ware.item).is_some() || !sold.insert(&ware.item) {
                issue(
                    out,
                    owner,
                    "invalid_ware",
                    format!("{} is sold here once, and is not also a trade good", ware.item),
                );
            }
        }
```

(Add `use std::collections::BTreeSet;` if `super::*` doesn't already provide
it; check `validation/mod.rs` imports.)

- [ ] **Step 6: Give the arena a quartermaster**

In `examples/arena/items.json`, add after `rat_tail`:

```json
  {
    "id": "healing_draught",
    "name": "Healing draught",
    "description": "Bitter, green, and better than bleeding.",
    "consumable": { "hp": 30 }
  },
```

In `examples/arena/world.json`, add a top-level `economy` block after
`combat`:

```json
  "economy": {
    "currency": { "format": "{amount} marks", "start": 10 },
    "goods": [{ "item": "rat_tail", "price": 6 }],
    "markets": [
      {
        "location": "gate",
        "kind": "town",
        "wares": [
          { "item": "healing_draught", "price": 8 },
          { "item": "iron_mail", "price": 60 }
        ]
      }
    ],
    "index_bounds": [100, 10000],
    "spread_percent": 15,
    "trade_step": 26
  }
```

- [ ] **Step 7: Run the spec tests, then the workspace**

Run: `cargo test -p realmkit-spec --test consumables`
Expected: PASS.

Run: `cargo test --workspace --locked`
Expected: PASS. Watch for:
- Arena CLI tests whose menu numbers moved. `Market ›` sits after
  `Train stats ›` and `Equipment ›`, so the gate's numbers shouldn't shift;
  if one does, update the number in that test.
- Arena terminal tests that list the gate menu.

- [ ] **Step 8: Update the spec document**

In `docs/superpowers/specs/2026-10-01-consumables-and-wares-design.md`:
- Replace the events line with `Event::Consumed { item, hp, mp }`.
- Replace `NotCarried` with `NotEnoughMaterials`.
- Change the fixture section to `examples/arena`: a gate quartermaster
  selling a healing draught (8 marks) and iron mail (60), with rat tails
  sellable as a good (price 6).
- Drop the equipment-ware `combat_disabled` line, with the reason given
  above.

- [ ] **Step 9: Lint and commit**

```bash
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked
git add -A crates/realmkit-spec crates/realmkit-engine/src/state.rs examples docs/superpowers/specs
git commit -m "spec: consumable items and fixed-price wares (Format 13)

Items may restore HP and MP when used; markets may sell items at a fixed
price, buy-only. Validation rejects empty or unbounded restores, consumable
equipment, consumables without combat, unpriced, duplicate or good-shadowing
wares. The arena's gate gains a quartermaster selling a healing draught and
iron mail, and buys rat tails.

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 2: Engine — `Use` while exploring and as a fight turn

**Files:**
- Modify: `crates/realmkit-engine/src/command.rs` (`Command::Use`, `Event::Consumed`)
- Modify: `crates/realmkit-engine/src/error.rs` (`NotConsumable`, `NothingToRestore`)
- Create: `crates/realmkit-engine/src/rules/consume.rs`
- Modify: `crates/realmkit-engine/src/rules/mod.rs` (module, dispatch, `combat_action`)
- Modify: `crates/realmkit-engine/src/encounter/mod.rs` (`consume_turn`)
- Modify: `crates/realmkit-engine/src/rules/actions.rs` (offer `Use`)
- Modify: `crates/realmkit-engine/src/save/mod.rs` (`Progress::of`: consumables join `taken`)
- Modify: `crates/realmkit-cli/src/render.rs` (render `Consumed`, so the workspace compiles)
- Test: `crates/realmkit-engine/tests/consumables.rs` (new)

**Interfaces:**
- Consumes: `Consumable` and `Item.consumable` (Task 1).
- Produces:
  - `Command::Use(Id)`
  - `Event::Consumed { item: Id, hp: u32, mp: u32 }`, with the amounts
    actually gained
  - `EngineError::NotConsumable(Id)` and `EngineError::NothingToRestore`
  - `pub(crate) fn rules::consume::gain(hp: u32, mp: u32, max: Stats, c: Consumable) -> (u32, u32)`

- [ ] **Step 1: Write the failing engine tests**

Create `crates/realmkit-engine/tests/consumables.rs`:

```rust
//! Using items that restore HP and MP, exploring and as a fight turn.

mod common;

use common::*;
use realmkit_engine::{Command::*, *};
use realmkit_spec::*;

fn draughts(engine: &Engine<'_>) -> u64 {
    engine.state().player.inventory.get("healing_draught").copied().unwrap_or(0)
}

/// The arena with enough marks for a few draughts (it starts with 10).
fn arena_with_marks(marks: u64) -> WorldSpec {
    let mut world = arena();
    world.world.economy.as_mut().unwrap().currency.start = marks;
    world
}

/// An arena player carrying `n` draughts, hurt by the warren's rat.
/// `world` needs at least `8 × n` starting marks.
fn hurt_with_draughts(world: &WorldSpec, n: u64) -> Engine<'_> {
    let mut engine = Engine::new_with_seed(world, 7).unwrap();
    engine.execute(Buy { good: "healing_draught".into(), quantity: n }).unwrap();
    engine.execute(Move(Direction::East)).unwrap();
    engine.execute(Engage("rat".into())).unwrap();
    fight_out(&mut engine);
    engine.execute(Move(Direction::West)).unwrap();
    assert!(vitals(&engine).hp < engine.player_stats().unwrap().hp);
    engine
}

#[test]
fn a_draught_restores_hp_up_to_the_maximum_and_is_spent() {
    let world = arena();
    let mut engine = hurt_with_draughts(&world, 1);
    let max = engine.player_stats().unwrap().hp;
    let before = vitals(&engine).hp;
    let events = engine.execute(Use("healing_draught".into())).unwrap();
    let gained = (max - before).min(30);
    assert_eq!(events, [Event::Consumed { item: "healing_draught".into(), hp: gained, mp: 0 }]);
    assert_eq!(vitals(&engine).hp, before + gained);
    assert_eq!(draughts(&engine), 0);
    assert!(!engine.state().player.inventory.contains_key("healing_draught"));
}

#[test]
fn using_is_refused_when_it_would_restore_nothing_or_nothing_is_carried() {
    let mut world = arena();
    let mut engine = Engine::new_with_seed(&world, 7).unwrap();
    engine.execute(Buy { good: "healing_draught".into(), quantity: 1 }).unwrap();
    let before = engine.state().clone();
    // Full health: nothing to restore, and nothing is spent.
    assert!(matches!(engine.execute(Use("healing_draught".into())), Err(EngineError::NothingToRestore)));
    assert_eq!(engine.state(), &before);
    assert!(matches!(engine.execute(Use("rat_tail".into())), Err(EngineError::NotConsumable(_))));
    assert!(matches!(engine.execute(Use("missing".into())), Err(EngineError::NotConsumable(_))));
    // An MP-only draught in a world whose player has no MP restores nothing.
    world.items.iter_mut().find(|i| i.id == "healing_draught").unwrap().consumable =
        Some(Consumable { hp: 0, mp: 10 });
    let mut engine = hurt_with_draughts(&world, 1);
    assert_eq!(engine.player_stats().unwrap().mp, 0);
    assert!(matches!(engine.execute(Use("healing_draught".into())), Err(EngineError::NothingToRestore)));
    // None carried.
    let plain = arena();
    let mut engine = hurt_with_draughts(&plain, 1);
    engine.execute(Use("healing_draught".into())).unwrap();
    assert!(matches!(
        engine.execute(Use("healing_draught".into())),
        Err(EngineError::NotEnoughMaterials(_))
    ));
}

#[test]
fn using_in_a_fight_spends_the_turn_and_the_opponents_act() {
    let world = arena_with_marks(100);
    let mut engine = hurt_with_draughts(&world, 2);
    engine.execute(Move(Direction::East)).unwrap();
    engine.execute(Engage("rat".into())).unwrap();
    let now = engine.encounter().unwrap().now;
    let events = engine.execute(Use("healing_draught".into())).unwrap();
    assert!(matches!(events[0], Event::Consumed { .. }));
    // The rat answered before the player's next turn, which came one basic action later.
    assert!(events.iter().any(|e| matches!(e, Event::DamageReceived { .. })));
    assert!(engine.encounter().unwrap().now > now);
    assert_eq!(draughts(&engine), 1);
    // Replays are exact.
    let mut again = hurt_with_draughts(&world, 2);
    again.execute(Move(Direction::East)).unwrap();
    again.execute(Engage("rat".into())).unwrap();
    assert_eq!(again.execute(Use("healing_draught".into())).unwrap(), events);
    assert_eq!(again.state(), engine.state());
    // A save in the middle loads.
    Engine::restore(&world, engine.snapshot()).unwrap();
}

#[test]
fn draughts_are_offered_when_carried_and_unavailable_at_full_health() {
    let world = arena();
    let mut engine = Engine::new_with_seed(&world, 7).unwrap();
    assert!(!offered(&engine).iter().any(|(c, _)| matches!(c, Use(_))));
    engine.execute(Buy { good: "healing_draught".into(), quantity: 1 }).unwrap();
    assert!(offered(&engine).contains(&(Use("healing_draught".into()), false)));
    let engine = hurt_with_draughts(&world, 1);
    assert!(offered(&engine).contains(&(Use("healing_draught".into()), true)));
}

#[test]
fn a_save_with_granted_consumables_eaten_loads() {
    // Loot that is consumable may be eaten away below what the defeat
    // granted. Make the draught the rat's only, once-only loot: not a ware,
    // not a good, not a repeatable group's loot, since those already make
    // any count acceptable and would hide the rule under test.
    let mut world = arena();
    let economy = world.world.economy.as_mut().unwrap();
    economy.markets[0].wares.clear();
    economy.goods.clear();
    let combat = world.world.combat.as_mut().unwrap();
    combat.groups.iter_mut().find(|g| g.id == "warren").unwrap().repeatable = false;
    combatant(&mut world, "rat").loot =
        vec![ItemStack { item: "healing_draught".into(), quantity: 1 }];
    let mut engine = Engine::new_with_seed(&world, 7).unwrap();
    engine.execute(Move(Direction::East)).unwrap();
    engine.execute(Engage("rat".into())).unwrap();
    fight_out(&mut engine);
    assert_eq!(draughts(&engine), 1);
    assert!(vitals(&engine).hp < engine.player_stats().unwrap().hp);
    engine.execute(Use("healing_draught".into())).unwrap();
    assert_eq!(draughts(&engine), 0);
    Engine::restore(&world, engine.snapshot()).unwrap();
}
```

Check that this test fails without Step 6's `taken` change (comment it out,
run the test, see `inventory does not match progress`, then restore it). It
must fail, or it isn't testing Review Focus item 3.

- [ ] **Step 2: Run and see failure**

Run: `cargo test -p realmkit-engine --test consumables`
Expected: compile errors (`Use`, `Consumed` and the new errors don't exist yet).

- [ ] **Step 3: Command, event, errors**

In `command.rs`, add to `Command` after `Rest`:

```rust
    /// Uses one carried consumable: restores HP and MP; in an encounter, it
    /// is the player's turn.
    Use(Id),
```

and to `Event` after `Rested`:

```rust
    /// One unit of `item` was used; `hp` and `mp` are what it restored.
    Consumed {
        item: Id,
        hp: u32,
        mp: u32,
    },
```

In `error.rs`, after `NotSafe`:

```rust
    #[error("{0} cannot be used")]
    NotConsumable(Id),
    #[error("that would restore nothing")]
    NothingToRestore,
```

- [ ] **Step 4: The rule**

Create `crates/realmkit-engine/src/rules/consume.rs`:

```rust
//! Using a consumable: it restores vitals now, and in an encounter it takes
//! the player's turn.

use super::*;
use realmkit_spec::Consumable;

/// What a consumable restores to vitals `hp`/`mp`, capped at `max`.
pub(crate) fn gain(hp: u32, mp: u32, max: Stats, c: Consumable) -> (u32, u32) {
    (
        c.hp.min(max.hp.saturating_sub(hp)),
        c.mp.min(max.mp.saturating_sub(mp)),
    )
}

/// The consumable `item`, if it is one.
pub(crate) fn consumable(world: &WorldSpec, item: &str) -> Result<Consumable, EngineError> {
    world
        .item(item)
        .and_then(|i| i.consumable)
        .ok_or_else(|| EngineError::NotConsumable(item.into()))
}

pub(crate) fn consume(
    world: &WorldSpec,
    state: &mut GameState,
    item: Id,
    events: &mut Vec<Event>,
) -> Result<(), EngineError> {
    let restores = consumable(world, &item)?;
    let combat = state.combat.as_ref().ok_or(EngineError::NotConsumable(item.clone()))?;
    let max = player_stats(world, combat);
    let now = player_vitals(state).unwrap();
    let (hp, mp) = gain(now.hp, now.mp, max, restores);
    if hp == 0 && mp == 0 {
        return Err(EngineError::NothingToRestore);
    }
    let stack = [realmkit_spec::ItemStack { item: item.clone(), quantity: 1 }];
    story::take_items(state, &stack, &mut Vec::new())?;
    events.push(Event::Consumed { item, hp, mp });
    let combat = state.combat.as_mut().unwrap();
    match &mut combat.stance {
        Stance::Exploring(vitals) => {
            vitals.hp += hp;
            vitals.mp += mp;
            state.dialogue = None;
            Ok(())
        }
        Stance::Fighting(_) => encounter::consume_turn(world, state, hp, mp, events),
    }
}
```

(`vitals.hp + hp ≤ max.hp ≤ STAT_BOUND`, so the plain `+` cannot overflow.)

In `encounter/mod.rs`, after `flee`:

```rust
/// Spends the player's turn on a consumable: `hp`/`mp` (already capped) are
/// restored now, the turn takes one basic action's time, and opponents act
/// until the player's next turn. No rage or technique XP is earned.
pub(super) fn consume_turn(
    world: &WorldSpec,
    state: &mut GameState,
    hp: u32,
    mp: u32,
    events: &mut Vec<Event>,
) -> Result<(), EngineError> {
    let (encounter, me) = fighting(world, &mut state.combat)?;
    let step = delay(&world.combat().unwrap().timeline, me.stats.speed, 100)?;
    let player = &mut encounter.participants[0];
    player.hp += hp;
    player.mp += mp;
    player.next_time = player
        .next_time
        .checked_add(step)
        .ok_or(EngineError::NumericLimit)?;
    advance(world, state, events, false)
}
```

In `rules/mod.rs`:
- Add `mod consume;`.
- Add `Command::Use(item) => consume::consume(world, state, item, &mut events)?,`
  to the `execute` match.
- Extend `combat_action` to
  `Command::Attack(_) | Command::UseSkill { .. } | Command::Flee | Command::Use(_)`.

- [ ] **Step 5: Offer it**

In `rules/actions.rs`, add a helper:

```rust
/// One entry per carried consumable, in inventory order; unavailable when
/// it would restore nothing.
fn consumables(world: &WorldSpec, state: &GameState) -> Vec<Action> {
    let (Some(combat), Some(now)) = (&state.combat, player_vitals(state)) else {
        return Vec::new();
    };
    let max = player_stats(world, combat);
    state
        .player
        .inventory
        .keys()
        .filter_map(|id| {
            let c = consume::consumable(world, id).ok()?;
            let (hp, mp) = consume::gain(now.hp, now.mp, max, c);
            Some(Action { command: Command::Use(id.clone()), available: hp > 0 || mp > 0 })
        })
        .collect()
}
```

- **Exploring:** in `actions()`, insert `actions.extend(consumables(world, state));`
  right after the `Equip` block and before `crafting::offered`.
- **Fighting:** in `fight_actions()`, insert
  `actions.extend(consumables(world, state));` before the `Flee` push.
- Make `consume` visible to `actions.rs` with `pub(super) mod consume;` or a
  `use super::consume;`, matching how `crafting::offered` is reached.

- [ ] **Step 6: Saves: eaten consumables may fall below what was granted**

In `save/mod.rs`, in `Progress::of`, change the `taken` computation to:

```rust
        let consumables = world.items.iter().filter(|i| i.consumable.is_some()).map(|i| &i.id);
        let taken = stacks(true)
            .map(|s| &s.item)
            .chain(consumables)
            .filter(|item| !loose.contains(item))
            .collect();
```

Update the `taken` field's doc comment to: "Items effects take or players use
up, never grant: their counts can fall below what progress granted, but
never rise above it."

- [ ] **Step 7: Render (keeps the CLI compiling)**

In `crates/realmkit-cli/src/render.rs`, next to `Event::Rested`:

```rust
            Event::Consumed { item, hp, mp } => {
                let mut gains = Vec::new();
                if *hp > 0 {
                    gains.push(format!("+{hp} HP"));
                }
                if *mp > 0 {
                    gains.push(format!("+{mp} MP"));
                }
                writeln!(output, "You use {}: {}.", world.item(item).unwrap().name, gains.join(", "))?
            }
```

- [ ] **Step 8: Run tests**

Run: `cargo test -p realmkit-engine --test consumables`
Expected: PASS.

Run: `cargo test --workspace --locked`
Expected: PASS.

- [ ] **Step 9: Lint and commit**

```bash
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked
git add -A crates
git commit -m "engine: use consumables, exploring or as a fight turn

Use spends one carried consumable and restores HP and MP up to the
maxima; one that would restore nothing is refused and kept. In an
encounter it is the player's turn: one basic action of time passes and
opponents act, with no rage or technique XP. Carried consumables are
offered, unavailable when they would restore nothing. Saves accept
consumable counts below what progress granted.

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 3: Engine — buying wares

**Files:**
- Modify: `crates/realmkit-engine/src/rules/economy.rs` (`trade`: the ware branch; `ware_price`)
- Modify: `crates/realmkit-engine/src/rules/actions.rs` (`trade()`: offer wares)
- Modify: `crates/realmkit-engine/src/rules/mod.rs` (re-export `ware_price`)
- Modify: `crates/realmkit-engine/src/lib.rs` (`Engine::ware_price`)
- Modify: `crates/realmkit-engine/src/save/mod.rs` (`Progress::of`: wares join `loose`)
- Test: `crates/realmkit-engine/tests/economy.rs` (append)

**Interfaces:**
- Consumes: `Ware`, `Market.wares` and `Economy::ware` (Task 1).
- Produces:
  - `pub fn Engine::ware_price(&self, item: &str) -> Option<u64>`: the price
    of one unit at the open market here.
  - `pub(crate) fn rules::economy::ware_price(world, state, item) -> Option<u64>`

- [ ] **Step 1: Write the failing tests**

Append to `crates/realmkit-engine/tests/economy.rs`:

```rust
#[test]
fn wares_sell_at_a_fixed_price_and_gear_arrives_as_pieces() {
    let world = arena();
    let mut engine = Engine::new_with_seed(&world, 7).unwrap();
    assert_eq!(engine.ware_price("healing_draught"), Some(8));
    assert_eq!(engine.ware_price("rat_tail"), None);
    let events = engine.execute(buy("healing_draught", 1)).unwrap();
    assert!(events.contains(&Event::Bought { good: "healing_draught".into(), quantity: 1, cost: 8 }));
    assert_eq!(currency(&engine), 2);
    // A fixed price: the next one costs the same, and none can be sold back.
    assert_eq!(engine.ware_price("healing_draught"), Some(8));
    assert!(matches!(engine.execute(sell("healing_draught", 1)), Err(EngineError::NotTraded(_))));
    assert!(matches!(engine.execute(buy("healing_draught", 1)), Err(EngineError::NotEnoughCurrency)));
    // Two pieces of mail are two pieces, and the money leaves once.
    let mut rich = arena();
    rich.world.economy.as_mut().unwrap().currency.start = 200;
    let mut engine = Engine::new_with_seed(&rich, 7).unwrap();
    let before: Vec<u64> = combat(&engine).gear.keys().copied().collect();
    engine.execute(buy("iron_mail", 2)).unwrap();
    assert_eq!(currency(&engine), 80);
    let new: Vec<_> = combat(&engine).gear.iter().filter(|(id, _)| !before.contains(id)).collect();
    assert_eq!(new.len(), 2);
    assert!(new.iter().all(|(_, g)| g.item == "iron_mail" && !g.equipped));
    assert_eq!(*new[1].0, *new[0].0 + 1);
    Engine::restore(&rich, engine.snapshot()).unwrap();
}

#[test]
fn wares_are_offered_only_at_an_open_market() {
    let world = arena();
    let mut engine = Engine::new_with_seed(&world, 7).unwrap();
    assert!(offered(&engine).contains(&(buy("healing_draught", 1), true)));
    assert!(offered(&engine).contains(&(buy("iron_mail", 1), false)));
    engine.execute(Move(Direction::East)).unwrap();
    assert!(!offered(&engine).iter().any(|(c, _)| *c == buy("healing_draught", 1)));
    assert!(matches!(engine.execute(buy("healing_draught", 1)), Err(EngineError::NoMarket)));
    // A market whose merchant is away sells no wares either.
    let mut world = marches();
    let greyford = world.world.economy.as_mut().unwrap().markets.iter_mut()
        .find(|m| m.location == "greyford").unwrap();
    greyford.wares.push(Ware { item: "keep_token".into(), price: 5 });
    let mut engine = Engine::new_with_seed(&world, 7).unwrap();
    engine.execute(Wait(15 * 60)).unwrap(); // past 23:00, Hild has gone home
    assert_eq!(engine.ware_price("keep_token"), None);
    assert!(matches!(engine.execute(buy("keep_token", 1)), Err(EngineError::NotHere(_))));
}
```

- Imports: add `Direction` and `Ware` if the file's `use` lines don't cover
  them. `realmkit_spec::*` covers both.
- Helpers: copy `arena()` into `tests/common/mod.rs` if it isn't already
  there (it is), and `combat`/`offered` exist there too.
- The marches wait: check `hild`'s `time_of_day` (360–1320) and the start
  minute (480). 15 h from 08:00 is 23:00, which is outside. Adjust if the
  fixture differs.
- `keep_token` must not be loose for the purchase to matter; the test only
  needs the refusal.

- [ ] **Step 2: Run and see failure**

Run: `cargo test -p realmkit-engine --test economy wares`
Expected: compile error (`ware_price` doesn't exist).

- [ ] **Step 3: The ware branch in `trade`**

In `rules/economy.rs`, at the top of `trade()` right after `market_here`,
replace the `economy.good(good)` lookup with:

```rust
    let (economy, market) = market_here(world, state)?;
    if quantity == 0 || quantity > TRADE_BOUND {
        return Err(EngineError::InvalidQuantity);
    }
    if let (Some(ware), true) = (economy.ware(market, good), buying) {
        return buy_ware(world, state, ware, quantity, events);
    }
    let good = economy
        .good(good)
        .ok_or_else(|| EngineError::NotTraded(good.into()))?;
```

Delete the later duplicate quantity check. Add:

```rust
/// Buys `quantity` of a ware at its fixed price, all or nothing.
fn buy_ware(
    world: &WorldSpec,
    state: &mut GameState,
    ware: &realmkit_spec::Ware,
    quantity: u64,
    events: &mut Vec<Event>,
) -> Result<(), EngineError> {
    let cost = ware.price.checked_mul(quantity).ok_or(EngineError::NumericLimit)?;
    let wallet = state.economy.as_mut().unwrap();
    wallet.currency = wallet.currency.checked_sub(cost).ok_or(EngineError::NotEnoughCurrency)?;
    let stack = [realmkit_spec::ItemStack { item: ware.item.clone(), quantity }];
    story::grant_items(world, state, &stack, &mut Vec::new())?;
    events.push(Event::Bought { good: ware.item.clone(), quantity, cost });
    Ok(())
}

/// One unit's price of a ware at the open market here.
pub(crate) fn ware_price(world: &WorldSpec, state: &GameState, item: &str) -> Option<u64> {
    let (economy, market) = market_here(world, state).ok()?;
    Some(economy.ware(market, item)?.price)
}
```

Selling a ware stays `NotTraded`, because it isn't a good.

- [ ] **Step 4: Offer wares and expose the price**

In `rules/actions.rs` `trade()`, after the goods' buy loop:

```rust
    let (_, market) = economy::market_here(world, state).unwrap();
    for ware in &market.wares {
        // A counted ware must fit the carried count too.
        let fits = held(&ware.item).is_none_or(|n| n.checked_add(1).is_some());
        actions.push(Action {
            command: Command::Buy { good: ware.item.clone(), quantity: 1 },
            available: wallet.currency >= ware.price && fits,
        });
    }
```

(The existing `let Ok((economy, _)) = ...` can bind `market` instead:
`let Ok((economy, market)) = ...`. Use that rather than calling
`market_here` twice.)

- In `rules/mod.rs`: change `pub(super) use economy::quote;` to
  `pub(super) use economy::{quote, ware_price};`.
- In `lib.rs`, after `quote`:

```rust
    /// One unit's fixed price of a ware at the open market here; `None` away
    /// from an open market or for an item not sold here.
    pub fn ware_price(&self, item: &str) -> Option<u64> {
        rules::ware_price(self.world, &self.state, item)
    }
```

- [ ] **Step 5: Saves: wares come and go freely**

In `save/mod.rs` `Progress::of`, after `let goods = ...`:

```rust
        // Wares can be bought any number of times.
        let wares = world.economy().into_iter().flat_map(|e| &e.markets).flat_map(|m| &m.wares);
```

and chain `.chain(wares.map(|w| &w.item))` after `.chain(goods.map(|g| &g.item))`
in `loose`. Update the comment above to "Trade goods and wares come and go
at markets too."

Then check that gear pieces bought as wares pass the `inventory` check: the
item is in `loose`, so any held count is accepted. The second Step 1 test
restores a save with two bought pieces; it must pass.

- [ ] **Step 6: Run tests**

Run: `cargo test -p realmkit-engine --test economy`
Expected: PASS. The pinned price-tick numbers are unchanged, since wares
draw no randomness.

Run: `cargo test --workspace --locked`
Expected: PASS.

- [ ] **Step 7: Lint and commit**

```bash
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked
git add -A crates
git commit -m "engine: buy wares at a fixed price

Buy on an item a market sells as a ware charges its fixed price per unit,
all or nothing, and grants it like any item, so equipment arrives as
separate pieces. Wares are offered at open markets, unavailable when
unaffordable, cannot be sold back, and are loose in the save check.

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 4: CLI — `Use item ›`, ware entries, market panel, `use <item>`, walkthrough

**Files:**
- Modify: `crates/realmkit-cli/src/menu.rs` (`Group::Consume`, labels for `Use` and ware `Buy`)
- Modify: `crates/realmkit-cli/src/panels.rs` (`market`: wares section)
- Modify: `crates/realmkit-cli/src/input.rs` (`use <item>`, help line)
- Modify: `examples/arena/walkthrough.txt`
- Test: `crates/realmkit-cli/src/menu.rs` `mod tests`, `crates/realmkit-cli/src/input.rs` `mod tests`, `crates/realmkit-cli/tests/terminal.rs`

**Interfaces:**
- Consumes: `Command::Use`, `Event::Consumed`, `Engine::ware_price` and
  `rules::consume::gain` via `Engine::actions()` availability (Tasks 2–3).
- Produces: no new public API.

- [ ] **Step 1: Write failing tests**

In `input.rs` `mod tests`, add:

```rust
    #[test]
    fn use_takes_an_item_alone_or_a_skill_and_a_target() {
        assert_eq!(parse("use healing_draught"), Ok(Input::Command(Command::Use("healing_draught".into()))));
        assert_eq!(
            parse("use heavy_blow ogre"),
            Ok(Input::Command(Command::UseSkill { skill: "heavy_blow".into(), target: "ogre".into() }))
        );
    }
```

In `crates/realmkit-cli/tests/terminal.rs`, add:

```rust
#[test]
fn the_arena_quartermaster_sells_a_draught_that_heals_mid_fight() {
    // The first rat fight hurts; the draught is drunk in the second.
    let input = [
        "market",
        "buy healing_draught",
        "east",
        "engage rat",
        "attack rat",
        "attack rat",
        "engage rat",
        "use healing_draught",
        "attack rat",
        "attack rat",
        "inventory",
        "west",
        "sell rat_tail",
        "use healing_draught",
    ]
    .join("\n");
    let output = run(&["play", ARENA, "--seed", "7"], &format!("{input}\n"));
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    for passage in [
        "Market at The Arena Gate:",
        "Wares:",
        "Healing draught — 8 marks",
        "Bought: Healing draught ×1 for 8 marks",
        "You use Healing draught: +",
        "Sold: Rat tail ×1 for",
        // Nothing left to drink.
        "you do not have enough healing_draught",
    ] {
        assert!(text.contains(passage), "missing {passage:?} in {text}");
    }
}
```

The last passage is the existing `NotEnoughMaterials` message. If the CLI
renders refusals differently, match what `render` prints for errors; check
`play.rs`'s error output.

In `menu.rs` `mod tests`, add a test modelled on
`training_waits_behind_a_submenu_that_stays_open_while_it_is_used`:
- Build an arena engine.
- Buy a draught, so it is carried while at full HP.
- Assert the main menu has an entry labelled `Use item ›`, and that inside it
  the label is `Healing draught ×1 — +30 HP [nothing to restore]`.
- Assert the `Market ›` group contains `Buy Healing draught — 8 marks` and
  `Buy Iron mail — 60 marks [cannot afford]`.

Read that existing test first and use its exact helpers (`Menu::new`,
`entries`, `open`).

- [ ] **Step 2: Run and see failure**

Run: `cargo test -p realmkit-cli`
Expected: the new tests fail. `use <one-arg>` is "wrong arguments", there is
no `Use item ›`, and ware labels are hidden because `quote` is `None`.

- [ ] **Step 3: Parse and help**

In `input.rs` `parse_with`, before `("use", [skill, target])`, add:

```rust
        ("use", [item]) => Command::Use((*item).into()),
```

In `help()`, after the `combat.is_some()` line, add:

```rust
    if world.items.iter().any(|i| i.consumable.is_some()) {
        attack += "use <item> — use something you carry, such as a draught\n";
    }
```

- [ ] **Step 4: Menu group and labels**

In `menu.rs`:
- Add the constants `const CONSUME_GROUP: &str = "Use item";` and
  `const NOTHING_TO_RESTORE: &str = "[nothing to restore]";`.
- Add `Consume` to `enum Group`, with `Command::Use(_) => Some(Self::Consume),`
  in `Group::of`.
- Add `Group::Consume => format!("{CONSUME_GROUP} {OPENS}"),` to
  `group_label`.

In `label()`, before the `Command::Buy` arm, add the `Use` arm:

```rust
        // "Healing draught ×2 — +30 HP", or why it would do nothing now.
        Command::Use(item) => {
            let consumable = world.item(item)?.consumable?;
            let held = engine.state().player.inventory.get(item).copied().unwrap_or(0);
            let mut gains = Vec::new();
            if consumable.hp > 0 {
                gains.push(format!("+{} HP", consumable.hp));
            }
            if consumable.mp > 0 {
                gains.push(format!("+{} {MP}", consumable.mp));
            }
            let why = if action.available { String::new() } else { format!(" {NOTHING_TO_RESTORE}") };
            format!("{} ×{held} — {}{why}", world.item(item)?.name, gains.join(", "))
        }
```

and change the start of the `Command::Buy` arm so wares get a fixed-price
label:

```rust
        Command::Buy { good, .. } => {
            let name = &world.item(good)?.name;
            // A ware: one fixed price, so no "next".
            if let Some(price) = engine.ware_price(good) {
                let why = if action.available { String::new() } else { format!(" {AFFORD}") };
                return Some(format!("{BUY} {name} — {}{why}", money(world, price)));
            }
            let quote = engine.quote(good)?;
            // ... existing body unchanged from here, minus the old `name` binding
```

Check the fight menu, i.e. `fight.rs` and how it builds its menu. If it uses
`Menu::new` and `actions()`, the `Use item ›` group appears there
automatically. Confirm by adding to the terminal test: after
`"engage rat"`, the output contains `Use item ›`.

- [ ] **Step 5: Market panel wares**

In `panels.rs` `market()`, before the final currency line:

```rust
    if !market.wares.is_empty() {
        writeln!(output, "  {}", paint.title("Wares:"))?;
        for ware in &market.wares {
            writeln!(
                output,
                "  {} — {}",
                world.item(&ware.item).unwrap().name,
                money(world, ware.price)
            )?;
        }
    }
```

The spec also asks for a gear ware's bonuses. If `render.rs` or `panels.rs`
already has a helper that formats an `Equipment`'s bonuses (look for how
`Equip` previews or the inventory describe gear), append its output in
parentheses. Otherwise skip it; the menu's equip preview covers it after
purchase. Note which you did in the commit message.

- [ ] **Step 6: Extend the arena walkthrough**

Edit `examples/arena/walkthrough.txt`:
- Insert `market` and `buy healing_draught` after `allocate hp`.
- Insert `use healing_draught` after the first `attack rat`.
- Insert `sell rat_tail` after the second `inventory`.

Run it:

`cargo run -p realmkit-cli -- play examples/arena --seed 7 --line < examples/arena/walkthrough.txt`

Expected: it ends normally. The output shows the purchase, the draught's
gain in the rat fight and the sale.

- [ ] **Step 7: Run tests**

Run: `cargo test --workspace --locked`
Expected: PASS.

- [ ] **Step 8: Lint and commit**

```bash
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked
git add -A crates examples/arena/walkthrough.txt
git commit -m "cli: use items from a submenu and buy wares

A Use item submenu, while exploring and in fights, lists carried
consumables with what they restore, marking those that would restore
nothing. Market lists wares at their fixed price, and the market panel
shows them. use <item> is a typed command beside use <skill> <target>.
The arena walkthrough buys, drinks and sells.

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 5: Docs

**Files:**
- Modify: `docs/world-format.md`
- Modify: `docs/implementation.md`
- Modify: `ROADMAP.md`
- Modify: `README.md`
- Modify: `docs/equipment.md` only if it says consumables are future work (`grep -n -i consumable docs/*.md`)

- [ ] **Step 1: World format**

In `docs/world-format.md`:
- Replace "Format 12" with "Format 13" where it names the current format
  (lines ~3, 36, 38, 53, 88, 419, 855). Rewrite line 3's sentence to say
  what Format 13 adds: consumable items and market wares.
- In the items section, document `consumable: { hp, mp }` with its
  validation rules (codes `invalid_consumable`, `combat_disabled`) and its
  behaviour: capped restore, refused when it restores nothing, a fight turn
  of one basic action.
- In the economy markets section, document `wares: [{ item, price }]`
  (codes `invalid_price`, `invalid_ware`): buy-only, fixed, unlimited, with
  gear arriving as pieces.

- [ ] **Step 2: Checklist, roadmap, README**

- **`docs/implementation.md`:** add after the Economy core entry:

```markdown
- [x] Consumables and wares (M4, M6a-2 part): items may restore HP and MP
  when used, capped at the maxima and refused when they would restore
  nothing; in an encounter a use is the player's turn, one basic action
  long. Markets may sell wares at a fixed price, buy-only and unlimited,
  equipment arriving as pieces. No new saved state; saves accept used-up
  consumables and freely bought wares. `examples/arena` exercises both.
  Package and save format 13.
```

- **`ROADMAP.md`:**
  - Change the M4 heading to `## M4 — Equipment, skills and character builds · complete`.
  - Replace "Consumables get their own later slice." with "Consumables
    (Format 13): an item restores HP and MP when used, exploring or as a
    fight turn. Cooldowns and status effects wait for a skill that needs
    them."
  - In M6a's "**M6a-2:**" sentence, add: "Wares, items a market sells at a
    fixed price, arrive early with consumables (Format 13); merchants' stock
    and money remain."
  - Grep for "Format 12" in ROADMAP and leave historical "as delivered
    (Format 12)" mentions alone.

- **`README.md`:** find where buying and selling or equipment are described
  for players (`grep -n -i "buy\|market\|equip" README.md`). Add one or two
  sentences: carried consumables appear under `Use item ›` (or `use <item>`),
  and also in fights, where using one takes your turn; some markets sell
  wares at fixed prices, gear included.

- [ ] **Step 3: Check every example still validates and commit**

```bash
for d in examples/*/; do cargo run -q -p realmkit-cli -- validate "$d" || echo "FAIL $d"; done
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked
git add -A docs ROADMAP.md README.md
git commit -m "docs: consumables and wares; mark M4 complete

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 6: Pop-small (outside the repo; nothing committed)

**Files:**
- Modify: `~/code/pop/build/pop_smoke.py`, which is git-ignored and never
  committed anywhere
- Regenerate: `~/code/pop/build/worlds/pop-small/`

- [ ] **Step 1: Bump the format and add consumables**

In `pop_smoke.py`:
- Set `"format_version": 13`.
- In the `items` comprehension, add a consumable restore per good:
  `{"itm_bread": 20, "itm_smoked_fish": 15, "itm_ale": 8}`. For example:

```python
HEALS = {"itm_bread": 20, "itm_smoked_fish": 15, "itm_ale": 8}
items = [
    {"id": g, "name": en(goods_src[g]["name"]), "description": "A trade good.",
     **({"consumable": {"hp": HEALS[g]}} if g in HEALS else {})}
    for g in GOODS
]
```

- [ ] **Step 2: Gear and wares at Laria**

- Combat block: add `"slots": ["weapon", "body", "head"]`.
- Look up three to five cheap weapons and armour in
  `~/code/pop/build/slices/eastern-marches/items.json`, matching by name or
  kind, e.g. a sword, a spear, a padded or leather armour and a helmet. Map
  their source stats to bonuses with a simple rule:
  - weapon damage ÷ 3 → `patk`;
  - body armour ÷ 3 and head armour ÷ 3 → `pdef`.
- Clamp each bonus to 1–20 and set price = source price ÷ 5. Print the
  chosen items and stats so the user can eyeball them.
- Add the items with `equipment` blocks, and `"wares": [...]` on Laria's
  market (`markets[0]`).

- [ ] **Step 3: Regenerate and smoke-test**

```bash
python3 ~/code/pop/build/pop_smoke.py ~/code/pop/build/worlds/pop-small
cd /home/jw/code/realm-kit
cargo run -q -p realmkit-cli -- validate ~/code/pop/build/worlds/pop-small
printf 'market\nbuy itm_bread 2\ntravel p_village_71\nengage brigand_1\nattack brigand_1\nuse itm_bread\nquit\n' | cargo run -q -p realmkit-cli -- play ~/code/pop/build/worlds/pop-small --seed 1 --line
```

Expected: it validates, and the transcript shows wares in Laria, the bread
eaten mid-fight and the brigand fight.

The user's saves in `~/.local/share/realmkit/pop-small` are bound to the old
revision and Format 12, so they will no longer load. Tell the user; don't
delete them.

---

## Self-review notes

- **Spec coverage:**
  - consumable content, validation and rules (exploring and in a fight):
    Tasks 1–2;
  - wares content, validation and rules: Tasks 1 and 3;
  - saves: Tasks 2–3;
  - client menus, panel and typed command: Task 4;
  - Format 13 and the fixture: Task 1;
  - docs: Task 5;
  - pop-small: Task 6.
- **The "not in this PR" items** (selling gear back, stock, merchant money,
  cooldowns, effects other than restores) have no task, by design.
- **Type consistency:**
  - `Consumable { hp, mp }`, `Ware { item, price }` and `Economy::ware(market, item)`;
  - `Command::Use(Id)`, `Event::Consumed { item, hp, mp }` and `consume::gain(hp, mp, max, c)`;
  - `Engine::ware_price(item) -> Option<u64>`.

  Each name is used the same way in every task.
