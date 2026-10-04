# World package format 17

Format 17 adds authored progression and investigation: evidence the player
discovers, story phases, main quests and quest prerequisites, route
outcomes, places the player must learn of before the map shows them, and
features to examine rather than talk to. Format 16 added optional map
positions, so a client can draw the places and roads, and start questions
the player answers at New Game. Format 15
completed the economy: markets that prosper or decline, merchants
with limited stock and purses, villages that feed their market town,
workshops the player owns, and the trading proficiency with proficiency
points from the level table. Format 14 added troops, who are recruited, level up in squads and draw
wages, and mass battles against authored armies, with allies who join.
Format 13 added consumable items that restore HP and MP, and wares that
markets sell at fixed prices. Format 12 composed conditions with `all`,
`any` and `not`, gave dialogue choices ordered effect lists, and added
optional world time with roads, scheduled events and characters who move,
and an optional economy of currency and markets whose prices follow
production. Combat stays optional. The level table and combat prose live in an
optional `combat` block in `world.json`; a world without that block has no
fighting, no XP and no levels, and its saves carry no combat state. Authors
should not insert dummy combat content into non-combat worlds. The roadmap treats
combat and other genre mechanics as source-grounded capabilities.

Older packages are rejected when loading with a message naming their format;
there is no migration. Convert them by hand:

- **Format 1** (separate `npcs.json`, `monsters.json` and `narrative.json`, a
  `player_name`): merge NPCs and monsters into `characters.json`, add a player
  character, and move `levels` and the narrative into `world.json`'s `combat`
  block, then apply the Format 2 steps.
- **Format 2** (M3a; `hp` and `attack` on level entries and combat profiles):
  replace them with a seven-stat `stats` object and add `special_name` to the
  combat block, then apply the Format 3 steps.
- **Format 3** (M3b; no timeline): add a `timeline` to the combat block, and
  `resources` if skills should regenerate MP or build rage in fights, then
  apply the Format 4 steps.
- **Format 4** (M3c-1): give each opponent whose skills unlock above level 1 a
  `level`, since opponents now use only skills unlocked at their level;
  `groups` are optional. Then apply the Formats 5–10 step.
- **Formats 5–10** (M3c-2, M3d, M4a, M4b, M4c, M4d): only raise the number;
  `crit`, `stat_points`, techniques, equipment, recipes, tiers and
  enchantments are optional. Then apply the Format 11 steps.
- **Format 11** (M4e): every `requires` and `known_when` list becomes one
  condition. A list of one condition becomes that condition; a longer list
  becomes `{ "kind": "all", "of": [...] }`; an empty list is left out. A
  dialogue choice's `effect` becomes a one-element `effects` list.
- **Formats 12–16** (M5a, M5b, M6a-1, consumables, troops, M6a-2, map and
  start questions): only raise the number. Every economy part below `tick`,
  `proficiency_points`, map positions, `start_questions`, `evidence`,
  `phases`, `outcomes`, a quest's `main` and `requires`, a location's
  `known_when` and a character's `kind` are optional.

Format 17 represents one fixed player-controlled character and one playable
route. For persistence/API identity, RealmKit exposes this implicit route under the
stable logical route ID `default`; Format 17 does not serialize a route collection
or route field. Future formats may package a canonical route, an
original-character route, or both over the same shared world and canonical
timeline. When both are
present, New Game selects between them directly; one route does not unlock the
other. Each route has its own player binding, start state, main questline, outcomes and
mutable save while reusing shared locations, NPCs, factions and other world
definitions where appropriate. Shared `Character` definitions already exist;
a future multi-route format only needs the small `PlayerSpec` route binding that
identifies which shared character the human controls. Do not duplicate a
canonical person as separate player and NPC entities merely because control
differs by route.

In an original-character route, the canonical protagonist remains in the package
as a canonical world character/NPC rather than being replaced by the player.

Format 17 also requires item and quest tables because they serve the current demo.
Inventory is not a long-term universal requirement, but quest progression is:
quests marked `main` form the route's main questline and the rest are side
quests. A non-combat player route still has a main questline whose objectives may use
dialogue, exploration, investigation or other capabilities instead of combat. Investigation evidence is independent state and may optionally
reference a physical entity/item without being stored "inside" inventory or
inferred from possession.

A package is a directory containing these required UTF-8 JSON files:

| File | Content |
| --- | --- |
| `world.json` | Format version, world ID/name/language, starting location, player character ID, declared flags, optional `combat`, `time` and `economy` blocks, optional `roads`, `events`, `start_questions`, `evidence`, `phases` and `outcomes` |
| `locations.json` | Array of locations with descriptions, directional exits, placed character IDs, optional map positions and when the player knows of them |
| `characters.json` | Array of characters (people or features) with descriptions, availability conditions, and optional dialogue and combat profile |
| `items.json` | Array of items with names and descriptions |
| `quests.json` | Array of quests with giver, defeat or flag objective, prose, rewards and completion flags |
| `dialogues.json` | Array of dialogue trees with nodes, choices, conditions and effects |

Empty content tables are `[]`; files must still exist. A host without a
filesystem, such as a browser, hands the same six files to
`WorldSpec::from_files` from memory; `PACKAGE_FILES` lists them. Extra files such as
author notes or future provenance sidecars are ignored by the runtime loader.
Unknown fields inside the defined JSON structures are rejected to catch typos.
Format 17 describes the current schema; incompatible changes require an explicit
version/migration decision.

IDs use ASCII letters, digits, `_` and `-`, with uniqueness within each entity
table and within each dialogue's node list. IDs are machine handles; displayed
names and prose are unrestricted Unicode and retain the source language.
`language` is the author-declared language tag, for example `en` or `zh-Hans`.
The validator requires a nonempty value; it is not a BCP 47 registry validator.

The package language is also the presentation language for play. A client loading
a source-backed world must display its own fixed labels, help, prompts, status
messages and player-visible errors in that language rather than falling back to
English. Stable schema keys, IDs, enum values and typed-command aliases are
machine-facing and may remain language-neutral ASCII. Format 17 does not yet carry
client locale strings; the M0 CLI therefore only fully satisfies this requirement
for English worlds.

## Locations and conditions

Exits are directed. To return along a path, author a separate reverse exit.
Directions are `north`, `south`, `east`, `west`, `up`, `down`.

The presentation model distinguishes **spatial placement** from **traversal
connectivity**. The explicit exit and road graph is authoritative movement
state. A location may give a `map` position for drawing an overland map:

```json
"map": { "x": 1180, "y": 840, "kind": "castle" }
```

- `x` grows east and `y` south, as on a screen, each from 0 to `MAP_BOUND`
  (10 000; `map_bounds`). Only relative positions matter: a client scales
  them to fit.
- `kind` is `town`, `castle`, `village` or `waypoint`. It chooses the glyph
  and how soon a label appears as the map zooms in; in that order, earlier
  kinds win when labels compete for room.
- Either every location has a position or none does (`map_partial`), and no
  two share one (`map_duplicate`).
- Positions never create, block or time a road or exit: adjacent places need
  not be connected, and a road or exit may join places far apart. A road is
  drawn as a line between its ends, and an exit as a line with an arrow
  towards its destination.

With positions, the engine offers a `Map` panel and answers a map query with
the places the player knows of, the roads with their travel minutes and the
exits between those places, and the player's place. A place is known while
its optional `known_when` condition holds, and always while the player
stands there:

```json
"known_when": { "kind": "evidence", "evidence": "wheel_ruts" }
```

