# World package format 1

Format 1 is the combat-enabled scaffold format used by the demo. Its mandatory
level table and narrative combat templates mean it does **not yet** represent the
long-term rule that combat is optional. A later compatible extension or format
version must allow combat data and combat state to be wholly absent; authors
should not insert dummy combat content into non-combat worlds. The roadmap treats
combat and other genre mechanics as source-grounded capabilities.

Format 1 also represents one fixed protagonist and one campaign. Future
multi-campaign support may package a canonical campaign and an unlockable
original-character campaign together. Each will require its own start state and
endings, plus explicit cross-campaign facts; runtime unlocking must never require
source material or generation tools.

A package is a directory containing these required UTF-8 JSON files:

| File | Content |
| --- | --- |
| `world.json` | Format version, world ID/name/language, starting location, player name, level table, declared flags |
| `locations.json` | Array of locations with descriptions, directional exits, NPC and monster IDs |
| `npcs.json` | Array of NPCs with descriptions, dialogue IDs and availability conditions |
| `monsters.json` | Array of unique monster instances with descriptions, HP, attack, XP and fixed loot |
| `items.json` | Array of items with names and descriptions |
| `quests.json` | Array of quests with giver, defeat objective, prose, rewards and completion flags |
| `dialogues.json` | Array of dialogue trees with nodes, choices, conditions and effects |
| `narrative.json` | Combat template variants, victory template and death text |

Empty content tables are `[]`; files must still exist. Extra files such as
author notes or future provenance sidecars are ignored by the runtime loader.
Unknown fields inside the defined JSON structures are rejected to catch typos.
Version 1 describes this initial schema; incompatible changes require an
explicit version/migration decision.

IDs use ASCII letters, digits, `_` and `-`, with uniqueness within each entity
table and within each dialogue's node list. IDs are machine handles; displayed
names and prose are unrestricted Unicode and retain the source language.
`language` is the author-declared language tag, for example `en` or `zh-Hans`.
The validator requires a nonempty value; it is not a BCP 47 registry validator.

## Locations and conditions

Exits are directed. To return along a path, author a separate reverse exit.
Directions are `north`, `south`, `east`, `west`, `up`, `down`.

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
exits, NPCs and dialogue choices. A condition tests either a declared flag or a
quest state:

```json
{ "kind": "quest", "quest": "quiet_the_track", "status": "ready" }
```

Quest statuses are `available`, `active`, `ready`, `completed`. All flags start
unset. Dialogue `set_flag` effects and quest completion flags set them; flags
are monotonic in this version. Conditions govern availability/choice visibility;
general prerequisite expressions, negation and quest chains are not implemented.

Each monster ID can appear at most once across all locations. This version has
no separate monster templates/spawns and no respawns. Quest targets must be
placed. NPCs may appear at several locations; conditions determine availability.

## Dialogue and quests

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
ends the conversation. Moving or attacking also closes the conversation.

The first objective type is `{"kind":"defeat","monster":"wolf"}`. An active
quest becomes ready when that instance dies. Accepting after its defeat makes
the quest ready immediately. Accept/complete actions require the available
giver in the player's current location, whether invoked by dialogue or a direct
command. Completion grants rewards once, sets flags, and emits stored completion
prose. Dialogue visibility conditions are not additional quest prerequisites.

## Numeric rules and templates

Level entries supply cumulative `xp`, maximum `hp` and `attack`. The first
entry requires zero XP; thresholds strictly increase. HP/attack are positive
and do not decrease between levels. HP/damage use `u32`; XP and item counts use
`u64`. Overflow refuses the whole command with no partial rewards or state.

`attack` and `hurt` each require at least one template. They allow
`{attacker}`, `{target}`, `{damage}`. The `victory` template allows `{target}`;
`death` is plain text. Example:

```json
"{attacker}一剑刺向{target}，造成{damage}点伤害。"
```

Placeholders use exact ASCII keys even for non-English text. Unknown or
unbalanced placeholders are validation errors; brace escaping is not supported
in templates yet. Plain prose fields are not interpolated. Substitution is
single-pass: a name containing `{damage}` remains a literal name.

The engine selects variant `successful_command_count % variant_count` using
the count before the current attack. Failed commands do not advance it.
Inspection commands count as successful commands, so the complete command
sequence determines narrative variants. No runtime randomness is involved.

## Validation feedback

`WorldSpec::diagnostics()` returns all detected content problems as serializable
`Diagnostic { severity, entity_id, code, message }` values in stable traversal
order. `validate()` returns `SpecError::Validation` with those diagnostics if
any are errors. `load()` validates before returning a playable world. File I/O
and JSON syntax/type errors retain the file path and underlying error.

Checks include version, IDs, references, dialogue links/effects, declared flags,
level rules, HP, loot quantities, monster placement and template placeholders.
Checks do not yet analyze graph reachability, condition satisfiability,
never-set flags, narrative quality, or battle/quest solvability. Passing validation
means the engine can interpret the data, not that every route is winnable.
