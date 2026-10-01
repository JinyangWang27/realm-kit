# Consumables and wares (Format 13)

PR 1 of 2. It closes M4 with consumables and delivers part of M6a-2: merchants
who sell items at fixed prices. PR 2 (troops and mass battles, Format 14) has
its own spec.

## Goal

Give currency something to buy besides trade goods, and tie the market to
combat: bread bought cheap in a village heals on the road, and a town smith
sells the sword that makes the next fight winnable. Both features are optional:
a world without the new fields has no new state, menus or checks.

## Consumables

### Content

`Item` gains an optional `consumable` component:

```json
{ "id": "bread", "name": "Bread", "description": "...",
  "consumable": { "hp": 25, "mp": 0 } }
```

- `hp` and `mp` are flat restores, each defaulting to 0.
- Validation (`items` domain):
  - `combat_disabled` when the world has no combat block.
  - `invalid_consumable` when both restores are 0, either exceeds
    `STAT_BOUND`, or the item also has `equipment`.
- A consumable may also be a trade good or a ware.

### Rules

`Command::Use(Id)` takes one unit from the inventory and restores vitals, each
capped at the player's effective maximum.

- **Exploring:** restores `Stance::Exploring` vitals. Refused
  (`EngineError::NothingToRestore`) when the capped HP gain and the capped MP
  gain are both 0.
- **In an encounter:** the player's turn. It restores the player participant's
  HP and MP, then the player's `next_time` advances by one basic action at
  100% time (the same `delay(timeline, speed, 100)` that flee uses), and
  opponents act until the player's next turn (`advance`).
  - Using an item that restores nothing is refused in a fight too.
  - It earns no technique XP and builds no rage, as with flee's turn.
- **Other refusals:** `NotCarried` (no unit held), `NotConsumable`, a dead
  player, and an open dialogue, which `Use` closes like other exploring
  commands do.
- **Events:** `ItemUsed { item }`, then `Restored { hp, mp }` with the amounts
  actually gained.
- **Simulator parity:** the damage and timeline formulas are unchanged; `Use`
  only spends a turn. `scripts/combat_sim` needs no change.

### Saves

There's no new state. The inventory check already treats item counts against
their sources, so a consumable that has been used simply has a lower count;
counts below the recorded-progress floor must be allowed for consumable
items, exactly as for items that effects only take. Check `Progress` in
`save/mod.rs`, which tracks items whose counts may fall.

### Client

- Exploring: a `Use item ›` submenu, offered when a consumable is carried,
  lists `Bread ×3 — +25 HP`. An entry that would restore nothing is shown
  unavailable.
- Fighting: the same submenu in the fight menu. It is grouped like
  `Equipment ›`, and the fight screen keeps working.
- Typed command: `use <item>`.
- Rendering: `You eat Bread: +25 HP.` uses fixed English interface strings in
  `render.rs`, like the other event lines.

## Wares

### Content

`Market` gains `wares`:

```json
{ "location": "laria", "kind": "town",
  "wares": [ { "item": "arming_sword", "price": 400 },
             { "item": "bandage", "price": 15 } ] }
```

- Buy-only, at a fixed price per unit, with unlimited stock and no index or
  spread.
- Validation (`economy` domain):
  - The `item` reference must exist.
  - `invalid_price` for a price outside 1..=`PRICE_BOUND`.
  - `invalid_ware` when the item is also a good, or listed twice at one
    market.
  - `combat_disabled` when a ware is equipment in a world without combat.

### Rules

`Command::Buy { good, quantity }` keeps its name and typed syntax. When `good`
names a ware at the open market here:

- It costs `price × quantity`, with checked arithmetic, and is refused with
  `NotEnoughCurrency` when unaffordable.
- It grants through `story::grant_items`, so equipment arrives as individual
  pieces (`gear::receive`) and counted items add to the inventory.
- It emits `Bought { good, quantity, cost }`.
- `quantity` is bounded by `TRADE_BOUND` as for goods. `Sell` on a ware is
  refused with `NotTraded`.
- Merchant presence rules apply unchanged, through `market_here`.

### Saves

There's no new state. Wares join the items that come and go freely: counted
wares join `loose`, as trade goods do (`save/mod.rs` ~line 336). Gear instances
of a ware item are accepted as purchases; check whether any gear provenance
check exists and add wares as a source if so.

### Client

- `Market ›` lists wares after the goods' buy entries:
  `Buy Arming Sword — 400 denars`. An unaffordable entry is shown unavailable.
- The market panel gets a `Wares` section with name and price, plus the item's
  equipment bonuses for gear, reusing the inventory's gear description.
- After buying gear the player equips it from `Equipment ›` as usual.

## Format

`FORMAT_VERSION` and `SAVE_FORMAT_VERSION` become 13, and every `examples/*`
package is bumped.

## Fixture: `examples/marches`

Marches has no combat today; it gains the minimum needed:

- A `combat` block with a short level table and `special_name`, and one safe
  location. No fighters yet; PR 2 adds troops and armies there.
- One consumable trade good, such as smoked eels at `{"hp": 15}`.
- A smith's wares at the town: one weapon, one armour piece and one counted
  consumable ware.
- The walkthrough buys and equips the weapon, buys a consumable ware and uses
  it, and keeps every existing step. The CLI terminal tests pick up the new
  output.

## Tests

- **Engine, `tests/consumables.rs`:**
  - Use while exploring restores and caps at the maximum.
  - Use is refused at full vitals, with none carried, and for a
    non-consumable.
  - Use in an encounter spends exactly one basic action of time, opponents
    act, and the result is deterministic.
  - A save mid-encounter after a `Use` loads, and a save with fewer
    consumables than progress granted loads.
  - Refused uses leave state unchanged.
- **Engine, `tests/economy.rs` additions:**
  - Buy a ware: currency falls by `price × n`.
  - Gear wares arrive as `n` separate pieces with fresh IDs.
  - Unaffordable purchases and selling a ware are refused.
  - Merchant absence closes wares too.
- **Spec:** every new diagnostic code above, plus marches round-trips.
- **CLI:** menu tests for `Use item ›` and ware entries, and the terminal
  walkthrough.

## Docs

- `docs/world-format.md`: `consumable` and `wares`.
- `docs/implementation.md`: two checklist entries.
- `ROADMAP.md`: M4 complete, with cooldowns and status effects deferred until
  a skill needs them; M6a-2 notes wares delivered.
- `README.md`: using items and buying wares.

## Outside this repo

Extend `~/code/pop/build/pop_smoke.py`, which is not committed anywhere, and
regenerate pop-small:

- Bread, smoked fish and ale become consumables.
- Laria's market sells a few weapons and armour pieces as wares.
- Brigand loot can include a cheap consumable.

## Not in this PR

- Selling gear back, limited stock and merchant money (the rest of M6a-2).
- Cooldowns, status effects and over-time effects.
- Consumables that do anything other than restore HP and MP.