An unknown place is off the map, and no road to it is offered or travelled
(`NoRoad`), so a hidden branch is no spoiler; compass exits are unaffected.
Knowledge is derived from conditions, usually evidence or flags, and is not
saved. `known_when` needs map positions (`map_disabled`). Characters who
move are not shown, since the map would reveal where they are now. Long roads can be split at waypoints (bridges, fords,
camps) whose legs' minutes add up to the whole.

Future formats may also group locations into Areas (for example a city, one
building floor or a wilderness region) with area-local positions; for an
initial grid layout, one coordinate should identify at most one location.

Clients decide how much of an area to show. A compact 5×5 city or building may be
shown in full; a larger area may use a scrolling viewport/minimap; a graph-only
area needs no grid at all. Viewport size is not package semantics. Opening a map
must not be required for ordinary movement through explicit exits.

An enterable building may be represented as a location on a city area whose exit
leads into another Area for the interior. Vertical `up`/`down` traversal remains
explicit and does not require a universal z-axis.

```json
{
  "id": "courtyard",
  "name": "Courtyard",
  "description": "Rain darkens the stone.",
  "exits": {
    "north": {
      "destination": "hall",
      "requires": { "kind": "flag", "flag": "hall_open" },
      "blocked_text": "The hall is locked."
    }
  }
}
```

A `requires` condition can appear on exits, characters and dialogue choices;
left out, it always holds. A condition is one typed leaf predicate, or a
composition of others:

```json
{ "kind": "flag", "flag": "hall_open" }
{ "kind": "quest", "quest": "quiet_the_track", "status": "ready" }
{ "kind": "technique", "technique": "azure_breath", "rank": 2 }
{ "kind": "item", "item": "pen", "quantity": 1 }
{ "kind": "all", "of": [ ... ] }
{ "kind": "any", "of": [ ... ] }
{ "kind": "not", "condition": { ... } }
```

- `flag` holds once the declared flag is set.
- `quest` holds while the quest has that status: `available`, `active`,
  `ready` or `completed`.
