# Combat architecture

RealmKit is a deterministic RPG engine, not a turn-based combat engine. A
fighting character's progression and rules hold however a fight is run; the
paused initiative timeline is one way to run one. This page says which parts
belong where, how MP and Rage work, how a future real-time client would use
the same rules, and what is not supported yet. Package fields are in
[world-format.md](world-format.md#combat-block-numeric-rules-and-templates).

## Three layers

| Layer | What it owns | Where |
| --- | --- | --- |
| **Character core** (universal) | Levels and XP, stat points, effective stats, gear and techniques, HP, MP and Rage, skill costs, damage and crits, Rage from acting and from damage, resource regeneration and decay, validated vitals | `realmkit-engine/src/character/`, `gear.rs`, `techniques.rs`, `formulas.rs`; spec `stats.rs`, most of `combat.rs` |
| **Turn-based encounters** (optional) | The initiative timeline, action delays, whose turn it is, opponents' policy, attack, skill, flee and item turns, how encounters start and end, and timeline MP regeneration | `realmkit-engine/src/encounter/`; spec `Timeline` |
| **Mass battles** (strategic, separate) | Troop stacks, frontage, orders, morale, pursuit and autoresolve | `realmkit-engine/src/battle.rs`; spec `troops.rs` |

The core knows nothing of initiative, turns, ticks, frames or a host engine.
The turn-based layer decides *when* someone acts and asks the core *what* the
action does: whether the skill can be paid for, what it costs, the damage, and
the Rage it earns. World time asks the core what elapsed minutes do. A mass
battle uses the player's own HP from the core, and troops' fixed stats; it is
not personal combat and is not a real-time system.

Field meanings are the core's, with each layer reading them in its own terms:

- `speed` is how quickly a character acts. The timeline turns it into the delay
  between turns; a real-time layer could turn it into an attack interval.
- A skill's `time` is the action's length relative to a basic attack. The
  timeline delays the user's next turn by it; a real-time layer could scale a
  recovery or animation window by it.
- `mp_regen_percent` belongs to the timeline alone: per baseline turn of
  encounter time. World-time regeneration is `mp_recovery`.

## One owner per character's vitals

`Vitals { hp, mp, rage, rage_remainder }` are a character's current HP and
resources. Maxima are derived from effective stats and never saved.

- The player's vitals live in `CombatState::vitals`, whatever the stance.
  Exploring, an encounter and a battle all read and change that one copy.
- An opponent's vitals live in its `Participant` for the length of the
  encounter: opponents are whole again next time, as before. The player's
  participant holds `None` and the save check rejects anything else.
- Troops in a battle are fungible counts in stacks, never characters with
  vitals.

`Stance` (`exploring`, `fighting`, `battle`) only records how the player is
fighting now. A future named companion would get its own `Vitals` beside the
player's in the same way, not a copy inside a stance.

## MP and Rage

Both resources exist for every fighter, and each character uses MP, Rage,
both or neither. A resource a character does not use stays at 0, gains and
regains nothing, pays for nothing, and is shown as absent, which is not the
same as a resource in use at 0 (`Engine::player_pools`, `Engine::pools_of`).
`player_resources` and a combat profile's `resources` choose explicitly;
without them a character uses MP when its maximum MP is above 0, and Rage when
the world builds Rage at all, which is exactly what worlds did before the
choice existed.

**MP is a reserve.** It starts full, skills spend it, and resting and levelling
up refill it. It is kept between fights. Time refills it on two clocks that
never overlap: `mp_regen_percent` on the encounter timeline, and
`mp_recovery` (a percentage of maximum MP at each multiple of `every` world
minutes). Attacking never earns MP.

**Rage is momentum.** It starts at 0 and builds from fighting:
`rage_per_action` for each attack or skill the character resolves, credited
after the action, and `rage_per_max_hp` for taking damage equal to maximum HP,
proportionally, with the fraction carried. Time never builds it. `rage_cap`
bounds it; at the cap nothing is banked toward the next point. By default Rage
belongs to one fight and clears when the fight ends, as it always has. With
`rage_carries` it stays with the character and `rage_decay` drains points at
fixed world minutes.

A skill costs `cost` from its `resource` (the original form) or `costs` in
both. It is usable only if every cost is covered; MP is checked and spent
first, and each resource spent is reported. A refused skill changes nothing
and draws nothing from the random streams.

A world may present Rage as sword momentum or fighting spirit in its own
words; it is still the same resource, not a third one.

### Clocks

World time and encounter time are separate clocks and never regenerate the
same MP twice:

- An encounter or battle never advances world time: travel, waiting and
  resting are refused during them.
- Encounter time exists only inside an encounter. Its regeneration progress
  (`Participant::mp_remainder`) ends with it.
- World-time recovery and decay happen at fixed multiples of `every`, counted
  from minute 0. Crossing boundaries is additive, so a span split anywhere
  (one long wait, several short ones, a journey interrupted by scheduled
  events) crosses the same boundaries, and nothing needs saving between them.
  Boundaries at a minute apply before anything else scheduled then.

## A future real-time layer

A real-time combat layer would be another consumer of the core, beside
`encounter/`, inside the engine so that it stays authoritative:

- It reads and writes the same `Vitals`, so HP, MP and Rage carry into and
  out of a fight without conversion, and saves need no second model.
- It uses `character::resources` to check and spend costs, earn Rage from
  actions and damage, and regenerate MP on its own clock through
  `resources::regenerate`, with the progress held by the fight as the
  timeline's is.
- It uses `character::strike` for damage, crits (from the combat stream) and
  HP loss, and `character::player_stats` for effective stats.
- Its clock must be a fixed-step, integer, engine-side clock fed by validated
  commands, never a host's frame rate or physics. It must not advance world
  time and its own clock together over the same span unless one of them stops
  regenerating, as the turn-based layer already guarantees.

A host (Godot or any other) submits intent; it never declares damage, kills or
loot. [Spatial hosts](spatial-hosts-and-military-battles.md) records the wider
host boundary.

## Not supported yet

- A real-time combat layer, its command set, its fixed-step clock and its
  saved mid-fight state.
- Committing elapsed host time in batches outside travel, waiting and
  resting (see [host-driven passage of time](spatial-hosts-and-military-battles.md#host-driven-passage-of-time-future-requirement)).
- Named companions with persistent vitals, and allies in encounters.
- HP regeneration over world time.
- Resource events for recovery and decay: clients read the new vitals.
- Opponents carrying Rage or wounds between encounters; their vitals belong
  to one fight.
- A third fundamental resource.

## Compatibility

Existing packages need no change and keep their rules revision: every new
field is optional and absent by default. Encounters, damage, RNG draws,
progression and battles are unchanged; every example walkthrough produces
byte-identical output.

Saves keep format 19 but store the player's vitals in `combat.vitals`
instead of inside the stance. As the [format policy](world-format.md#versions)
says, a save that no longer parses is refused, never migrated; a save in the
earlier layout is refused by name, with the advice to start a new
playthrough.
