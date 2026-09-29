# World package format 4

Format 4 makes combat optional. The level table and combat prose live in an
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
  `resources` if skills should regenerate MP or build rage in fights.

A world without combat only needs its `format_version` raised.

Format 4 represents one fixed player-controlled character and one playable
route. For persistence/API identity, RealmKit exposes this implicit route under the
stable logical route ID `default`; Format 4 does not serialize a route collection
or route field. Future formats may package a canonical route, an
original-character route, or both over the same shared world and canonical
timeline. When both are
present, New Game selects between them directly; one route does not unlock the
other. Each route has its own player binding, start state, main questline, outcomes and
mutable save while reusing shared locations, NPCs, factions and other world
definitions where appropriate. Future formats should evolve toward shared `Character` definitions plus a small
`PlayerSpec` route binding that identifies which character the human controls.
Do not duplicate a canonical person as separate player and NPC entities merely
because control differs by route.

In an original-character route, the canonical protagonist remains in the package
as a canonical world character/NPC rather than being replaced by the player.

Format 4 also requires item and quest tables because they serve the current demo.
Inventory is not a long-term universal requirement, but quest progression is:
future formats should generalize quests into main and optional side questlines
rather than remove them. A non-combat player route still has a main questline whose objectives may use
dialogue, exploration, investigation or other capabilities instead of combat. Investigation evidence is independent state and may optionally
reference a physical entity/item without being stored "inside" inventory or
inferred from possession.

A package is a directory containing these required UTF-8 JSON files:

| File | Content |
| --- | --- |
| `world.json` | Format version, world ID/name/language, starting location, player character ID, declared flags, optional `combat` block |
| `locations.json` | Array of locations with descriptions, directional exits and placed character IDs |
| `characters.json` | Array of characters with descriptions, availability conditions, and optional dialogue and combat profile |
| `items.json` | Array of items with names and descriptions |
| `quests.json` | Array of quests with giver, defeat or flag objective, prose, rewards and completion flags |
| `dialogues.json` | Array of dialogue trees with nodes, choices, conditions and effects |

Empty content tables are `[]`; files must still exist. Extra files such as
author notes or future provenance sidecars are ignored by the runtime loader.
Unknown fields inside the defined JSON structures are rejected to catch typos.
Version 2 describes this schema; incompatible changes require an explicit
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
machine-facing and may remain language-neutral ASCII. Format 4 does not yet carry
client locale strings; the M0 CLI therefore only fully satisfies this requirement
for English worlds.

## Locations and conditions

Exits are directed. To return along a path, author a separate reverse exit.
Directions are `north`, `south`, `east`, `west`, `up`, `down`.

The long-term presentation model distinguishes **spatial placement** from
**traversal connectivity**. The explicit exit graph is authoritative movement
state. Future formats may optionally group locations into Areas (for example a
city, one building floor or a wilderness region) and give locations area-local
integer `(x, y)` positions for map presentation.