- `technique` holds once the player has learned the technique at least to
  that rank (see [Techniques](#techniques)), so a realm can gate an exit, a
  dialogue choice or a character.
- `item` holds while the player carries at least `quantity` (1 or more) of a
  counted item; equipment pieces are individuals and cannot be counted.
- `time_of_day` holds during part of each day; see
  [World time](#world-time-roads-and-events).
- `currency` (`{ "kind": "currency", "amount": 150 }`) holds while the player
  has at least that much; see [Economy](#economy).
- `workshop` (`{ "kind": "workshop", "workshop": "weavery", "location":
  "vellmarket" }`) holds while the player owns a workshop of that kind, in
  that town if `location` is given, which must be a town market
  (`invalid_town`); see [Workshops](#workshops).
- `proficiency` (`{ "kind": "proficiency", "proficiency": "trading", "rank":
  2 }`) holds once the player's rank is at least `rank` (1 to the
  proficiency's `max`); see
  [Proficiencies](#proficiencies).
- `evidence` (`{ "kind": "evidence", "evidence": "wheel_ruts" }`) holds once
  the player has discovered that evidence; see [Evidence](#evidence).
- `phase` (`{ "kind": "phase", "phase": "leads" }`) holds once the story
  has reached that phase or a later one; see
  [Story phases](#story-phases-main-quests-and-outcomes).
- `all` holds when every condition in `of` does, `any` when at least one
  does, and `not` when its `condition` does not. `of` must not be empty.

All flags start unset. Dialogue `set_flag` effects and quest completion flags
set them; flags are monotonic in this version. Evaluating a condition is pure:
showing a menu or a choice never changes state. Predicates stay typed: there
are no property paths, formula strings or generic numeric comparisons.

## World time, roads and events

A world may keep a clock. The optional `time` block in `world.json` counts
minutes from an authored epoch; the clock moves only when the player travels a
road, waits or rests, never on its own and never for other commands. A world
without it has no time at all, and its saves carry none.

```json
"time": {
  "start": 480,
  "clock": "Day {day}, {hour}:{minute}",
  "wait": 60,
  "rest": 480
}
```

- `start` is the minute a new game begins at (here 08:00 on day 1), at most
  1,000,000,000.
- `clock` is how clients show the time: `{day}` counts from 1, `{hour}` and
  `{minute}` have two digits. A day has 1,440 minutes.
- `wait`, if present, lets the player wait; the menu offers this many minutes
  at a time and `wait <minutes>` (or `wait 2h`, `wait 1d`) any length from 1
  to 43,200 minutes. Without it there is no waiting.
- `rest`, if present, is how long resting at a safe place takes; it needs the
  combat block.

`roads` join locations, in either direction:

```json
"roads": [
  { "id": "greyford-ashmere", "between": ["greyford", "ashmere"], "minutes": 120 },
  {
    "id": "fen-causeway",
    "between": ["ashmere", "vellmarket"],
    "minutes": 180,
    "requires": { "kind": "flag", "flag": "thaw" },
    "blocked_text": "Meltwater still covers the fen causeway."
  }
]
```

A road joins two different locations, and at most one road joins any pair.
`travel <location-id>` (or `go <location-id>`) takes the road from the
player's location: the player arrives, then its `minutes` (up to 43,200) pass.
A road without minutes takes no time and needs no clock. A road may have a
`requires` condition, and then needs a `blocked_text` shown when it does not
hold. Roads and compass exits can be mixed, even at one location; exits stay
directed and take no time. Clients list roads in authored order.

`events` happen on a schedule:

```json
"events": [
  { "id": "thaw", "schedule": { "at": 2280 }, "effects": [{ "kind": "set_flag", "flag": "thaw" }] },
  { "id": "bell", "schedule": { "at": 1800, "every": 1440 }, "requires": { ... }, "effects": [ ... ] }
]
```

A `schedule` first falls at minute `at`, which must be after `start`, and then,
if it has one, every `every` minutes (at least 1). An occurrence applies the
event's `effects` in order when its optional `requires` holds, and does nothing
otherwise. An occurrence whose effects cannot all apply, such as a grant past
a bound, is skipped whole, so it never holds time back. Events may `set_flag`, `grant_items`, `grant_currency` and
`grant_technique` (a recurring event without XP); they cannot accept or
complete quests, take items or take payment, which belong to conversations.

A character may move among locations on a schedule:

```json
{
  "id": "wenna",
  "name": "Old Wenna",
  "dialogue": "wenna",
  "moves": { "among": ["greyford", "ashmere", "vellmarket"], "schedule": { "at": 1800, "every": 1440 } }
}
```

At each occurrence it goes to one of `among` (two or more locations), drawn
from the world's seeded random stream; it may stay where it is. It starts
where it is placed, which must be exactly one location of `among`, and is
present only where it is now. A mover has no combat profile. The client is
told when it arrives at or leaves the player's location, while it is present
under its conditions; nobody is seen coming or going while the player is on
the road, and the destination shows who is there on arrival.

When time passes, every occurrence it crosses happens in chronological order,
with the clock at that occurrence's minute. Occurrences at the same minute go
in schedule order: events in authored order, movers in character order, then
the economy's price tick.
Because every first occurrence is after `start`, nothing is due when play
begins, and an occurrence can never schedule another at its own minute. Saves
keep the minute and each mover's location; the next occurrence of every
schedule follows from the minute, so nothing else is saved.

A `time_of_day` condition holds while the minute of the day is in
`from..to`, wrapping past midnight when `from` is larger:

```json
{ "kind": "time_of_day", "from": 480, "to": 1200 }
```

It needs the time block, as do events, movers and roads with minutes
(`time_disabled` otherwise).

## Economy

The optional `economy` block in `world.json` gives the player currency and
lets locations be markets. Prices are not authored as prices: each market
keeps a price index per good, in thousandths of the good's base price, and
the index moves with what the market makes and needs.

```json
"economy": {
  "currency": { "format": "{amount} silver", "start": 100 },
  "goods": [
    { "item": "wool", "price": 40, "demand": { "town": 2 } },
    { "item": "cloth", "price": 120, "demand": { "town": 6, "village": 1 }, "input": "wool" }
  ],
  "producers": [
    { "id": "flocks", "name": "Sheep runs", "yields": { "wool": 4 } },
    { "id": "looms", "name": "Looms", "yields": { "cloth": 2 }, "consumes": { "wool": 3 } }
  ],
  "markets": [
    {
      "location": "vellmarket",
      "kind": "town",
      "merchant": "maddoc",
      "producers": { "flocks": 6, "looms": 6 },
      "prices": { "wool": 745, "cloth": 712 }
    }
  ],
  "links": [{ "between": ["greyford", "ashmere"], "percent": 10 }],
  "index_bounds": [100, 10000],
  "spread_percent": 15,
  "trade_step": 26,
  "tick": {
    "schedule": { "at": 1440, "every": 1440 },
    "supply_step": 8,
    "damp_below": 900,
    "revert_percent": 3,
    "input_pull_percent": 10
  }
}
```

- **Currency.** `format` shows an amount through `{amount}`, in the world's
  language; `start` is what a new game begins with. The player holds at most
  10^12.
- **Goods** are counted items, each listed once, traded at every market. A
  good has a base `price` (1 to 1,000,000), the units each kind of market
  (`town` or `village`) consumes on a tick as `demand`, and optionally the
  good it is made from as `input`.
- **Producers** are kinds such as fields, flocks or looms, named in the
  world's language: what one unit `yields` and `consumes` on a tick.
- **Markets** are locations, at most one each. A market's `kind` chooses its
  demand, its `producers` say how many of each it has, and its starting
  `prices` are indices (1,000 when left out) within `index_bounds`. A market
  with a `merchant` trades only while that character is present and
  undefeated, so a merchant's hours or travels close it. The merchant must
  be placed at the market or move among locations that include it. A market may replace the economy's
  `spread_percent`.
- **Links** join two markets whose prices pull together; one market's links
  share at most 100 percent in total.
- **Wares** are items a market sells at a fixed price, listed as
  `"wares": [{ "item": "healing_draught", "price": 8 }]`. They are buy-only,
  never run out and never move the market's prices. Equipment bought this
  way arrives as separate pieces. A ware's price is 1 to 1,000,000
  (`invalid_price`), and it is sold once per market and is not also a good
  (`invalid_ware`). Wares follow the market's merchant: no merchant, no sale.

Buying one unit costs `price × index × (100 + spread) / 100,000`, at least 1;
selling one fetches `price × index × 100 / (1,000 × (100 + spread))`, each
rounded down once. Units trade one at a time: each bought unit raises the
market's index by `trade_step` and each sold one lowers it by the same,
within the bounds, so dumping a whole cargo in one town stops paying.
`buy <item> [units]` and `sell <item> [units]` trade up to 1,000 units at
once, all or nothing; `market` shows the prices here.

Trading back and forth never pays, in either order. One step moves the index
both ways, so trades that end holding what the player started with leave the
index where it was. Validation requires every spread (the economy's and each
market's) to satisfy (100 + spread)² × lowest index ≥ 10,000 × (lowest index
+ `trade_step`) (`invalid_spread`). With bounds from 100 and a step of 26,
that is a spread of at least 13%. Separate buy and sell steps would let a
large stack bought or sold at one price come back at a profit, so there is
one step.

With a `tick` (which needs the time block), prices move on its schedule in
four phases, each finished for every market before the next:

1. **Supply.** Production is the market's producers' yields; consumption is
   its kind's demand plus what its producers consume. Producers make do with
   less of a dear good: while its index is above 1,000, what they consume of
   it is scaled by 1,000 ÷ index, rounded down once, so an industry short of
   its input does not drive the price to the bound. A surplus lowers the
   index by a draw below `supply_step` × the surplus, multiplied by index ÷
   `damp_below` while the index is below `damp_below`; a shortage raises it by
   a draw below `supply_step` × the shortage. The result stays within the
   bounds. Draws come from the economy's own `market` random stream, in market
   order, then goods order, only where supply is unbalanced.
2. **Revert.** The gap between the index and 1,000 shrinks by
   `revert_percent`, rounded towards zero.
3. **Inputs.** A good whose input is dearer moves `input_pull_percent` of the
   gap up towards it, measured on the phase-2 prices.
4. **Links.** Both markets of a link move `percent` of their gap towards each
   other, measured on the phase-3 prices and applied together.

Without a tick, only trade moves prices, and the world draws nothing for
them. The engine never warms prices up: the package authors starting indices.
`python3 -m scripts.combat_sim economy <world> --ticks 30 --prices` runs the
tick offline and prints indices to author, and the engine's tests pin numbers
that simulator produces. Saves keep the currency and every index; trade goods
may be carried in any number.

Validation keeps every authored number small enough that no tick or trade
can overflow: at most 10,000 producers of a kind, units per producer and
demand; at most 100,000,000 of a good made or used by one market per tick
(counted at the base price, where producers use the most, with a town's
villages and the highest demand percentage);
index bounds that contain 1,000 within 1 to 100,000.

The parts below are each optional and independent: a world authors only the
blocks it wants, and without one there is no state, menu entry or event for
it. Prosperity, stock and workshops each need the time block and run on
their own schedule. When several fall due at the same minute, they go in
this order: the price tick, prosperity, restocking, then workshop
settlement.

### Villages and their town

A village market may name its market town with `"town": "greyford"`. On
the price tick the village's production and demand count towards the town's
supply as well as its own, each at its own index and prosperity. Only a
village names a town, and the town must be a town market (`invalid_town`).
The two converge only through an authored link.

### Prosperity

```json
"prosperity": {
  "schedule": { "at": 1440, "every": 1440 },
  "base": 50,
  "scarce_above": 1200,
  "scarcity": 10,
  "demand_percent": [80, 120]
}
```

Every market has a prosperity from 0 to 100, starting at its own
`"prosperity"` value or at `base`. On each occurrence it moves one point
towards an ideal: `base`, lowered by `scarcity` for every good the market
needs whose index is at or above `scarce_above`, and never below 0. A market
needs a good that its kind demands or its producers consume. Prosperity
draws nothing.

`demand_percent` scales the market kind's demand on the price tick. It is
the percentage at prosperity 0 and at prosperity 100, linear in between:
`demand × (lo × (100 − p) + hi × p) ÷ 10,000`, rounded down once.
Producers' own consumption is not scaled. Left out, it is `[100, 100]`.

### Stock

```json
"stock": {
  "schedule": { "at": 1440, "every": 1440 },
  "units": 40,
  "currency": 600,
  "prosperity_percent": [50, 150]
}
```

With stock, each market holds units of every good and a purse:

- Buying takes units from the stock and refuses more than it holds. The
  menu marks an empty good `[sold out]`.
- Selling is paid from the purse and refuses a sale it cannot cover.
- A ware's price goes into the purse.
- A purse holds at most the currency bound; what would pass it is let go,
  so a full purse never stops the player buying.

On each occurrence every market is restocked, in market order:

- The purse becomes `currency`.
- Each good's target is a share of `units` by weight. A good's weight is
  what the market (and its villages) makes of it × 1,000 ÷ its index,
  rounded down, so cheap local goods fill the shelves.
- Each good with a target `t` draws its stock below `2t + 1` from its own
  `stock` random stream. A good with no target gets none and draws nothing.

With prosperity, `units` and `currency` are scaled by `prosperity_percent`
like demand; without it, that field is ignored with a warning
(`unused_percent`). A new game starts every market at its targets, drawing
nothing. The scaled units stay within 500,000 and the scaled purse within
the currency bound. Restocking never moves prices, and the price tick's
draws are the same with or without stock.

### Workshops

```json
"workshops": {
  "schedule": { "at": 10080, "every": 10080 },
  "limit": 1,
  "kinds": [
    { "id": "weavery", "name": "Weavery", "good": "cloth", "output": 6,
      "inputs": { "wool": 9 }, "overhead": 120, "price": 150, "resale": 75 }
  ]
}
```

The player buys workshops through dialogue:

- `buy_workshop` pays the kind's `price` for one in the town market where the
  player stands. It is refused in a village or elsewhere, past `limit`
  workshops of any kinds in that town, or without the money.
- `sell_workshop` sells one back for its `resale`.

On each occurrence, every workshop settles at its town's current indices,
with no spread:

- Its net is the value of `output` units of `good`, minus the value of its
  `inputs`, minus `overhead`.
- The value of `n` units is `price × index × n ÷ 1,000`, rounded down once.
- All nets are summed. A gain is received up to the currency bound, and
  anything past it is reported as forgone; a loss is paid from what the
  player holds, and anything left over is reported as a shortfall.
- Workshops never move prices.

Validation:

- `good` and `inputs` must be trade goods, each 1 to 10,000 units.
- The limit is 1 to 100 (`invalid_limit`).
- `resale` is at most `price` (`invalid_resale`), so buying and selling
  back never pays.

### Proficiencies

A proficiency is a rank the player gains, used by the capability that
defines it. The economy defines `trading`:

```json
"trading": { "name": "Trade", "max": 3, "narrow_percent": 4 }
```

- Each rank takes `narrow_percent` off every spread: `spread × (100 −
  narrow_percent × rank) ÷ 100`, rounded down. With 15% and rank 1, that is
  14%.
- `max` is 1 to 100, and `narrow_percent × max` is at most 100.
- The round-trip check above applies to the narrowest spread, at `max`.

Ranks come from two places:

- **Points.** A level entry may grant `"proficiency_points"`, the first
  level's being the starting pool. The player spends them with
  `train trading [points]`. Levels that grant points in a world with no
  proficiency are `proficiencies_disabled`, and levels that grant more in
  all than the proficiencies' `max` ranks add up to are `invalid_points`.
- **Effects.** `raise_proficiency` teaches ranks without using points; its
  `ranks` is 1 to `max`. Like a technique grant, it teaches only the ranks
  that fit under `max` and nothing once there, so a lesson is never
  refused.

Taught ranks can take the place of points: points that no proficiency has
a rank left for can never be spent, and are not shown as unspent. Saves
keep each rank's trained and taught parts. Unspent points are derived from
the level, and a save holds taught ranks only in a world with an effect
that teaches them, and workshops only of kinds an effect sells.

## Characters

Everyone in the world, including the player, is one entry in `characters.json`.
Talking and fighting are optional components:

```json
[
  { "id": "you", "name": "You", "description": "A stranger in Ashbell." },
  { "id": "elder", "name": "Elder Mara", "description": "…", "dialogue": "mara" },
  {
    "id": "wolf",
    "name": "The Ash Wolf",
    "description": "…",
    "combat": {
      "stats": { "hp": 20, "patk": 8, "pdef": 4, "satk": 0, "sdef": 4, "speed": 110 },
      "xp": 10,
      "loot": [{ "item": "ash_pelt", "quantity": 1 }]
    }
  }
]
```

`world.player` names the player character. It has no dialogue or combat profile
and is placed nowhere; in a combat world its numbers come from the level table.
A location's `characters` list places the others. A placed character is present
while its `requires` condition holds; the player can talk to it if it has a
`dialogue` and attack it if it has a `combat` profile: its `stats` (see
[Stats](#stats-damage-and-skills)), the `xp` granted on defeat, optional `loot`,
optional `skills` (skill IDs, usable once the profile's `level` reaches each
skill's unlock level), `basic_channel` (`physical` by default), `level`
(default 1; it also scales the XP the character grants) and an optional
`group`. A defeated character is gone: it is no longer listed and cannot be
talked to. A character with a combat profile is one instance: it may be placed
at most once, and its defeat is permanent (no respawns) unless its group is
repeatable. Other
characters may appear at several locations. Combat profiles require the world's
`combat` block.

A character with `"kind": "feature"` is a thing rather than a person, such as
an abandoned camp or a ring of stones. Examining it opens its dialogue, whose
lines describe what is seen; clients offer "Examine" instead of "Talk to".
A feature needs a dialogue and has no combat profile, army or movement
(`invalid_feature`). `kind` defaults to `person`.

## Start questions

`start_questions` in `world.json` are asked at New Game, before the first
turn, such as the player's background. Every question is asked, in order, and
an answer never skips or adds a question.

```json
"start_questions": [
  {
    "id": "errand",
    "name": "Errand",
    "text": "What brings you to the archive?",
    "options": [
      {
        "id": "scholar",
        "text": "I study the old river and its maps.",
        "effects": [{ "kind": "set_flag", "flag": "river_scholar" }]
      },
      { "id": "reader", "text": "Only to read." }
    ]
  }
]
```

- `name` is a short label for the answer once given; the status panel shows
  `Errand: Only to read.`
- Each question needs at least one option (`empty_start_question`); question
  IDs, and option IDs within a question, are unique.
- An option's `effects` apply in order to the starting state: `set_flag`,
  `grant_items`, `grant_currency`, `grant_technique` (with XP, since an answer
  is given once) and `raise_proficiency`. Quests, `take_items`,
  `pay_currency` and workshops are refused (`invalid_effect`): no giver or
  market is at hand before play begins. The starting currency plus the
  largest currency grant of each question must stay within `CURRENCY_BOUND`
  (`start_overflow`).
- Later conditions and text read what an answer produced like any other
  state. Saves keep the chosen option IDs, and a save must answer every
  question with one of its options.

Start questions shape the initial state; they are not identity editing, and
nothing re-runs them once play begins.

## Evidence

`evidence` in `world.json` defines what the player can find out in an
investigation: observations, testimony or physical clues.

```json
"evidence": [
  {
    "id": "displaced_cargo",
    "name": "Displaced cargo",
    "description": "The bandits' crates carry the lost wagons' seal.",
    "item": "caravan_seal"
  }
]
```

- The `discover_evidence` effect makes evidence known for good, reported as
  `EvidenceDiscovered`; discovering it again does nothing. The `evidence`
  condition reads it.
- `item` optionally links the evidence to a physical item. Carrying the item
  does not make the evidence known, nor does knowing the evidence give the
  item: possession and recognition are separate transitions.
- Authored conditions decide what a combination of evidence supports, such as
  two readings out of three under `any` of `all`s; there is no inference
  engine.
- A condition on evidence that no effect discovers is an error
  (`undiscoverable_evidence`). Start answers and scheduled events cannot
  discover evidence (`invalid_effect`).

## Story phases, main quests and outcomes

`phases` in `world.json` lists the story's phases in order. The first is
current at the start; the `enter_phase` effect moves the story on to a later
phase, passing any between, and is ignored for the current or an earlier
one. A phase changes only through story effects: world time, waiting and
resting never move it. Every phase after the first must be entered by some
effect (`unreachable_phase`); start answers and events cannot enter phases.

```json
"phases": [
  { "id": "road", "name": "The Road to Thornwick" },
  { "id": "leads", "name": "The Buried Road" }
]
```

A quest with `"main": true` belongs to the route's main questline; others are
side quests. A quest's optional `requires` condition must hold to take it up
(refused with `QuestLocked`), which is how quests chain and how side quests
unlock in waves by phase. Validation rejects a quest that waits on itself
through its prerequisites (`quest_cycle`) and a main quest that requires a side
quest's progress on every branch (`main_requires_side`); a side quest may still
be one alternative under `any`. Unchosen quests simply stay available: there
is no failure for a lead not taken.

`outcomes` are the route's authored endings:

```json
"outcomes": [
  {
    "id": "first_chapter",
    "name": "The first chapter closes",
    "text": "You have chosen your road.",
    "when": { "kind": "flag", "flag": "lead_chosen" }
  }
]
```

After each command, if no outcome has been recorded, the outcome whose `when`
now holds is recorded for good and reported as `OutcomeReached`; play goes on.
Two holding at once refuse the command (`AmbiguousOutcome`); two outcomes
with the same `when` are rejected at load (`ambiguous_outcomes`). No outcome may
hold at the start: each `when` must require, on every branch, something no
start provides, such as a quest taken up, evidence, a workshop, a phase after
the first or a flag no start answer sets (`outcome_at_start`). Terminal and failure
endings are not yet modelled.

The journal (`Engine::journal`, the CLI's Quests panel) shows the current
phase, main quests then side quests, leaving out available quests whose
`requires` does not hold yet, the evidence known and the outcome reached.

## Dialogue and quests

Each dialogue has a `start` node ID and a `nodes` array. A node has authored
`text` and optional `choices`. Each choice has authored `text`, an optional
`requires` condition, optional `next`, and an optional list of typed
`effects`:

```json
{ "kind": "accept_quest", "quest": "quiet_the_track" }
{ "kind": "complete_quest", "quest": "quiet_the_track" }
{ "kind": "set_flag", "flag": "hall_open" }
{ "kind": "grant_technique", "technique": "cloud_palm", "rank": 1, "xp": 0 }
{ "kind": "grant_items", "items": [{ "item": "pen", "quantity": 1 }] }
{ "kind": "take_items", "items": [{ "item": "pen", "quantity": 1 }] }
{ "kind": "grant_currency", "amount": 60 }
{ "kind": "pay_currency", "amount": 150 }
{ "kind": "buy_workshop", "workshop": "weavery" }
{ "kind": "sell_workshop", "workshop": "weavery" }
{ "kind": "raise_proficiency", "proficiency": "trading", "ranks": 1 }
{ "kind": "discover_evidence", "evidence": "wheel_ruts" }
{ "kind": "enter_phase", "phase": "lost_wagons" }
```

Effects apply in authored order to the staged state, and the choice commits
or fails as a whole: if any effect is refused, such as taking items the player
does not carry or completing a quest that is not ready, nothing the earlier
effects did is kept. `grant_items` gives items as quest rewards do (equipment
arrives as individual pieces); `take_items` hands over counted items only.
`grant_currency` and `pay_currency` need the economy; paying more than the
player has refuses the choice. `buy_workshop` and `sell_workshop` act in the
town where the player stands ([Workshops](#workshops)), and
`raise_proficiency` teaches ranks without spending points, up to the top
rank ([Proficiencies](#proficiencies)). Events cannot
use these three.
A choice can be taken again while its condition holds, so a one-time gift
pairs `grant_items` with `set_flag` under a `not` condition on that flag.

Choices are filtered and then numbered contiguously from one. Omitting `next`
ends the conversation. A node with no visible choices displays its text and
ends the conversation. Moving, attacking, using a skill or resting also closes
the conversation.

A quest's `giver` must be a character with a dialogue. Its objective is one of:

```json
{ "kind": "defeat", "character": "wolf" }
{ "kind": "flag", "flag": "map_found" }
```

A `defeat` objective needs the world's `combat` block and a placed character
with a combat profile; an active quest becomes ready when that character is
defeated. A `flag` objective names a declared flag; an active quest becomes
ready when the flag is set, so a world without combat can still have a main
questline. Accepting after the defeat or flag makes the quest ready immediately.
`reward_xp` defaults to 0 and must stay 0 in a world without combat.
`reward_techniques` lists technique grants (see [Techniques](#techniques)). Accept/complete actions require the available
giver in the player's current location, whether invoked by dialogue or a direct
command. Completion grants rewards once, sets flags, and emits stored completion
prose. A quest's own `requires` is its prerequisite; a dialogue choice that
accepts it should carry the same condition, or taking the choice is refused.

## Combat block, numeric rules and templates

The optional `combat` object in `world.json` holds the level table, skills and
combat prose:

```json
"combat": {
  "special_name": "Witchcraft",
  "cross_share": 25,
  "timeline": { "action_cost": 100000, "speed_cap": 200 },
  "resources": { "mp_regen_percent": 3, "rage_per_action": 1, "rage_per_max_hp": 20 },
  "levels": [
    { "xp": 0, "stats": { "hp": 34, "mp": 24, "patk": 3, "pdef": 4, "satk": 10, "sdef": 6, "speed": 100 } },
    { "xp": 12, "stats": { "hp": 40, "mp": 30, "patk": 3, "pdef": 5, "satk": 12, "sdef": 7, "speed": 100 } }
  ],
  "skills": [
    {
      "id": "bolt",
      "name": "Bolt",
      "power": 170,
      "channel": "special",
      "cost": 12,
      "text": "{attacker} loose a bolt of witchlight. {target} takes {damage} damage."
    }
  ],
  "player_skills": ["bolt"],
  "player_basic_channel": "physical",
  "narrative": {
    "attack": ["{attacker} swing the staff. {target} takes {damage} damage."],
    "hurt": ["{attacker} rakes {target} with a cold touch: {damage} damage."],
    "victory": "{target} concedes.",
    "death": "The duel is lost."
  }
}
```

### Stats, damage and skills

Every level entry and combat profile has seven `stats`: `hp`, `mp` (optional,
default 0), physical attack and defence `patk`/`pdef`, special attack and
defence `satk`/`sdef`, and `speed`. Each is at most 9,999, and HP and speed are
at least 1. Speed sets how often a character acts in an encounter.

Level entries supply cumulative `xp` and the player's stats at that level. The
first entry requires zero XP; thresholds strictly increase; no stat decreases
between levels. Saves hold only current HP and MP, XP, level and spent stat
points; every other stat is derived. Levelling up restores HP and MP fully, to
the effective maxima.

### Stat points

A level entry may grant `points` on reaching it (the first level's are the
starting pool), and the combat block's `stat_points` says what they buy:

```json
"stat_points": {
  "values": { "hp": 5, "patk": 1, "pdef": 1, "speed": 2 },
  "caps": { "speed": 10 },
  "respec": "safe"
}
```

`values` lists every stat that accepts points and what one point adds; `caps`
optionally limits the points one stat may take; `respec` is `never` (the
default) or `safe`, which refunds every point at a safe location. Points and
`stat_points` come together, and even every point in one stat, up to its cap,
must keep that stat within 9,999 at every level. The player spends points
outside encounters (`allocate <stat> [points]`); a gained maximum HP or MP is
gained now too. Effective stats are the level table plus spent points, derived
whenever needed and never saved.
HP/damage use `u32`; XP and item counts use `u64`. Overflow refuses the whole
command with no partial rewards or state.

There are two damage channels, physical and special. `special_name` (required,
non-empty) is the world's name for special — magic, 内力, mana — and clients
show it wherever "special" would appear. A hit uses the combined attack `A` and
combined defence `D` of its channel: 100 × the channel's stat plus
`cross_share` (0–100, default 25) × the other channel's stat. Then

```text
damage = max(1, floor(A × power × A / (100 × 100 × (A + D))))
```

capped at the target's remaining HP. Defence equal to the combined attack
halves damage; no scale constant or character level enters.

A skill has an `id`, `name`, `power` (1–1,000; a basic attack is 100), a
`channel`, a `cost` (default 0, spent exactly as authored) in its `resource`
(`mp`, the default, or `rage`), an action `time` in percent of a basic attack
(default 100), the `level` at which the player can use it (default 1), an
optional `cross_share` overriding the world's, and its own `text` template
(`{attacker}`, `{target}`, `{damage}`). `player_skills` lists the player's skills; `player_basic_channel`
and a profile's `basic_channel` set the channel of each character's basic
attack. Every usable skill and basic attack needs attack in its channel, and
every MP skill must be affordable with the MP its user has when it unlocks;
rage builds up during a fight, so rage costs have no such ceiling.

### Techniques

A technique is mastered rank by rank, and the author names every rank in the
source's own terms; players see that name, never a number:

```json
"techniques": [
  {
    "id": "azure_breath",
    "name": "Azure Breath",
    "xp_share_percent": 20,
    "ranks": [
      { "name": "First Layer", "xp": 0, "passive": { "mp": 10, "satk": 2 } },
      { "name": "Second Layer", "xp": 10, "passive": { "mp": 20, "satk": 4 } },
      { "name": "Third Layer", "xp": 30, "passive": { "mp": 35, "satk": 7 },
        "requires": { "kind": "flag", "flag": "scripture_found" } }
    ]
  },
  {
    "id": "cloud_palm",
    "name": "Cloud Palm",
    "ranks": [
      { "name": "Drifting Cloud", "xp": 0, "skill": "palm_drifting" },
      { "name": "Gathering Storm", "xp": 30, "skill": "palm_storm" }
    ]
  }
],
"player_techniques": [{ "technique": "azure_breath" }],
"core_art": "azure_breath",
"technique_xp_per_use": 10
```

- **Ranks** have cumulative technique-XP thresholds starting at 0. A rank may
  name the `skill` the technique is used as at that rank (an ordinary skill,
  which then joins the player's usable skills) and a `passive` stat bonus: the
  technique's whole bonus at that rank, replacing the previous rank's. An
  internal art is a technique with passives and no skill.
- **Training.** Each use of a technique's skill in an encounter earns
  `technique_xp_per_use` (default 10), with the same level falloff as character
  XP. Each victory also gives every learned technique its `xp_share_percent` of
  the character XP earned; keep it small so internal arts deepen slowly.
- **Gates.** A rank's `requires` condition is a breakthrough gate: technique
  XP waits at that rank's threshold until they hold, and the technique rises
  as soon as they do.
- **Grants** (`player_techniques`, the `grant_technique` dialogue effect and
  quest `reward_techniques`) teach a technique if unknown, raise it to at least
  `rank` (teaching passes gates), then add `xp`. A dialogue choice can be taken
  again, so dialogue grants carry no XP; one-time XP comes from quest rewards.
  A skill belongs to at most one technique.
- **Realm.** `core_art` names the technique whose current rank name is shown as
  the player's realm.

Effective stats are the level table, spent stat points and learned
techniques' current rank bonuses, derived whenever needed. Even every rank's
largest bonus together with every stat point must keep each stat within 9,999.

### Equipment

The combat block declares the world's `slots` and the player's starting gear;
an item with an `equipment` part can be worn:

```json
"slots": ["main_hand", "off_hand", "body", "neck"],
"player_equipment": ["practice_sword", "buckler", "leather_vest"]
```

```json
{
  "id": "greatsword",
  "name": "Greatsword",
  "description": "It needs both hands and a moment to swing.",
  "equipment": {
    "slots": ["main_hand", "off_hand"],
    "bonuses": { "patk": 8 },
    "basic_time": 130
  }
}
```

- `slots` lists the slots a piece occupies (a two-handed weapon takes two);
  `bonuses` adds to stats while worn; `speed_penalty` subtracts from speed;
  a weapon's `basic_channel` and `basic_time` replace the wearer's basic attack
  (pieces that set either must share a slot, so only one is ever worn);
  `modifiers` scale damage taken on a channel by `num / den` (0–10 over 1–10:
  `0/1` immunity, `1/2` resistance, `2/1` vulnerability).
- Each piece obtained (starting gear, loot, quest rewards) is an individual
  item with its own number; other items keep counts. Starting gear is worn in
  order while its slots are free.
- The player wears pieces outside encounters (`equip <#>`); whatever held the
  slots returns to the pack. Worn bonuses are part of effective stats, derived
  and never saved.
- Several modifiers on one channel multiply, any immunity means no damage at
  all, and the product stays within 1/10 to 10. Speed penalties add up and
  apply before the speed cap; speed never drops below 1. The best piece for
  every slot, with levels, points and technique bonuses, must keep each stat
  within 9,999.

### Consumables

An item with a `consumable` part restores HP and MP when used:

```json
{
  "id": "healing_draught",
  "name": "Healing draught",
  "description": "Bitter, green, and better than bleeding.",
  "consumable": { "hp": 30 }
}
```

- `hp` and `mp` each default to 0. At least one is positive, both are at
  most 9,999, a consumable is not also equipment, and it restores MP only
  where some level, stat point, technique or piece can give the player MP
  (`invalid_consumable`).
  A world without combat has no HP or MP to restore (`combat_disabled`).
- `use <item>` spends one unit and restores up to the effective maxima. A
  use that would restore nothing is refused, and nothing is spent.
- In an encounter a use is the player's turn: it takes one basic action's
  time and the opponents act before the player's next turn. It earns no rage
  and no technique XP.
- A consumable may also be a trade good or a ware, so food bought cheaply in
  one place heals in another.

### Forging and improvement

A location can offer crafting `stations`, and the combat block lists
`recipes` that forge a wearable item at one of them:

```json
"stations": ["anvil"]
```

```json
{
  "id": "iron_sword",
  "station": "anvil",
  "inputs": [{ "item": "iron_ingot", "quantity": 2 }],
  "output": "iron_sword",
  "known_when": { "kind": "flag", "flag": "taught_forging" },
  "requires": { "kind": "technique", "technique": "smithing", "rank": 1 },
  "trains": { "technique": "smithing", "xp": 10 }
}
```

An item's `equipment` can list improvement `tiers`, in order:

```json
"tiers": [{
  "name": "Fine {item}",
  "bonuses": { "patk": 6 },
  "speed_penalty": 5,
  "station": "anvil",
  "cost": [{ "item": "iron_ingot", "quantity": 1 }],
  "requires": { "kind": "technique", "technique": "smithing", "rank": 2 }
}]
```

- A recipe is hidden until `known_when` holds (a teacher, a found plan or a
  quest sets its flag), then listed; it can be used once `requires` holds,
  and the menu says what is missing. Proficiency is an ordinary technique,
  such as Smithing with named ranks; `trains` gives it XP on each success,
  and teaches it if unknown. It may not set a rank.
- `forge <recipe>` at the station spends the inputs and makes one new piece.
  `improve <#>` raises a piece one tier at that tier's station. A tier's
  `bonuses` replace the item's (or the previous tier's) whole, and its
  `speed_penalty`, when set, replaces the penalty; slots, weapon overrides
  and modifiers stay. The piece is named by the tier's template, where
  `{item}` is the item's name, so word order can follow the world's language.
- Every check (station, requirements, materials) runs before anything
  changes; a refusal spends nothing. Results are guaranteed, with no rolls.
  Nothing is forged during a fight.
- Materials are ordinary counted items, each listed once per recipe or tier;
  equipment cannot be a material. The best tier of every piece counts toward
  the 9,999 worst case, and a weapon's basic-attack channel must keep an
  attack at every tier. Stations need a combat block.

### Enchanting

The combat block can list `enchantments`, each laid on a piece at a station:

```json
{
  "id": "keenness",
  "name": "{item} of Keenness",
  "slots": ["hand"],
  "bonuses": { "patk": 2 },
  "station": "altar",
  "catalyst": [{ "item": "ember_shard", "quantity": 1 }],
  "known_when": { "kind": "flag", "flag": "taught_enchanting" },
  "requires": { "kind": "technique", "technique": "enchanting", "rank": 1 },
  "trains": { "technique": "enchanting", "xp": 10 }
}
```

- An enchantment fits any piece occupying one of its `slots`. `enchant <#>
  <enchantment>` at the station spends the catalyst and lays it on the piece
  for good: a piece takes one enchantment, never replaced or removed.
- Its `bonuses` add to the piece's at whatever tier it is, and improving the
  piece keeps them. The name template applies after the tier's, so `{item}`
  is "Fine Iron sword" and the piece becomes "Fine Iron sword of Keenness".
- Knowledge, requirements, training and the order of checks work as for
  recipes. Effects are passive stat bonuses only. The best enchantment that
  fits each piece counts toward the 9,999 worst case.

### Encounters

`engage` starts an encounter with a fighter at the player's location; it then
owns everyone's HP and MP until it ends. Each participant acts on a paused
initiative timeline: an action of `time` percent delays its actor's next turn
by

```text
delay = max(1, ceil(action_cost × time / (100 × min(speed, speed_cap))))
```

ticks (speed at least 1). Everyone's first turn follows one opening delay, so a
faster opponent may act before the player's first command, and a much faster
one may act several times in a row. Ties go to the player's side, then to
participant order. Each command resolves the player's attack or skill, then
every opponent's turn until the player's next turn or the end; an opponent uses
its strongest affordable skill (equal power prefers the cheaper one, then the
later tier), else its basic attack. Only attacks, skills and panels work during
a fight. `action_cost` (1 to 10^12) only sets integer precision; 100,000 gives
every speed up to 255 its own delay. Clients may show the next few turns, labelled
as a projection that assumes basic-action times.

MP regenerates during encounters at `mp_regen_percent` of maximum MP per
baseline turn (one basic action at speed 100) of elapsed time, carrying the
fraction; time at full MP banks nothing. Rage starts at 0 in every encounter,
grows by `rage_per_action` after each of the actor's own actions (so an actor
one point short cannot spend what its own action earns), and by
`rage_per_max_hp` for taking damage equal to its maximum HP, proportionally and
cumulatively. Rage never outlasts the encounter; `resources` defaults to zero,
which leaves either resource unused.

The combat block's `groups` shape encounters:

```json
"groups": [
  { "id": "pack" },
  { "id": "warren", "repeatable": true },
  { "id": "spar", "yield_share": 50, "victory_flags": ["spar_won"], "defeat_flags": ["spar_lost"] },
  { "id": "pit", "no_flee": true }
]
```

Engaging a fighter whose profile names a `group` brings in every member placed
at the location, present under its conditions and not defeated, in the
location's order; an ungrouped fighter comes alone. Opponents target the first
opponent still fighting.

`flee` spends the player's turn: opponents act until the player's next turn,
and if the player is still alive then, the encounter ends with nothing granted
or recorded; the opponents are whole again next time. A `no_flee` group
forbids it.

In a group with a `yield_share` (1–100), nobody dies: a hit stops at 1 HP, and a
participant at or below `max(1, ⌊max HP × share / 100⌋)` yields and leaves the
schedule alive. The player yielding ends the encounter as a loss: the group's
`defeat_flags` are set and play continues with the HP left. A defeat objective
cannot target a member of a yielding group.

The encounter is won when every opponent has died or yielded. The player's HP
and MP return to exploring, and each opponent that died grants its loot and XP
and advances defeat objectives. Its XP scales by level difference: ±10% per
level of the opponent's level above or below the player's, capped at ±40% and
rounded down, and nothing from an opponent five or more levels below. It is
recorded as defeated unless its group is `repeatable`, which can be fought
again at once and rewards every victory (a defeat objective counts the first).
Yielded opponents grant and record nothing. A victory sets the group's
`victory_flags`. The player dying is ordinary death.

### Critical hits

A skill's `crit`, the combat block's `player_basic_crit` and a profile's
`basic_crit` let a landed hit deal more damage:

```json
"crit": { "chance_percent": 25, "multiplier_percent": 150 }
```

The chance is 1–100% and the multiplier 101–1,000% of a normal hit, applied
inside the damage formula before its single rounding. A world that authors any
crit keeps a seeded `combat` stream in its saved state (SplitMix64, versioned),
and a world with characters who move keeps a separate `world` stream; each
starts from the seed mixed with its own constant, so draws in one never shift
the other. A world with neither keeps no random state and plays exactly as it
did. A draw happens only
when an action with a crit resolves, so a refused command never consumes one,
and the same seed and commands always replay identically. Clients choose the
seed (the CLI takes `--seed`, else uses the clock, and prints it).

A location with `"safe": true` lets the player `rest` outside encounters,
restoring HP and MP; it needs the combat block.

`attack` and `hurt` each require at least one template; they narrate basic
attacks, while skills use their own `text`. They allow
`{attacker}`, `{target}`, `{damage}`. The `victory` template allows `{target}`;
`death` is plain text. Example:

```json
"{attacker}一剑刺向{target}，造成{damage}点伤害。"
```

Placeholders use exact ASCII keys even for non-English text. Unknown or
unbalanced placeholders are validation errors; brace escaping is not supported
in templates yet. Plain prose fields are not interpolated. Substitution is
single-pass: a name containing `{damage}` remains a literal name.

Format 17 currently selects combat prose variants from the current
`state.turn % variant_count` value using the turn before the attack. Failed
commands do not advance `state.turn`, and presentation-only inspection commands
(`look`, inventory, status and quests) also do not advance it. Other successful
gameplay commands may therefore affect which variant a later attack selects.
Variant selection uses no randomness.

This remains a current implementation detail, not a content contract authors
should depend on. A later combat implementation may keep deterministic selection
while keying variants to a more local gameplay/narrative sequence (for example an
encounter-local attack sequence) if stronger semantic independence is useful.

## Troops and battles

The optional `troops` block in `world.json` (it needs `combat`) gives the
player soldiers; the optional `battle` block (it needs `troops`) lets them
fight authored armies.

```json
"troops": {
  "classes": [
    { "id": "foot", "name": "Foot" },
    { "id": "archers", "name": "Archers", "ranged": true },
    { "id": "riders", "name": "Riders", "mounted": true }
  ],
  "lines": [
    {
      "id": "levy",
      "class": "foot",
      "levels": [
        { "xp": 0, "name": "Levy", "wage": 1, "stats": { "hp": 20, "patk": 6, "pdef": 4, "satk": 0, "sdef": 2, "speed": 100 } },
        { "xp": 6, "stats": { "hp": 22, "patk": 7, "pdef": 5, "satk": 0, "sdef": 2, "speed": 100 } },
        { "xp": 10, "name": "Spearman", "wage": 2, "stats": { "hp": 26, "patk": 9, "pdef": 6, "satk": 0, "sdef": 2, "speed": 100 } }
      ],
      "upgrades": [{ "to": "bowmen", "cost": 20 }, { "to": "riders", "cost": 40 }]
    }
  ],
  "limit": 30,
  "wounded_percent": 50,
  "upkeep": { "schedule": { "at": 1440, "every": 1440 }, "recover_percent": 50, "desert_percent": 20 }
}
```

- **Classes** are the world's own: ranged troops shoot from behind the
  line, mounted ones can flank; a class is one, the other or neither
  (`invalid_class`).
- **Lines** are ladders of levels. Each level sets stats and the XP each
  soldier needs to rise into it; only the first needs none, and it must be
  named (`invalid_line`). A level may rename the soldier or change the wage,
  and otherwise keeps the previous level's. `channel` (physical by default)
  is the line's attack channel. `upgrades` are branches a last-level soldier
  can take into another line's first level, for a `cost` in currency each.
- **Squads.** The roster keeps one squad per line and level, whose soldiers
  share an XP pool. A squad whose share (the pool divided by its healthy and
  wounded soldiers) covers the next level rises together, paying for it and
  carrying the rest. A soldier who leaves (upgraded, killed, deserting) takes
  their share. `upgrade <line> <to> [n]` turns last-level soldiers into a
  branch, each carrying its share.
- **The roster limit** counts healthy and wounded soldiers, not the player
  (1 to 10,000, `invalid_limit`).
- **Upkeep** (needs world time) pays every soldier's wage at once on its
  schedule, or, when the currency does not cover it, pays nothing and loses
  `desert_percent` of each squad (at least one, the healthy first). Then
  `recover_percent` of each squad's wounded mend, rounded up. Resting at a
  safe place mends them all. Wages without upkeep draw a warning
  (`unused_wages`).

A location may offer recruits, each line from a pool that starts full and
refills on a schedule (needs world time):

```json
"recruits": {
  "troops": [{ "line": "levy", "price": 10, "size": 8 }],
  "refill": { "schedule": { "at": 1440, "every": 1440 }, "amount": 2 }
}
```

`recruit <line> [n]` takes level-1 soldiers from the pool for their price,
within the roster limit.

```json
"battle": {
  "frontage": 12,
  "rounds": 8,
  "roll": [90, 110],
  "matchups": { "riders": { "archers": 150 }, "foot": { "riders": 120 } },
  "morale": { "factor": 150, "floor": 50, "rout": 30 },
  "hold_percent": 60,
  "flank_percent": 130,
  "pursuit_class": "riders",
  "player_xp_percent": 30
}
```

A character may lead an army instead of fighting alone (`invalid_army`):

```json
"army": {
  "troops": [{ "line": "outlaws", "count": 8 }, { "line": "outlaws", "level": 2, "count": 2 }],
  "xp": 60,
  "loot": [{ "item": "eels", "quantity": 4 }],
  "repeatable": true
}
```

An army with a `joins` condition is an ally instead: it is never engaged,
and it joins the player's side in any battle fought where it is present
while the condition holds. Its XP, loot and repeatability do nothing
(`unused_reward`).

`engage <army>` starts a battle, which holds the player's HP and MP until it
ends. Side 0 is the player (one head, with effective stats), the roster's
healthy squads (lines as authored, levels from the highest) and any allies;
side 1 is the army. Each round the player orders `charge`, `hold`, `flank`
(with riders) or `retreat`, or `autoresolve` charges to the end; the enemy
always charges. A round, computed from the counts at its start for both
sides at once:

1. **Who fights.** `frontage` melee heads per side, filled in order; every
   ranged head shoots. On `flank`, mounted troops leave the line to attack
   the enemy's archers, if any stand.
2. **Targets.** The enemy's melee units, or its archers once no melee stands
   (or its archers, for flankers). A unit's fighting count is split across
   them by their counts, floors first, then one each to the largest
   remainders.
3. **Damage.** Count × the personal damage formula at power 100 in the
   attacker's channel, × the matchup (100 when absent) × the side's morale
   modifier (`floor` + (100 − `floor`) × morale ÷ 100) × the side's roll
   (one draw per round from the `battle` random stream) × `hold_percent` on
   melee hits while either side holds × `flank_percent` for flankers × 200%
   for the pursuit class in a pursuit, all ÷ 100⁶, rounded down once.
4. **Losses.** Damage carries over within a stack until it makes up a
   soldier's HP. The player loses HP and at 0 is knocked out, never killed.
5. **Morale** (from 100) falls by losses × `factor` ÷ the side's starting
   size, the remainder carried; a side below `rout` breaks.
6. **End.** A side that broke or has nobody standing loses, and the winner
   takes a pursuit round in which the loser deals nothing; both at once is a
   draw. After `rounds` rounds the weaker side (count × HP × attack, scaled
   by morale) withdraws, the player's side on a tie. A retreat gives the
   enemy a pursuit and the battle.

Afterwards `wounded_percent` of the player side's losses are wounded, the
rest killed, and the player is back to exploring on at least 1 HP. A victory
gives the player `player_xp_percent` of the army's XP and shares the rest
among the squads still standing by their healthy count, promotes them,
grants the loot, and records a non-repeatable army as defeated.
`python3 -m scripts.combat_sim battle <world> --army ID [--roster
line:level:count] [--ally ID] [--orders ...] --seed N` runs the same rule
offline.

## Validation feedback

`WorldSpec::diagnostics()` returns all detected content problems as serializable
`Diagnostic { severity, entity_id, code, message }` values in stable traversal
order. `validate()` returns `SpecError::Validation` with those diagnostics if
any are errors. `load()` validates before returning a playable world. File I/O
and JSON syntax/type errors retain the file path and underlying error.

Checks include version, IDs, references (in every leaf of a condition tree),
non-empty `all`/`any`, counted items in item conditions and `take_items`,
dialogue links/effects, declared flags,
the player character, quest givers and targets, combat content in worlds
without combat (`combat_disabled`), level rules, stat, power and share bounds,
skill references, usable and affordable skills, loot quantities, fighter
placement, template placeholders, evidence that can be discovered, phases
that can be entered, quest prerequisite cycles and main quests waiting on side
quests. `load()` reports a package whose
`format_version` is not 17 as `SpecError::UnsupportedFormat` before parsing it.
Checks do not yet analyze graph reachability, condition satisfiability,
never-set flags, narrative quality, or battle/quest solvability. Passing validation
means the engine can interpret the data, not that every route is winnable.
