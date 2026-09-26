# World package format 1

Format 1 is the combat-enabled scaffold format used by the demo. Its mandatory
level table and narrative combat templates mean it does **not yet** represent the
long-term rule that combat is optional. A later compatible extension or format
version must allow combat data and combat state to be wholly absent; authors
should not insert dummy combat content into non-combat worlds. The roadmap treats
combat and other genre mechanics as source-grounded capabilities.

Format 1 also represents one fixed player-controlled protagonist and one
playable route. Future formats may package a canonical route, an original-character
route, or both over the same shared world and canonical timeline. When both are
present, New Game selects between them directly; one route does not unlock the
other. Each route has its own player binding, start state, main questline, outcomes and
mutable save while reusing shared locations, NPCs, factions and other world
definitions where appropriate. Future static player identity should use a
`PlayerSpec`-like concept rather than `ProtagonistSpec`: the controlled
character is not necessarily the source story's protagonist.

In an original-character route, the canonical protagonist remains in the package
as a canonical world entity/NPC rather than being replaced by the player.

Format 1 also requires item and quest tables because they serve the current demo.
Inventory is not a long-term universal requirement, but quest progression is:
future formats should generalize quests into main and optional side questlines
rather than remove them. A non-combat world still has a main questline whose
objectives may use dialogue, exploration, investigation or other capabilities
instead of combat. Investigation evidence is independent state and may optionally
reference a physical entity/item without being stored "inside" inventory or
inferred from possession.

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

The package language is also the presentation language for play. A client loading
a source-backed world must display its own fixed labels, help, prompts, status
messages and player-visible errors in that language rather than falling back to
English. Stable schema keys, IDs, enum values and typed-command aliases are
machine-facing and may remain language-neutral ASCII. Format 1 does not yet carry
client locale strings; the M0 CLI therefore only fully satisfies this requirement
for English worlds.

## Locations and conditions

Exits are directed. To return along a path, author a separate reverse exit.
Directions are `north`, `south`, `east`, `west`, `up`, `down`.

The long-term presentation model should distinguish **spatial placement** from
**traversal connectivity**. Clients may render a 3×3 local neighborhood centered
on the current location, including diagonal nearby cells for orientation, while
movement remains limited to explicit cardinal exits. A location shown northeast
of the player is not automatically reachable by a diagonal move.

Future format work may therefore add presentation-oriented placement metadata
(for example area-local integer coordinates or an equivalent layout relation)
without deriving exits from coordinates. Conversely, an exit remains the
authoritative statement that movement is possible even if a client chooses a
different visual layout. Vertical `up`/`down` travel is outside the 2D 3×3
plane and should be rendered as a separate contextual action.

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

Format 1 has a single flat quest collection. The long-term model should retain
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

Format 1 currently selects variant
`successful_command_count % variant_count` using the count before the current
attack. Failed commands do not advance it. Inspection commands count as
successful commands, so the complete command sequence determines narrative
variants. No runtime randomness is involved.

This is a current implementation detail, not a content contract authors should
depend on. It couples unrelated browsing/inspection actions to later combat prose.
A later runtime revision should keep selection deterministic while keying it to
the relevant gameplay/narrative event sequence (for example an encounter-local
attack sequence) so presentation-only actions cannot change which prose variant
appears.

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