Coordinates never create exits: adjacent cells need not be traversable, and an
explicit exit may connect locations that are not adjacent in the layout. For an
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
      "requires": [{ "kind": "flag", "flag": "hall_open" }],
      "blocked_text": "The hall is locked."
    }
  }
}
```

`requires` lists are AND conditions and default to empty. They can appear on
exits, characters and dialogue choices. A condition tests either a declared flag or a
quest state:

```json
{ "kind": "quest", "quest": "quiet_the_track", "status": "ready" }
```

Quest statuses are `available`, `active`, `ready`, `completed`. All flags start
unset. Dialogue `set_flag` effects and quest completion flags set them; flags
are monotonic in this version. Conditions govern availability/choice visibility.

This flat conjunctive representation is a Format 4 limitation. The long-term
condition model uses pure typed predicates composed with `All / Any / Not`;
predicates remain domain-specific and typed rather than becoming arbitrary
expressions/property paths. Typed effects execute in authored order as part of the
engine's atomic state transition.

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
while its `requires` conditions hold; the player can talk to it if it has a
`dialogue` and attack it if it has a `combat` profile: its `stats` (see
[Stats](#stats-damage-and-skills)), the `xp` granted on defeat, optional `loot`,
optional `skills` (skill IDs; profiles have no level yet, so every listed skill
is usable) and `basic_channel` (`physical` by default). A defeated character is gone: it is no longer
listed and cannot be talked to. A character with a combat profile is one instance: it
may be placed at most once, and its defeat is permanent (no respawns). Other
characters may appear at several locations. Combat profiles require the world's
`combat` block.

## Dialogue and quests

Format 4 has a single flat quest collection. The long-term model should retain
quests as core story progression but organize them into a main questline plus
optional side questlines. Questlines share world entities rather than owning
private copies of NPCs or locations. Side quest availability should be gated by
explicit main-story/story-phase conditions, and side outcomes may feed typed
state into later main-quest conditions.

Each dialogue has a `start` node ID and a `nodes` array. A node has authored
`text` and optional `choices`. Each choice has authored `text`, optional
`requires`, optional `next`, and an optional typed `effect`:

```json
{ "kind": "accept_quest", "quest": "quiet_the_track" }
{ "kind": "complete_quest", "quest": "quiet_the_track" }
{ "kind": "set_flag", "flag": "hall_open" }
```

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
`reward_xp` defaults to 0 and must stay 0 in a world without combat. Accept/complete actions require the available
giver in the player's current location, whether invoked by dialogue or a direct
command. Completion grants rewards once, sets flags, and emits stored completion
prose. Dialogue visibility conditions are not additional quest prerequisites.

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
between levels. Saves hold only current HP and MP, XP and level; every other
stat is read from the level table. Levelling up restores HP and MP fully.
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
a fight. `action_cost` only sets integer precision; 100,000 gives every speed up
to 255 its own delay.

MP regenerates during encounters at `mp_regen_percent` of maximum MP per
baseline turn (one basic action at speed 100) of elapsed time, carrying the
fraction; time at full MP banks nothing. Rage starts at 0 in every encounter,
grows by `rage_per_action` after each of the actor's own actions (so an actor
one point short cannot spend what its own action earns), and by
`rage_per_max_hp` for taking damage equal to its maximum HP, proportionally and
cumulatively. Rage never outlasts the encounter; `resources` defaults to zero,
which leaves either resource unused.

Defeating every opponent ends the encounter: the player's HP and MP return to
exploring, each defeated opponent's loot and XP are granted once, and defeat
objectives advance. A defeated character stays defeated. A location with
`"safe": true` lets the player `rest` outside encounters, restoring HP and MP;
it needs the combat block.

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

Format 4 currently selects combat prose variants from the current
`state.turn % variant_count` value using the turn before the attack. Failed
commands do not advance `state.turn`, and presentation-only inspection commands
(`look`, inventory, status and quests) also do not advance it. Other successful
gameplay commands may therefore affect which variant a later attack selects. No
runtime randomness is involved.

This remains a current implementation detail, not a content contract authors
should depend on. A later combat implementation may keep deterministic selection
while keying variants to a more local gameplay/narrative sequence (for example an
encounter-local attack sequence) if stronger semantic independence is useful.

## Validation feedback

`WorldSpec::diagnostics()` returns all detected content problems as serializable
`Diagnostic { severity, entity_id, code, message }` values in stable traversal
order. `validate()` returns `SpecError::Validation` with those diagnostics if
any are errors. `load()` validates before returning a playable world. File I/O
and JSON syntax/type errors retain the file path and underlying error.

Checks include version, IDs, references, dialogue links/effects, declared flags,
the player character, quest givers and targets, combat content in worlds
without combat (`combat_disabled`), level rules, stat, power and share bounds,
skill references, usable and affordable skills, loot quantities, fighter
placement and template placeholders. `load()` reports a package whose
`format_version` is not 4 as `SpecError::UnsupportedFormat` before parsing it.
Checks do not yet analyze graph reachability, condition satisfiability,
never-set flags, narrative quality, or battle/quest solvability. Passing validation
means the engine can interpret the data, not that every route is winnable.
