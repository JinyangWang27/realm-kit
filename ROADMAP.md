# RealmKit roadmap

Status: discussion draft. Milestones describe playable outcomes, not release
dates. M0 through M4 are complete; M5, M6 and the presentation track are
partially delivered; M7 through M9 remain proposed. Individual slices below are the source of truth for delivered
scope. Combat pacing and balance formulas remain design decisions where their
sections still say so, not promises about current behavior.
The [open-decisions register](docs/open-decisions.md) records unresolved choices
and when they must be settled.

## Principles that carry through every milestone

- AI participates only in authoring. A finished package runs without AI,
  credentials, source material or network access.
- The engine owns all state and rules. Menus, typed commands and future clients
  invoke the same structured commands.
- All player-facing text in a source-backed world uses the source's language.
  This includes authored narrative/dialogue and client-owned labels, help, prompts
  and runtime messages. Machine-facing identifiers and stable command tokens may
  remain language-neutral. Narrative and branches are authored before play.
- Rules, scheduling, template selection and stochastic mechanics are replayable.
  Randomness, when used, comes from explicit saved RNG state and only selects among
  authored possibilities.
- External agents use a typed authoring core; serialization and MCP are adapters.
- The shared world owns the location graph, characters/entities and story-phase
  definitions. Every world has one or more player routes; each route owns a
  PlayerSpec-like player-control binding to a shared Character, start
  state/location, exactly one main questline and one or more authored outcomes.
  Side questlines are optional, but
  main-story progress unlocks them in bounded waves so exploration remains open
  without making the main story irrelevant. Repeatable offers, such as a guild's
  delivery jobs in a sandbox world, are activities rather than side questlines,
  so they sit outside those waves.
- Time models stay separate: story phase is core narrative progression; an
  in-world clock/calendar is optional; encounter timelines are local schedulers;
  real-world thinking time advances none of them.
- Gameplay capabilities are source-grounded and composable. Inventory, combat,
  crafting, investigation, equipment and similar systems are optional; absent
  capabilities contribute no required data, runtime state or player actions.
  Evidence belongs to investigation state; a physical item may be linked to
  evidence through a typed relation without making inventory universal. The
  [capability catalog](docs/capabilities.md) guides selection without defining a
  mandatory feature list.
- Add a system when a milestone needs it. No empty future crates or generic ECS.

## Original-game priority path

The first intended full original game is RealmKit's main pressure test. Its
requirements set the near-term implementation priority, but they do **not** become
mandatory engine features for unrelated worlds. When choosing between speculative
generality and a feature needed by the original game's next playable slice, build the
original game's requirement with the smallest reusable typed boundary.

The current priority is:

1. **P0 — player-facing presentation seam · delivered**. The
   overland-map data/query for a continent-scale world, and proof that the
   spec/engine can be hosted by a graphical client without terminal or
   filesystem assumptions. The target product client is text-first and
   graphical; React/Tauri/web details stay outside the engine. Keep
   `realmkit-cli` as the reference/debug client.
2. **P0 — authored opening, progression and investigation · MVP slice
   delivered**. Route start choices for an ordinary-background
   opening; story phases, main quests, quest prerequisites and
   route outcomes; evidence as investigation state; places the player learns
   of before the map shows them; and features to examine.
   `examples/caravan-trail` plays an MVP-shaped slice from a caravan's
   arrival through an investigation to a choice of leads. Authoring the
   original game's first slice then added `at_least`
   conditions, evidence facts that fill in, ask-once and back dialogue
   choices, unavailable choices with a reason, and language overlays with
   saves bound to the rules alone. Still to come when the original game's
   content needs them: failing, abandoned and deadline quests, terminal and
   failure outcomes, dialogue-by-role and conditional text.
3. **P0 — political identity.** Factions and typed standing/relation tracks are
   required before the game world's states, martial schools and orders can be represented
   as gameplay rather than lore-only labels.
4. **P1 — living sandbox world.** Finish the retinue pieces the original game needs
   (especially companions, provisions/morale and travel speed), then holdings and
   sieges, world agents, faction strategy, and the minimum politics/order
   composition needed for fealty and organizations. The already delivered
   economy and mass-battle slices are foundations. Prisoner detail, courtship,
   champion duels and similar subfeatures wait for the original game's authored content that
   actually needs them.
5. **P2 — Shared play.** Preserve a complete offline solo game. Shared
   play is a later optional authoritative mode over the same deterministic
   rules, with an explicit local-to-server authority handoff and no attempt to
   merge divergent offline and shared histories.

This priority path changes sequencing, not modularity: a world that needs no
factions, army simulation, map, multiplayer or combat still authors none of them.
## M0 — Playable foundation · complete

Delivered: four-crate Rust workspace, static JSON world packages, structured
validation, deterministic command/event engine, typed authoring operations,
terminal commands, and the hand-authored demo.

The demo covers movement, dialogue choices, accepting a quest, fighting a wolf,
loot, XP, leveling, quest completion and unlocking the chapel. Tests exercise the
full journey and replay it deterministically. Runtime builds independently of
worldgen and plays offline.

Current limits: fixed damage, immediate enemy counterattacks and session-only
state.

## M1 — Play without memorizing commands · complete

Delivered: context-sensitive engine actions plus a terminal menu that supports
arrow/Enter/Esc navigation, numbered shortcuts, direct movement keys and typed
commands. Pipes/scripts retain line mode, and the engine rechecks legality when a
selected action executes.

```text
Ashbell Village

> 1. Talk to Elder Mara
  2. Travel north — The Pine Track
  3. Travel east — The Chapel [locked]
  4. Inventory
  5. Character
  6. Quests

↑/↓ select · Enter confirm · number choose · Esc back
```

M1 also makes `look`, inventory, status and quest-panel inspection
presentation-only: they do not advance the engine turn. The complete demo is
tested through numbered menus as well as scripted typed input.

The later architecture discussion added richer presentation goals that were not
part of the merged M1 implementation: optional Area-based spatial layouts,
full-map or client-sized viewport rendering, discovery-aware map disclosure, and
full fixed-interface localization to the package language. Those remain future
presentation work tracked in
[Section 14](docs/open-decisions.md#14-presentation-and-information-disclosure);
they are not retroactively part of M1 acceptance.

## M2 — Continue an adventure across sessions · complete

Delivered: engine `SaveSnapshot` export and all-or-nothing validated restore
bound to package ID, content-digest revision and the `default` route (now a
digest of the rules alone, leaving out prose and language); CLI
`--saves <directory>` with atomic JSON files, a lineage index that forks when an
older save is loaded, `save`/`load` commands, auto-saves at route start and
quest completion, and newest-save recovery on death that reports invalid saves
instead of skipping them. Save and load are typed commands; they are not yet
entries in the numbered menu.

The original scope:

Treat saves as storage-neutral `SaveSnapshot` data. The engine serializes and
validates deterministic state; CLI/mobile/server layers choose files, SQLite,
databases or other durable storage.

- Save/load the selected player route's core mutable state, including location,
  main-quest progress and other authored story/world state, plus only optional
  capability state that exists for that route/playthrough, including any relevant
  shared world-scoped capability state—for example investigation evidence,
  inventory, combat stats or defeated enemies. A capability present elsewhere in
  the package does not require placeholder state in a route that never uses it.
  Keep authored definitions separate from mutable saves.
- Version saves and identify the world package they belong to. M2 treats the
  current Format 1 single playable route as an explicit logical route with stable
  ID `default` for save/API identity, even though Format 1 does not yet serialize
  a route collection. SaveSnapshot records that `player_route_id`; later formats
  with explicit PlayerRoutes use their authored route IDs. Reject incompatible
  saves clearly; add migrations when an actual format change requires them.
- Write saves atomically and keep the previous save safe if writing fails.
- Support manual saves and deterministic auto-saves in one active chronological
  recovery lineage. Loading an older snapshot forks the active lineage at that
  snapshot: saves from the abandoned future are no longer automatic-recovery
  candidates, though a client may retain them for explicit manual branch
  selection. For M2, auto-save at route start and at stable progression
  boundaries the current format actually exposes (for example quest completion).
  As richer story phases are implemented, their major transitions become default
  checkpoint boundaries too. Allow additional authored/capability checkpoints
  where useful, but do not auto-save on every ordinary movement step or
  presentation command.
- Ordinary recoverable death attempts to restore the newest manual/auto-save
  recovery snapshot. If that snapshot fails load validation, surface the failure
  and let the client explicitly offer an older snapshot; never silently skip a
  corrupt newest entry. Restoring replaces the current playthrough state with the
  saved deterministic snapshot rather than "undoing" commands piecemeal. Explicit
  authored terminal death/failure outcomes bypass this recovery behavior.
- Save creation consumes no story/world/encounter time and must not perturb
  narrative variant selection.
- Preserve every deterministic counter; later combat scheduling must also survive
  save/load. Treat loaded saves as input that needs validation.

**Done when:** saving mid-quest, quitting and resuming produces the same subsequent
events as uninterrupted play; ordinary death attempts the newest recovery entry
and surfaces validation failure before any explicit older-snapshot fallback;
route-start and currently supported progression-boundary auto-saves are
reproducible; and a broken or mismatched save cannot corrupt a world or silently
reset progress.

## M3 — Optional combat capability: stats and meaningful speed · complete

Introduce one coherent combat model for worlds that need combat rather than
adding unrelated stat fields to every world. A combat-free package has no combat
profiles, HP/MP state, monsters, skills, damage templates, attack commands or
combat UI. Engine and clients discover that absence from the validated package.

The existing demo remains a combat-enabled fixture. Before expanding its combat
rules, move combat-only player fields out of mandatory base state so a future
detective fixture can load and play without dummy HP or attack values. Use a small
optional state component or equivalent explicit Rust type; do not build a generic
plugin framework or ECS.

| Stat | Initial purpose |
| --- | --- |
| HP / maximum HP | Survival and healing limit |
| MP / maximum MP | Resource for skills |
| Physical attack | Strength of physical damage |
| Physical defence | Mitigation of physical damage |
| Special attack | Strength of special damage: magic, 内力, mana, as the world's `special_name` says |
| Special defence | Mitigation of special damage |
| Speed | Action timing; exact model discussed below |

Use these direct combat stats first. Strength, intelligence, dexterity, elemental
resistance, accuracy and critical chance can come later when builds need them.

- Apply the same stat and damage rules to players and monsters.
- Add a basic physical attack, one rage-costing physical skill, one MP-costing
  special skill and one free special skill, sufficient to test both damage
  channels and both [skill resources](#skill-resources). Each character's basic
  attack has an authored channel, physical by default; M4 weapons may set it.
  Costly skills come in
  tiers unlocked by character level, and monsters get level-gated skills under
  the same rules; see [Tuned values](#tuned-values). Define safe-location HP/MP
  recovery explicitly.
- The skill's channel selects the dominant attack/defence pair, and the other
  channel adds its authored share, as described under
  [Damage](#damage-two-channels-gradual-defence--proposed-formula). Keep skill
  power separate from the character's attack stat.
- Centralize damage calculation in the engine; content supplies bounded numeric
  parameters, never executable formula strings.
- Specify rounding, minimum damage, resource costs, death and action cancellation.
  Use checked integer arithmetic and reject invalid stats or parameters.
- Keep the first combat slice simple, but support stochastic combat mechanics such
  as critical hits through RealmKit's explicit seeded RNG facility. Basic attacks
  may remain certain initially; randomness is an authored rule rather than a
  requirement for every combat action. This is the optional final slice, M3d.
- When stochastic mechanics are implemented, use explicit/versioned PRNG behavior
  with saved RNG state. Keep semantically unrelated random domains independent
  where incidental draw coupling would produce surprising gameplay changes.

**Done when:** a small duel demonstrates physical and special builds that play
differently, MP and rage expenditure and recovery, and, once the M3c timeline
exists, a measurable benefit from increased speed. "Differently" means distinct
actions and resources, not passing the balance targets: tuning waits until the
systems are complete (see [Balance simulation](#balance-simulation--proposed)).
In M3, builds are fixture stat blocks; player-chosen builds arrive with
equipment in M4.
A one-against-two fixture exercises several opponents, explicit targeting and
tie order. Tests cover formulas, scheduling, ties, death, resource rejection,
replay and save/load, including a save made mid-encounter. A combat-free fixture
validates and plays through a main quest without combat data, levels or menus.

### Optional parts of combat · proposed

Combat is optional, and so are most of its parts. A world uses a part by
authoring content that needs it. An absent part leaves no state, UI, required
data or placeholder, exactly as an absent capability does. Only the parts every
fight needs are fixed.

| Part | Present when | Without it |
| --- | --- | --- |
| Core: HP, attack and defence stats, speed, damage, timeline, basic attack, encounters | The world has combat | No combat at all |
| Character levels and XP | The world authors a level table | Stats come from authored base stats, techniques and equipment; defeats grant no XP |
| MP | Any skill costs MP | No MP stat, bar or regeneration |
| MP regeneration in encounters | A rate is authored | MP refills only by resting |
| Rage | Any skill costs rage | No rage state or constants |
| Skills beyond the basic attack | Any are authored | Basic attack only |
| Technique ranks (M4) | A technique authors more than one rank | Every technique has one fixed rank |
| Stat points (M4) | The level table grants points | Stats come only from the level table, techniques and equipment |
| Realm display (M4) | A core internal art is named | No realm shown |
| Repeatable groups, yielding, flee restrictions | Authored on a group | Groups fight once, to the death, and allow fleeing |
| Seeded RNG (M3d) | Stochastic content exists | No RNG state |

Validation warns about configuration that no content uses, such as rage
constants without a rage skill, rather than rejecting it.

**Power comes from stats, never from character level.** A character at level 1
can be very strong: 虚竹 receives 无崖子's seventy years of 内力 at once (传功), and
段誉 absorbs 内力 through 北冥神功. Stats may come from a level table, technique
passives, equipment or authored grants, and the damage formula and timeline read
only the resulting stats. Character level, where it exists, records experience.
It still drives XP falloff; a world without levels compares an opponent's
authored level with the player's realm rank instead, or has no falloff.

### Delivery slices · proposed

Each slice ships and is tested on its own. M3a depends on none of the open
balance decisions.

1. **M3a — combat becomes optional · delivered.** Merge `npcs.json` and
   `monsters.json` into one character list whose dialogue and combat profile are
   optional components, matching the glossary's shared Character model. The
   player-controlled character becomes an entry in that list, named by the world
   in place of `player_name`, as a first step toward PlayerSpec. Move the
   level table and the attack/hurt narrative templates into an optional world
   `combat` block; for now XP and levels belong to combat, so a combat-free world
   has neither. Replace the persistent `monster_hp` map with a set of defeated
   characters. Add a flag-based quest objective so a combat-free fixture can have
   a main quest. Reject Format 1 packages and saves clearly rather than migrating
   them: every content edit already invalidates saves through the package
   revision. The demo plays as before. As delivered, opponent HP still persists
   between separate attacks, inside the optional combat state as
   `CombatState.opponent_hp` (0 means defeated), because nothing else owns it
   until M3c encounters do; M3c replaces it with the defeated set.
2. **M3b — stats, damage and skills · delivered.** The seven-stat block, gradual defence
   reduction by the damage formula confirmed in decision 3, the skills and MP
   costs listed under
   [Tuned values](#tuned-values), and a `Rest` command at authored safe locations
   that restores HP and MP outside encounters. Rage and MP regeneration during
   encounters need encounter time, so they arrive with M3c. Save only current HP/MP,
   level and XP; derive maximums and attack/defence values from the level so no
   bonus can be saved twice. Decide explicitly whether level-up still restores
   HP/MP fully, as it does today. Enemies keep counterattacking immediately.
   The combat block gains a required `special_name`, the world's name for the
   special channel, which clients show in place of "special". Speed is authored
   from M3b so M3c needs no format change, but it has no effect until the M3c
   timeline; the docs say so. MP costs are flat, exactly as authored.
   As delivered, level-up keeps restoring HP and MP fully; opponents keep HP
   and MP between attacks until M3c encounters own them; a combat profile has
   no level yet, so every skill it lists is usable; rage is not implemented, so
   the rage skills in [Tuned values](#tuned-values) wait for M3c.
3. **M3c — encounters on the timeline.** `Engage`, participants and sides,
   timeline scheduling, the projected turn order, and mid-encounter save/load, as
   described in [Combatants and encounters](#combatants-and-encounters--proposed).
   Grinding arrives here too: repeatable encounter groups and `Flee`, along with
   authored yielding. Fixtures cover a 1v1 duel, a 1v2 pack, a repeatable hunting
   ground, a fled fight and a sparring match that ends in a yield. Delivered in
   two parts. **M3c-1 · delivered:** one-opponent encounters, the timeline,
   rage and in-fight MP regeneration, the projected turn order and
   mid-encounter saves, matching `scripts/combat_sim` exactly (package and save
   format 4); defeated characters replace M3a's persistent opponent vitals.
   **M3c-2 · delivered:** groups and packs, repeatable groups, `Flee`,
   yielding with victory and defeat flags, opponent levels with level-gated
   skills, and XP falloff (`examples/arena`); package and save format 5. Flee's wind-up always completes
   within the one command that declares it, so no pending flight is saved;
   the saved `pending` field waits until turns can pause mid-wind-up (M9).
4. **M3d — seeded RNG and critical hits (optional) · delivered.** A small
   hand-written, versioned PRNG such as SplitMix64 or PCG32 with saved state. Do
   not use `rand`'s `StdRng`: its output is not guaranteed stable across
   versions, which would break replay and saves. As delivered: SplitMix64
   version 1 with a per-domain combat stream, present only in worlds that author
   a crit; crits on skills and basic attacks multiply before the single rounding;
   package and save format 6.

### Combatants and encounters · proposed

Combat is not monster-specific. Any character — the player-controlled one, an
NPC or a creature — can take part in an encounter when it has a combat profile.
Encounter state holds any number of participants and sides from the start. M3
content exercises one player against one or more opponents; allied participants
(XvY) arrive when a world needs companions or party play, without changing the
encounter state shape.

```text
CombatProfile       optional component of a Character definition:
                    stats, plus loot/XP granted when defeated
Encounter           active local state; at most one per playthrough
├── now             current timeline time
└── participants    in fixed order
    ├── character   CharacterId; copies of one definition differ by position
    ├── side        SideId
    ├── control     player | policy
    ├── hp, mp, rage
    ├── remainders  fractional progress toward the next MP and rage point
    ├── pending     none, or a declared flee awaiting its turn
    └── next_time
```

- **Starting.** `Engage(character)` names any character at the current location
  whose combat profile is engageable under its authored conditions, so an NPC can
  become fightable after a story flag without being a separate monster type. A
  profile may name an authored group: engaging any member brings in every
  undefeated member present at the location, in authored order. Later triggers,
  such as an ambush on entering a location or a dialogue effect that turns an NPC
  hostile, start encounters through the same rule. M3 needs only `Engage`.
- **Sides and victory.** Every participant belongs to a side. The player's side
  comes first, and an engaged group joins the opposing side. An encounter ends in
  victory when no opposing participant is alive. Sides are IDs rather than a
  boolean so allies, and later more than two sides, reuse the same state;
  relationships between three or more sides are decided when a world needs them.
- **Control.** A participant is controlled by the player, in which case the
  timeline pauses for a command, or by an authored policy that the engine resolves
  immediately. M3's only policy targets the first living opponent in participant
  order with the strongest skill it can afford, falling back to its basic attack.
  Equal power prefers the cheaper skill, then the later tier. Opponents follow the same skill rules as players, so their level-gated skills
  matter; the balance simulation depends on this. Richer policies, such as
  targeting lowest HP, saving resources or random targeting through the RNG, are
  authored enum variants added with content that needs them, never scripts. Multiplayer adds more player controllers; each
  pending human turn pauses only its own encounter (M9).
- **Commands.** One player command resolves that participant's action, then every
  policy-controlled action until a player-controlled participant is next or the
  encounter ends. Actions name targets explicitly, as in `Attack(target)` and
  `UseSkill(skill, target)`. The engine rejects dead targets, characters outside
  the encounter and, for damaging actions, allies. During an encounter only
  combat actions, `Flee` and presentation panels are available: no moving or
  talking.
- **Order.** Every participant's first action comes after one opening delay, so
  a faster opponent may act before the player's first command. Ties resolve by
  `(next_time, side order, participant order)`, which puts the player's side
  first. A participant at 0 HP leaves the schedule immediately and loses pending
  actions. Rejected commands consume no time, MP or RNG state.
- **MP.** Checked when the command is validated and deducted as the action
  resolves, in the same atomic transition. There is no wind-up, so choosing an
  action and resolving it are one step.
- **Speed changes.** Delay is computed when an actor acts, so a changed speed
  applies from that actor's next action. M3 has no speed buffs and no equipping
  during an encounter.
- **Ending.** Victory grants each defeated opponent's authored loot and XP once,
  records defeated authored characters, advances quests and clears the encounter.
  The player-controlled character reaching 0 HP is ordinary death, handled by
  M2's recovery, whatever allies survive. Downed/revive rules wait for a world
  that needs them.
- **Fleeing.** `Flee` is the one explicit exception to actions resolving
  immediately: it has a wind-up. Declaring it on the player's turn schedules the
  escape for that participant's next turn with the normal action time, during
  which the player takes no other action, opponents act as usual and damage
  applies. The escape resolves when that turn arrives if the player is still
  alive, so a faster player escapes sooner. The pending flee is part of the
  participant's saved state, so a save taken during the wind-up still escapes on
  load instead of asking for a new action. Other actions keep resolving at once. An authored group may forbid
  fleeing, as a boss or canonical duel might. Fleeing grants nothing and records
  no defeats.
- **Yielding.** A group may be authored to yield at a share of maximum HP, as in
  比武 that stops short (点到为止), a joust or a canonical duel. The rule applies
  to every participant in the encounter, one at a time:
  - Each participant's threshold is `max(1, ⌊max HP × share ÷ 100⌋)`.
  - A hit never takes a participant below 1 HP; one that ends at or below its
    threshold yields at once and leaves the schedule alive, like a defeated
    participant that did not die.
  - A side has yielded when none of its members is still fighting. The player's
    own character yielding ends the encounter as the player side's yield,
    whatever allies remain, just as the player's death does.

  The encounter then ends with its authored victory or defeat effects, such as
  flags, and nobody is recorded as defeated or dead. A yielding player keeps the
  remaining HP and play continues, so losing a sparring match is not M2 death
  recovery. Yielding also lets a canonical character protected by a canon anchor
  lose without being killed.
- **Vitals ownership.** While an encounter is active it is the only owner of every
  participant's HP and MP. The player's persistent vitals move in when it starts
  and back when it ends, so a save never holds two copies. Opponent HP and all
  rage exist only inside the encounter: after a flight, the opponents are whole
  again next time. A mid-encounter save holds rage and the fractional MP and rage
  remainders too, so loading cannot change when a skill next becomes affordable.
- **Repeatable groups.** An authored group may be repeatable, like a hunting
  ground. Defeating it records nothing as defeated, so it can be engaged again
  immediately, and grants its loot and XP each time, with XP falling off by
  level difference. A defeat objective counts the first qualifying victory only,
  and quest completion rewards stay one-time. Respawn delays measured in World
  Time wait for M5.
- **Identity.** Participants that are authored singleton characters use their
  character IDs. Copies of one definition within an encounter, such as three
  wolves, are told apart by their participant position, shown as "Wolf 2" and so
  on. They never outlive the encounter, so they need no
  [Section 10](docs/open-decisions.md#10-definitions-instances-and-identity)
  runtime instance IDs; those are only for copies with persistent state.
- **Preview.** The engine exposes the next few scheduled actions, projected with
  the common action cost. Clients show that projection and label it as one.

Group size needs balancing as a unit: each extra opponent acts as often as a lone
one would. In the balance simulation below, two same-level ordinary monsters
cost a level-5 warrior about 63% of its HP, so packs should mostly be built from
a weaker minion tier.

### Stat ranges and caps · proposed

The absolute numbers are arbitrary. What matters is how attack and defence
compare, since defence equal to the attacker's combined attack halves damage;
where speeds sit relative to the baseline of 100; and how many meaningful steps
lie between starting and final values. Stats are not stored in 8 bits, so a cap of
255 would only be a convention borrowed from older games.

- **Engine bounds.** Validation rejects any authored or derived combat stat,
  including HP and MP, above 9,999; skill power outside 1–1,000; and, from M4, a
  damage multiplier whose numerator is outside 0–10 or denominator outside 1–10.
  Together they keep the damage numerator `A × P × A × M_num` below 4 × 10^16
  and the denominator `100 × 100 × (A + D) × M_den` below 4 × 10^11, far inside
  `u64`: the combined attack `A`, and likewise `D`, is at most 1,999,800 even at
  a 100% cross share. The stat bound alone is not
  enough, because power is part of the product. Arithmetic stays checked anyway.
  These are safety bounds, not balance targets, and the stat bound also keeps
  displays narrow. Lower bounds apply too: HP and speed at least 1; MP, attack,
  defence and XP rewards at least 0; skill costs at least 0 and action times at
  least 1. Every skill a character can use needs a positive combined attack for
  its channel, so zero attack is rejected rather than dealing minimum damage.
  Basic attacks are free and available from level 1, since they are the fallback
  action; other skills unlock at level 1 or later; authored stats are checked
  before any rounding; and per-level stats never fall as levels rise, as the
  level table already requires.
- **Speed cap.** A required world-level parameter because it governs scheduling.
  The starting candidate is 200 with baseline 100.
- **Other stat caps.** None in M3: the level table is authored, so authors already
  bound player stats. Add optional authored caps for other stats in M4 if
  equipment stacking needs them, applied after all modifiers as speed is.
- **Design scale.** Baseline speed is 100. A uniform stat cap of 100 would leave
  speed no room to rise. With 10% growth per level, a 200-HP character passes the
  9,999 bound at level 43, and a boss with four times the HP of an 80-HP opponent
  at level 38, so a world with more levels needs a higher bound or
  slower growth. The sample content therefore stops at level 35, and `check`
  validates every sample character and tier against the bounds up to there.
- **Timeline resolution.** With action cost 10,000, speeds 100–200 produce only
  51 distinct delays; near the cap about four speed points share one delay. Action
  cost 100,000 gives every integer speed from 1 to 255 its own delay, so use
  100,000 as the starting value. The timeline examples below use 10,000 for
  readability.

### Balance simulation · proposed

Balancing is deferred until the combat, build and equipment systems are all
implemented. Until then the simulator and its targets are a guide for choosing
starting numbers and spotting structural problems, not a gate: a slice may ship
with fixture numbers that miss a target, and final tuning happens once over the
complete system.

The `scripts/combat_sim` package models these rules: the damage formula, the
timeline, tie order and the basic enemy behaviour, not the engine itself. From
the repository root, `python3 -m scripts.combat_sim` prints a report, `check`
asserts the balance targets, `tune` nudges each tuned value by about 15% to show
which targets break, and `--formula` compares the K formulas. Tuned values live
in `content.py`, separate from the rules in `model.py`, so a prototype swaps
content without editing the model. Every target is checked against each sample
opponent in `content.py`: a physical-attacking beast and a special-attacking
spirit. They are ordinary characters with different numbers, not monster types;
testing against only one would favour whichever build defends against its
channel. The level table, stored as cumulative thresholds from 0 exactly as the
engine stores it, and each opponent's XP, authored on its profile, are sample
data there too; only the falloff by level difference is a proposed rule. The targets suit a game where grinding for levels (打怪练级) matters:

- A same-level ordinary monster takes 3–6 player actions and costs 15–35% of HP,
  allowing two to five fights between rests.
- A monster four levels higher is usually a loss or costs at least 40% of HP.
- A chapter boss needs zero to three levels above its own, so grinding pays off.
- Farming monsters far below the player's level stops giving XP.

- Builds trade off rather than one leading: across both opponents and levels 5,
  10 and 20, each build must win at least a third of the comparisons where they
  differ (actions, HP lost with a 3-point tie band, fights per rest, boss
  level). Neither may need more than one extra level to beat the same boss.
- Skills unlocked at higher levels clearly matter: at level 20, a build limited
  to its level-1 skills loses at least 5 more percentage points of HP.

#### Skill resources

Each skill names the resource it spends. Two resources give the builds
different limits:

| Resource | Starts at | Refills | Limits |
| --- | --- | --- | --- |
| MP | Saved value, up to maximum | 3% of maximum per baseline turn of encounter time; fully by resting | Sustained spending across fights |
| Rage | 0 in every encounter | +1 per own action; +20 for taking damage equal to maximum HP, proportionally and cumulatively across hits | Spending early in a fight |

- **MP regeneration follows encounter time, not actions.** A baseline turn is one
  basic action at speed 100. A faster actor regenerates at the same rate per unit
  of time, as the speed rules require, but gets more actions to spend it on.
- **MP costs are flat, exactly as authored** · decided. The engine never
  derives a cost, so the menu shows the number the author wrote, and a deeper
  pool from any source (levels, a 内力 grant, a technique rank) genuinely means
  more casts. Authors control sustain with the content they already write:
  how fast maximum MP grows in the level table, and each tier's cost. An
  earlier proposal scaled costs with the level table's MP pool; see
  [Findings](#findings) for why authored MP growth replaces it.
- **Rage never persists outside an encounter** and is not saved between fights.
  It is part of the saved encounter state during one.
- **Rage from an action is credited after the action resolves.** A skill's cost
  is checked before it resolves, so an actor one point short cannot spend the
  point its own action is about to earn. Damage rage is credited when the hit
  lands. The ordering changes balance, so `check` tests it through a real action.
- **Outside encounters,** MP refills only by resting in M3. Regeneration over
  World Time can follow when a world enables it (M5).
- Resources are authored per skill as one of a closed set. Do not build a
  general resource framework before a world needs a third resource.

#### Tuned values

Level-1 values that meet every target. HP, attack and defence grow 10% per
level for players and opponents alike; speed does not grow, and neither does the
mage's maximum MP. Growth is only how
the sample per-level tables are generated: the engine reads authored per-level
values and computes no growth. Generated values are exact and rounded half up,
since Python's `round()` rounds halves to even and Rust's `f64::round` rounds
them away from zero:

| | HP | MP | P.Atk | P.Def | S.Atk | S.Def | Spd |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Warrior fixture | 200 | — | 24 | 15 | 5 | 10 | 100 |
| Mage fixture | 170 | 75 | 8 | 10 | 20 | 15 | 100 |
| Beast (sample opponent) | 80 | — | 16 | 10 | 0 | 10 | 110 |
| Spirit (sample opponent) | 80 | — | 0 | 10 | 16 | 10 | 110 |

Skills unlock at a character level, and each later tier trades up: more power
for the same cost. Opponents follow the same rules with their own tiers. Each
character also has its own basic attack, so its channel belongs to the
character: the spirit's touch is special, which a creature without physical
attack needs to hurt anyone.

| Skill | Who | Channel | From level | Power | Cost |
| --- | --- | --- | --- | --- | --- |
| Basic attack | Everyone | Physical, or special for the spirit | 1 | 100 | — |
| Spark | Mage | Special | 1 | 80 | — |
| Bolt / fireball / starfall | Mage | Special | 1 / 10 / 20 | 170 / 210 / 250 | 12 MP |
| Rage strike / cleave / execute | Warrior | Physical | 1 / 10 / 20 | 150 / 185 / 220 | 5 rage |
| Rend / maul / savage | Beast | Physical | 1 / 10 / 20 | 130 / 155 / 180 | 5 rage |
| Hex / curse / wither | Spirit | Special | 1 / 10 / 20 | 130 / 155 / 180 | 5 rage |

The cross share is 25%. Minions have half HP and 60% attack; bosses have four
times the HP and 130% attack, both physical and special. The sim uses the
strongest affordable skill on every turn. Results at level 10, within a few
points at every level from 1 to 30:

| | Warrior vs beast | Mage vs beast | Warrior vs spirit | Mage vs spirit |
| --- | --- | --- | --- | --- |
| Same-level fight | 4 actions, 21% HP | 3 actions, 20% HP | 4 actions, 23% HP | 3 actions, 18% HP |
| Fights per rest | 4 | 3 | 4 | 3 |
| Level for a level-5 / 10 / 20 boss | 6 / 11 / 21 | 7 / 12 / 22 | 6 / 11 / 21 | 7 / 11 / 21 |
| Grinding from level 1 to 10 | 35 kills, 10 rests | 42 kills, 15 rests | 42 kills, 9 rests | 35 kills, 10 rests |
| Level 20 with level-1 skills only | 31% HP instead of 24% | 32% instead of 23% | 34% instead of 27% | 28% instead of 20% |

Each build wins 9 of the 18 comparisons where they differ. The warrior lasts
longer between rests and handles beast bosses a level earlier; the mage finishes
faster everywhere and takes less damage from spirits. Opponent skill tiers keep
same-level fights steady as players unlock theirs: without them, the mage's
level-20 fights fell to 3 actions and 18% HP, and opponents four levels higher
became easy.

`tune` nudges player and opponent stats, skill and basic-attack power, cost and
action time, tier multipliers, the growth rates (including the mage's MP
growth), the speed cap and the resource rules, and 107 of 169 nudges keep every
target. Zero stats and costs are nudged upward only, since
0 → 1 can matter: a beast with 1 special attack already breaks boss parity. It
reports the first target each nudge breaks. Half of the 62 breaks (31) are boss
parity against beasts, each a two-level gap where one is
allowed; allowing two levels (`BOSS_LEVEL_GAP` in `targets.py`) is a design
choice, not a tuning fix. Not every break is a near miss, though: lowering the
warrior's physical attack from 24 to 20 or its HP from 200 to 170, or raising the
mage's HP to 195, leaves the warrior winning only 1 to 4 of 15 or 16
comparisons, so the trade-off collapses toward the mage. Those three values need
the tightest control when balancing resumes, along with rage per action, which
stops the warrior's later skills from mattering when changed from 1 in either
direction, and the growth rate, which breaks boss parity at 8.5% a level. Action
time matters too: making a skill 15% quicker acts like a power increase, and 6 of
the 26 skill action-time nudges break a target, such as a quicker fireball making
level-10 beast fights cost only 12% HP. The remaining breaks are fights that
become too short, later skills that stop mattering enough, or too many fights
between rests.

The grinding figures assume that levelling up restores HP and MP fully, as the
Format 1 engine does; the roadmap leaves that open for M3b. Without it, kills
stay the same but every build needs 5 or 6 more rests to reach level 10, for
example 15 instead of 10 for the warrior against beasts. `XpRules` in
`simulator.py` makes the rule explicit.

#### Choosing skill power

Power is authored on each skill, weapon and technique rank; the engine never
computes it. Rules of thumb for authors, to be verified with `check` and `tune`:

- **Free skills stay at or below 100**, the basic attack, as spark's 80 does.
  Otherwise nobody uses the basic attack.
- **Costly skills gain power in proportion to what they spend.** Bolt adds 70
  over a basic attack for 12 MP; rage strike adds 50 for 5 rage. Compare skills
  by extra power per unit of resource, and remember that MP limits a whole rest
  cycle while rage limits one fight.
- **Later tiers trade up by roughly 20% at the same cost**, as 170 → 210 → 250
  and 150 → 185 → 220 do, or keep their power and cost less. Give monsters
  matching tiers so same-level fights stay steady and players need the new skills.
- **Slower actions need power at least in proportion to their action time.** A
  heavy weapon's basic attack at 160% time needs about 160 power to break even,
  and more to be worth its lost turns.
- **Check hits-to-kill breakpoints.** A little extra power that saves one hit
  removes whole enemy turns, so small changes can swing short fights.

#### Findings

- **Damage must not depend on a fixed K or on level.** With a constant K,
  defence outgrows it and fights lengthen: a level-30 warrior needed 10 actions
  per ordinary fight instead of 4. Scaling K with the attacker's level, as WoW
  does with armour, fixes that but ties damage to level: a level-1 character
  given level-15 stats needed 5 actions instead of 4 against a level-15 monster.
  The K-free ratio formula has neither problem, and the tuned values above use
  it. Run `--formula k-scaled` or `--formula k-fixed` to compare.
- **Each resource favours a fight length; combining them balances.** With MP
  refilled only by resting, long boss fights starved the mage, which needed three
  to five extra levels. Rage alone made the warrior the only viable boss build,
  because it grows with fight length. MP regeneration during fights and rage from
  damage taken bring both builds within one level of each other.
- **A free fallback spell matters.** A mage with no MP and only a physical attack
  needed 12 actions for an ordinary fight.
- **Short fights change at hits-to-kill breakpoints.** A zero-MP heavy strike
  with power 170 and 160% action time adds only about 6% damage per unit of
  time, but it halved fight length: the monster died in 2 hits instead of 4 and
  got half as many turns. Tune skills against hits-to-kill, not only damage rate.
- **Speed helps unevenly in short fights for the same reason.** Under the
  earlier values, with 4-action fights, speeds 110, 120 and 140 gave identical
  results. With the current values, 100, 110, 140 and 200 give 22%, 17%, 12%
  and 8% HP lost for a level-5 warrior against a beast; 110 and 120 still tie.
- **Balance must be judged against more than one opponent.** With only
  physical attackers, the mage's special defence barely counted and the warrior
  looked sturdier than it is. The same comparison against a special attacker
  reversed several results.
- **A weak identity target misleads a search.** "Neither build dominates" passed
  when the mage won 13 comparisons to the warrior's 4, because a single edge was
  enough. Searching for robustness under that target drifted towards one build
  leading everywhere. Requiring each build to win a share of the comparisons
  fixed it.
- **The basic attack's channel belongs to the character.** With a universal
  physical basic attack, a spirit without physical attack did 1 damage per basic
  attack and its bosses were trivial.
- **Packs need a minion tier.** Two same-level ordinary monsters cost a level-5
  warrior 63% of its HP; two minions cost 12%.
- **Flat MP costs need authored MP growth to match.** With costs flat and the
  mage's pool growing 10% a level, a level-20 mage fits about six times as many
  bolts per rest; the warrior then won only 1 of 13 comparisons. Keeping the
  mage's maximum MP at 75 on every level restores the balance: casts per rest
  and regeneration per cast depend only on pool ÷ cost, so this is the same
  sustain the earlier level-scaled costs produced, and the report is unchanged
  but for one rest in two grinding lines. A world that wants casters to gain
  sustain with level lets MP grow; one that grants 内力 lets it grow by grant.
  With the sample content, MP growth of 1% a level keeps every target; at 1.5%
  the warrior already wins only 5 of 16 comparisons, so faster MP growth needs
  compensation elsewhere, such as costlier later tiers or more for the warrior.
- **Grinding needs XP that falls off with level difference.** With ±10% XP per
  level of difference, capped at ±40% and rounded down, and nothing from monsters
  five or more levels below,
  farming level-1 monsters stalls at level 6. Without the falloff it reaches
  level 10 in 225 safe kills.

### Speed: current recommendation is a paused initiative timeline

Round-based ordering is simple: each actor acts once per round, fastest first.
Its limitation is that extra speed does nothing after an actor already outranks
all opponents. Our discussion has reopened that choice; a timeline is the current
recommendation, pending agreement on its practical rules.

On a timeline, each actor has a next-action timestamp in **encounter timeline
units**. The engine advances directly to the next actor. It pauses for player
input; thinking for five minutes advances neither encounter time, optional world
time nor story phase. Enemy turns can resolve automatically until the player is
ready again. This is still turn-based play, not a reflex game.

Encounter timeline units have no intrinsic conversion to wall-clock seconds or
optional world time. If an authored encounter should consume world time, that
must be an explicit effect; never derive calendar progression by summing combat
ticks.

A starting proposal is:

```text
effective_speed = clamp(speed_after_all_modifiers, 1, speed_cap)
delay = max(1, ceil(action_cost / effective_speed))
next_action_time = current_action_time + delay
```

Action cost and the speed cap are positive integers. `action_cost = 10,000` is an illustrative
scale, not a finalized balance constant. Initially all actions can share that
cost; faster/heavier skills only need separate costs when the design uses them.
Schedule the first action after one delay so speed matters immediately.

Example with equal action costs: your speed is 100 (delay 100), and a wolf's
speed is 200 (delay 50). Suppose the stable tie-breaker puts you first:

| Virtual time | Action |
| --- | --- |
| 50 | Wolf acts |
| 100 | You act, then wolf acts if still alive |
| 150 | Wolf acts |
| 200 | You act, then wolf acts if still alive |

The wolf gets two actions per 100 time units. Reverse the speeds and you get the
extra turns. Nothing happens while the player is choosing an action. Show the
next few turns in the UI so consecutive enemy actions are understandable.

This makes speed powerful: it improves damage output and access to healing,
items and other actions. Balance it alongside attack and defence. Integer rounding
creates speed breakpoints; bound supported speeds and choose enough timeline
resolution to avoid unintended plateaus. A hard effective-speed cap is agreed;
its numeric value must be chosen through combat balancing. Diminishing returns
are not currently required.

Tie-breaking, MP timing, later speed changes and dead actors have proposed
answers in [Combatants and encounters](#combatants-and-encounters--proposed).
Rejected commands must not advance time, and save/load must preserve the schedule
exactly. The initial version need not include speed buffs.

#### Speed effects to discuss

For an unchanged action cost, action frequency is approximately proportional to
effective speed until the cap. These are long-run rates, not guaranteed action
counts in a short fight:

| Speed | Delay with cost 10,000 | Relative action frequency |
| --- | --- | --- |
| 50 | 200 | 0.5× |
| 100 | 100 | 1× |
| 150 | 67 | Approximately 1.5× |
| 200 | 50 | 2× |

Proposed separation of responsibilities:

- Attack and skill power determine damage per hit; speed determines opportunities
  to act. Do not also add speed directly to damage without a specific skill rule.
- Speed does not inherently grant dodge, accuracy or critical chance. Those are
  independent mechanics if the game eventually needs them.
- A faster actor can spend MP and consumables sooner, but does not automatically
  gain more resources or faster passive regeneration.
- Action cost can express commitment: a quick strike could cost 6,000, a normal
  attack 10,000, and a heavy strike 16,000. These are illustrative values, not
  approved skills or balance numbers.
- A simple initial model resolves the chosen action immediately, then uses its
  cost to schedule the actor's next turn. That cost is recovery time. Casting
  wind-up and interruptible actions would require separate, explicit rules.

Under this proposal a speed-100 actor making a heavy strike waits 160 ticks for
their next turn; a speed-200 actor waits 80. The first turn still uses the common
opening cost, since no action has yet been selected. The player always has time
to read and choose: virtual recovery does not mean waiting in real time.

#### Armour trade-off and speed cap · direction agreed

Heavier armour can exchange speed for protection. Apply equipment bonuses,
armour penalties and any future status modifiers before clamping effective speed.
The cap governs scheduling for players and monsters alike; gear or buffs must not
bypass it. Display the effective value and indicate when it is capped.

The proposed scheduling floor is one, preventing division by zero or negative
delays. Resolve penalties with checked arithmetic that cannot underflow. A future
stun should explicitly prevent acting rather than represent speed as zero.

The exact cap and armour penalties remain open. For example, a baseline of 100
and a cap of 200 would limit speed alone to twice the baseline action frequency
for equal-cost actions. This is a tuning candidate, not a finalized rule. A cap
does not itself bound consecutive turns against a much slower opponent or remove
the advantage of cheaper actions; test those combinations too.

Add acceptance checks for values below the floor, above the cap, armour removal,
and stacked bonuses. Save/load and replay must preserve the same effective speed
and schedule. Additional speed above the cap grants no further action frequency.

Open balance choices include how much speed equipment grants and the precise
order/composition of any future percentage modifiers.
Compare damage and resource use across the same virtual-time interval, alongside
survival and actual encounter outcomes. High speed may dominate if it costs no
attack, defence or other build investment; do not treat all stat points as equally
valuable merely because they are integers.

### Damage: two channels, gradual defence · proposed formula

Use gradual defence scaling rather than subtracting defence directly from attack.
Explicit immunity and vulnerability are a separate layer from ordinary defence.

There are two damage channels, **physical** and **special**. A world gives
special its own name and meaning: magic, 内力, mana, 气机. Each skill has one
channel. The stats matching that channel dominate, and the other channel's
stats add a smaller authored share, so deep 内力 also turns aside some physical
blows. A world that keeps the channels strictly apart sets the share to zero.

Proposed calculation for a positive-power damaging hit that has landed:

```text
S = cross share as an integer percentage; world default, a skill may override
A = 100 × matching attack  + S × other-channel attack
D = 100 × matching defence + S × other-channel defence
P = skill power as an integer percentage; basic attack = 100
M_num / M_den = resolved incoming-damage multiplier for the skill's channel
               normal = 1/1; immunity = 0/1; vulnerability = 2/1

if M_num == 0:
    damage = 0
else:
    damage = max(1, floor(A × P × A × M_num
                         / (100 × 100 × (A + D) × M_den)))
```

A and D carry a factor of 100 so the cross share adds no rounding step. Require
nonnegative defence, a multiplier numerator from 0 to 10 and denominator from
1 to 10 (see [Engine bounds](#stat-ranges-and-caps--proposed)), a share from 0 to 100, and valid positive attack/power for this damaging-hit
calculation. Round once, at the end, with checked wide intermediate arithmetic.
Cap actual HP loss at remaining HP. Healing, non-damaging skills and any future
misses use separate rules. **Immunity overrides minimum damage:** an immune
target takes zero, never the fallback one point.

Defence equal to the combined attack halves damage, and there is no separate
scale K. Examples with physical attack 40, special attack 20, power 100%, a 25%
share and a normal 1× multiplier (so a physical hit's combined attack is 45):

| Defender | Physical hit | Special hit (combined attack 30) |
| --- | --- | --- |
| No defence | 45 | 30 |
| Physical defence 45 | 22 | 21 |
| Special defence 80, no physical defence | 31 | 8 |
| Physical defence 20, no special defence | 31 | 25 |

The third row is deep 内力 blocking a physical blow as well as 20 points of
physical defence would. The formula reads only stats, never character level,
so power from techniques, grants or equipment counts exactly like power from
levels. The earlier proposal divided by `K + D` with a world-level K; the
[balance simulation](#findings) showed that a fixed K lets fights drag at high
levels and a level-scaled K penalises strong low-level characters.

M3 needs the two channels with a fixed 1/1 multiplier. Immunity, vulnerability
and modifier stacking described below arrive in M4 with the equipment that
needs them.

#### Exceptional armour: immunity and vulnerability

An armour can combine ordinary defence with explicit damage-type modifiers:

| Incoming type | Multiplier | Meaning |
| --- | --- | --- |
| Physical | 0× | Completely immune to physical damage |
| Special | 2× | Twice the damage that would otherwise pass through its defence |
| Other | 1× | Ordinary defence reduction |

For example, a special hit with an exact post-defence value of 40 deals 80 under
the vulnerability. A physical hit deals zero regardless of its attack value.
Multiply before the single final rounding step, rather than rounding the
post-defence value first.

Special is now the second channel, whatever a world calls it, so the example
needs no extra category. A world that needs a finer distinction, such as
illusions that ignore armour, adds it as an explicit immunity or bypass rule
rather than a third channel. **Double damage and defence bypass are distinct properties**:
vulnerability does not silently ignore defence. An explicit bypass rule would
omit the defence reduction while still respecting separately specified immunity
rules; immunity-piercing, if ever needed, would be another explicit rule.

The combination rule for several pieces of gear/status effects is still open.
Do not sum or multiply modifiers by accident. Specify immunity precedence and
vulnerability/resistance stacking before allowing multiple sources in a build.

## M4 — Equipment, skills and character builds · complete

Scope includes both defining equipment during world authoring and letting players
forge, improve and enchant it. See the [equipment and crafting proposal](docs/equipment.md)
for shared definitions, individual item instances, recipes and the first playable
crafting journey. Its detailed rules remain proposals.

Equipment does not imply crafting. Forging, improvement and enchanting are
independent optional world capabilities. Source-derived worlds include them only
when grounded in the source material; otherwise their data and UI are absent.
The M4 demonstration world proves the reusable systems without enabling them in
every package.

- Add explicit authored equipment slots, equip/unequip, consumables and learned
  skills. Items may occupy multiple slots; do not require a universal global slot
  list before representative worlds need one.
- Derive effective stats from base progression plus equipment; prevent repeated
  equip/unequip from permanently accumulating bonuses.
- Give skills authored descriptions, costs, damage channels, power and
  [technique ranks](#technique-ranks--decided). Introduce cooldowns/status
  effects only alongside skills that require them.
- Add immunity/vulnerability multipliers, specify modifier stacking and immunity precedence, choose armour speed
  penalties, and add optional caps for stats other than speed if stacking needs
  them. **Decided (M4c):** several modifiers on one channel multiply; any
  immunity wins (zero damage); the combined multiplier is kept within 1/10 to
  10. Each armour piece authors a flat speed penalty; penalties add up and
  apply before the speed cap, and effective speed never drops below 1.
  **Consumables**: an item restores HP and MP when used,
  exploring or as a fight turn. Cooldowns and status effects wait for a
  skill that needs them.
- Define durations in terms of the chosen combat clock; do not casually mix
  wall-clock seconds, actor turns and timeline units.
- Show why stats changed and what an action costs before confirming it.
- Deliver in slices: M4a player-allocated stat points (delivered); M4b technique
  ranks (delivered); M4c equipment instances and equip/unequip (delivered);
  M4d stations, deterministic
  forging and authored improvement-state transitions (delivered); M4e one compatible
  learned enchantment per item (delivered). Runtime instances use deterministic
  saved IDs only when distinguishable copies need independent state; unique
  legendary equipment may still have one mutable instance, while fungible
  identical resources remain definition + quantity. Materials/quality are concrete
  authored data rather than a universal runtime hierarchy. Preserve authored
  source-language names and prose throughout crafting.
- **Player-allocated stat points · decided, a core part of M4** and, like
  every combat part, optional per world. Content authors points gained per
  level, which stats accept them and how much one point is worth, plus a respec
  policy. Saves store the allocation, and effective stats are derived as level
  table + allocation + technique passives + equipment, never saved. With flat MP
  costs, points poured into MP mean near-unlimited casting, so allocation needs
  per-stat caps or a low MP value per point; the balance simulation gains a
  target that no single-stat build dominates when balancing resumes. Worlds
  whose sources grow power through cultivation, such as wuxia 内力, may use
  technique ranks instead or as well.
- Keep recipe knowledge separate from proficiency: authored teachers, plans,
  quests or discoveries grant recipes, while smithing determines whether a known
  recipe can be used. Proficiency alone does not reveal recipes initially.

### Technique ranks · decided

Wuxia sources measure power by how deeply a technique is mastered: 龙象般若功 has
ten layers, 九阴真经 is learned layer by layer, and 郭靖 learns 降龙十八掌 one
move at a time. Instead of a separate realm-tier system, each learned technique
has a rank.

- **Rank tables.** Each technique authors one entry per rank: a name, power,
  cost, action time, and optionally passive stat grants and a changed weapon
  requirement. The world author names every rank of every technique, in the
  source's own terms (第一重 … 第十重 for 龙象般若功, one move's name per rank
  for 降龙十八掌, "Novice" … "Master" elsewhere); clients show that name, never
  a bare number.
  Internal arts (内功/心法) are passive techniques whose ranks grant stats such as
  maximum MP or special attack. A sword art's top rank may drop its weapon
  requirement, as 独孤求败 moves from 木剑 to 无剑. Each rank authors its own costs.
- **Rising through use.** Using a technique in an encounter earns technique XP,
  with the same falloff by level difference as character XP, so practising on
  weak opponents stops paying. Nothing is earned outside encounters.
- **Passive arts rise slowly with experience** (decided). An internal art has
  no skill to use, so each victory gives it an authored share of the character
  XP earned, typically small, so cultivation deepens much more slowly than
  levels.
- **Rising through teaching and events.** Authored effects from masters,
  manuals, quests and 奇遇 grant a technique, set its rank directly, or grant
  technique XP.
- **Breakthrough gates.** Reaching a rank may require authored conditions such as
  a flag, quest state or story phase: 九阴真经's later layers need the second
  volume. Technique XP stops at the gated threshold until the gate opens, so the
  per-chapter level cap becomes a per-technique cap.
- **Realms.** A world may name one technique as the character's core internal
  art. Its current rank's authored name is then displayed as the realm (境界),
  and conditions can test the rank. No separate tier system exists.
- **Roles.** Where a world has character levels, they supply base body stats
  and experience; techniques supply skill power and passives. A high-rank
  internal art can make a level-1 character strong, because power comes from
  stats rather than level. A world may also drop character levels and progress
  through techniques alone. Two progression axes double the balance surface, so
  the balance simulation must model ranks before their numbers are chosen.
- **Saves** hold each learned technique's rank and technique XP. Derived stats
  are recomputed from level, technique passives and equipment, never saved.
- A technique rank is not a proficiency in the
  [Section 8](docs/open-decisions.md#8-checks-and-proficiencies) sense: it belongs
  to the learned technique, and RealmKit still has no universal skill table.

**Done when:** at least two meaningfully different builds can finish a short
adventure, with tested equipment/resource rules and readable combat feedback.
The forge → equip → improve → enchant → save/load journey preserves individual
item identity and consumes resources atomically without duplicating bonuses.

## Presentation track · in progress

Improvements to the terminal client, delivered in small PRs between
milestones. They change how play reads, never the engine or its rules.

1. **Grouped menus** · delivered. Stat training and equipment each wait behind
   one entry (`Train stats — 3 points ›`, `Equipment ›`). The submenu is
   headed by its name, ends with `Back` (Esc works too), and stays open while
   its actions are used, so several points can be trained in a row.
2. **Quieter combat log** · delivered. Technique XP earned during a fight
   is shown as one line when it ends (`Technique XP: Cloud Palm +20, Azure
   Breath +1`), whether it was won or fled. Rank-ups still appear at once,
   and a reward without XP no longer prints `+0 XP`.
3. **Multi-line status** · delivered. The character panel shows the level and
   realm, then vitals, stats and progression on their own lines, with the XP
   the next level needs and any points waiting to be spent.
4. **Light styling** · delivered. In a terminal, names, headings and the
   selected entry are bold, growth is green, criticals yellow and death red,
   and key hints are dimmed. Pipes, `--line` and `NO_COLOR` get plain text.
5. **Fight screen** · delivered. In terminal play a fight takes the alternate
   screen, redrawn each turn: health bars for everyone, the next turns, the
   last six lines of the fight (the latest turn marked) and the menu. When
   it ends, scrollback keeps only the opening line and the final turn. Line
   mode and pipes keep the full log.
6. **Overland map** · delivered, **original-game P0**. A map of
   places and roads drawn from authored display positions, with zoom,
   panning and labels that make room for each other, and a fixed view in
   line mode ([Overland map](docs/sandbox-worlds.md#overland-map)). Unlike
   the items above, it needs a package field for positions and an engine
   query. As delivered: characters who move are left
   out until knowledge and news exists; travel to a chosen place waits for
   travel speed and world agents. `known_when` keeps a place
   off the map, and off the roads, until the player learns of it.
7. **Embeddable client surface** · delivered, **original-game P0**.
   The engine stays presentation-agnostic; a non-terminal host loads a
   package from memory (`WorldSpec::from_files`), inspects views and actions,
   executes typed commands and imports/exports snapshots as JSON bytes, with
   commands, events, actions, the map view and the journal
   serializable, and
   `realmkit-engine` builds for `wasm32-unknown-unknown`. No adapter crate:
   the React/Tauri application and its wasm-bindgen or IPC glue belong
   outside RealmKit. No generic UI framework is added to the engine.

## M5 — Longer authored adventures and source-specific mechanics

- Add runtime instances for combatant copies whose state outlives an encounter,
  such as a wounded creature that roams. M3 already supports several copies within
  one encounter and keeps opponent HP there rather than in one global record. Use
  instances only where distinguishable copies require independent mutable state;
  do not instance every authored entity.
- Generalize the core quest model into main and side questlines with typed
  multi-target objectives, prerequisites and chains. Side questlines are unlocked
  by explicit main-story/story-phase conditions and may feed explicit state back
  into later main quests.
- Add respawn delays measured in World Time for worlds that enable it. M3
  already provides immediately repeatable groups and fleeing.
- Expand authored dialogue and branches, with consistent NPC availability and
  understandable journal entries.
- Make Inventory genuinely optional in runtime/spec/presentation rather than
  retaining placeholders. An inventory-free world has no inventory state,
  no Inventory command/action/panel, and no required inventory/item definitions
  merely to satisfy the engine.
- Add source-specific capabilities only with a representative world. For example,
  a detective story may use a main investigation questline with clues, evidence,
  interviews, contradictions, deductions and a final accusation while omitting
  combat and inventory entirely. Add a physical evidence item only when the story
  needs one; link it to investigation evidence through a typed reference rather
  than treating evidence as inventory. Its success and failure paths remain
  deterministic and pre-authored.
- Establish first-class authored endings and route completion state, rather
  than treating player death as the only terminal outcome.
- Keep story-phase transitions event-driven. Add optional World Time only when a
  representative world needs travel durations, schedules, day/night, rest tied to
  elapsed time, appointments or deadlines. Represent it as minutes from an
  authored epoch that are monotonic within one forward committed history, advance
  it only explicitly, and restore the saved value exactly when rewinding to an
  older snapshot. Resolve crossed scheduled events chronologically with stable
  authored order for ties. Do not assign universal durations to ordinary
  commands.
- Support authored stochastic world-event opportunities when a representative
  world needs rare encounters/discoveries. Trigger rolls only at explicit gameplay
  transitions and select only among pre-authored outcomes.
- Treat spatial navigation as a current-format assumption rather than a permanent
  universal. Do not generalize it away until a representative non-spatial world
  needs that change. A later career/life/institution simulation should be able to
  use calendar, economy, relationships, reputation, authored events and
  progression without fake rooms or directional traversal merely to satisfy the
  engine. See [the non-spatial simulation note](docs/simulation-worlds.md).

### Delivery slices · proposed

M5 also lays the general foundations that
[living sandbox worlds](docs/sandbox-worlds.md) need, and M6 depends on
these slices. The investigation fixture, optional Inventory and authored endings
above are delivered alongside them, in whichever order a fixture needs them.

For the original game, the identifiers below describe scope rather than a strict execution
order: **M5f start choices** (delivered) and the authored-progression part of **M5c** are P0,
and M5e is pulled forward as soon as repeated social roles or state-sensitive
dialogue in the opening town would otherwise duplicate content.

1. **M5a — condition tree and effect lists · delivered.** `requires` becomes
   one condition composed with `All / Any / Not` over typed leaf predicates,
   and a dialogue choice carries an ordered list of `effects` in place of one
   optional `effect`, applied atomically. Numeric predicates stay
   domain-typed, as [decided](docs/open-decisions.md#6-conditions-and-effects).
   As delivered: an `item` predicate for carried counted items and
   `grant_items`/`take_items` effects arrive with it, and
   `examples/quiet-archive` uses `any`, `not` and multi-effect choices.
2. **M5b — world time, travel and recurring schedules.** Optional World Time
   as specified above; roads with authored travel durations and exits beyond
   the six compass directions; wait and rest actions that consume time; and
   recurring schedules that the dispatch cursor expands lazily, one due
   occurrence at a time; and characters who move among authored locations on
   a schedule, drawn from a `world` RNG domain. **Delivered**:
   undirected roads with minutes, conditions and blocked text, mixable with
   compass exits; `wait` with an authored menu step and `rest` with an
   authored duration; one-shot and recurring authored events whose effects
   are flags, items and technique grants; movers; and a `time_of_day`
   condition. Only the minute and movers' locations are saved, since every
   next occurrence follows from the minute. Effects cannot start or stop a
   schedule; conditions gate what an occurrence does. `examples/marches`
   begins here. Follow-up: a character who appears or leaves because the
   clock entered or left its `time_of_day` hours, and a market that opens
   or closes with its merchant, change silently; report them as movers'
   arrivals are reported. **Extended** with an optional proleptic Gregorian
   calendar: `time.calendar.epoch` dates minute 0, and the clock may show
   `{year}`, `{month}` and `{day_of_month}`. WorldTime remains elapsed
   minutes, the calendar position is derived and never saved, and there is
   one time source of truth. The spec finds the next day and month
   boundaries after a minute, which the original game's financial
   settlement will use: banking interest, monthly payroll, salaries and
   holding settlement, none delivered yet.
3. **M5c — authored progression and quest lifecycle · partly
   delivered**: story phases moved only by `enter_phase`, `main` quests,
   quest `requires` for chains and phase-gated waves, and route outcomes
   recorded once (completion only, non-terminal); validation rejects
   prerequisite cycles, main quests waiting on side quests and phases
   nothing enters. Still proposed: the rest of this item. Generalize the current
   quest table into explicit story phases, one route-owned main questline and
   optional side questlines with typed prerequisites/chains; side outcomes may
   feed explicit state back into later main progression. Add first-class authored
   route outcomes/completion state. Quests may fail, carry world-time deadlines
   or be abandoned. Repeatable offers remain activities rather than side
   questlines: their parameters are drawn by the saved RNG from authored
   candidate lists at an explicit gameplay transition, such as a recurring
   refresh, never when a menu opens.
4. **M5d — runtime instances** for spawned copies and parties whose state
   outlives one encounter, as in the first bullet above. Unique authored
   characters keep their authored IDs and carry state such as wounds or
   captivity under them.
5. **M5e — dialogue by role, text variants and languages · languages
   delivered**. Authored roles whose dialogues and topics every
   member shares, `Speaker` references in conditions, effects and templates,
   line slots that each member fills, role offer givers, and text fields with
   conditional variants are still proposed. Delivered: a package may carry
   complete overlays of its prose in other languages (`text/<tag>.json`,
   keyed by each field's JSON path), chosen when the package is loaded, and
   saves bind to a revision of the rules alone, so they move between
   languages and survive prose fixes. Also delivered, from the original
   game's dialogue: choices asked `once`, `back` choices that become
   the hub's own leaving when its questions are used up, and `blocked_text`
   that lists an unmet choice with its reason, the way a locked exit is.
6. **M5f — start choices · delivered**. A route's authored start
   questions at New Game, each option an ordered effect list applied to the
   initial state, with the chosen option IDs saved for display. Every
   question is asked, in order; an answer never skips or adds one.
   `examples/quiet-archive` asks why the player came.
7. **Investigation slice · delivered**. Evidence definitions,
   optionally linked to an item; `discover_evidence` and an `evidence`
   condition; `EvidenceDiscovered`; evidence in saves and the journal.
   Authored conditions decide what evidence supports; there are no
   deductions, contradictions or accusations yet. `examples/caravan-trail`
   reaches its conclusion by several evidence paths, and the quiet archive
   gates Pell's thanks on the stitched map. Each evidence may also give its
   source, fixed facts and interpretations that later evidence, flags or
   phases unlock, reported as `EvidenceReinterpreted`: the evidence stays the
   same while the player's understanding of it changes. A fact may wait on a
   lasting `when`, so evidence found at the first observation fills
   in as the investigation goes on (`EvidenceFactLearned`), and an
   `at_least` condition means "any two of these readings" needs no list of
   pairs.

**Done when:** longer hand-authored fixtures demonstrate branching progression
and tested paths to completion, can be saved/resumed, and include at least one
inventory-free non-combat fixture that loads and plays with no inventory state,
definitions/placeholders, Inventory command or inventory UI. A dungeon is useful
only for a world that needs one.

## M6 — Living sandbox worlds · proposed

Let a world keep moving without the player: an overland realm of rival
kingdoms where the player trades, hires soldiers, earns standing, holds fiefs
and joins orders while lords, bandits and caravans act on their own. The
[sandbox design direction](docs/sandbox-worlds.md) describes each capability;
[Section 18](docs/open-decisions.md#18-living-sandbox-worlds) of the register
tracks its open questions. Every capability, and every part inside one, stays
optional and ships with the slice of the reference fixture that proves it.
Hard dependencies between capabilities are few and validated, interactions
happen only when both sides are present, and the full sandbox is simply the
world that picks all of them ([Modularity](docs/sandbox-worlds.md#modularity)).

Each slice depends on the M5 foundations it uses, following the hard
requirements in [Modularity](docs/sandbox-worlds.md#modularity): the economy
needs only M5a's effects and M5b's schedules, so it came first. Deliver in
slices, each bumping the package and save format as usual.

The original game now supplies the concrete ordering pressure. After the already delivered
economy and first troop/battle slices, prioritize **M6b factions/standing**, the
remaining **M6c** retinue/companion/travel needs, then **M6e holdings/sieges**,
**M6f world agents**, **M6g faction strategy**, and only the minimal **M6h**
politics/order composition that the game's authored content requires. M6d additions
such as prisoners, injuries and champion duels are not blockers unless the
original-game fixture actually uses them.

1. **M6a — economy.** Currency; markets whose per-good price index follows
   authored production and demand on a recurring price tick; linked markets
   that converge; prosperity; merchants who restock; buy and sell that move the
   index, with a price preview; player workshops; and upkeep on recurring
   schedules. Trading is the first proficiency, so the proficiency mechanism
   arrives here too: points per level, personal or party proficiencies, and
   study from items over world time. The price tick is mirrored in
   `scripts/combat_sim` before tests pin its numbers. Delivered in two parts.
   **M6a-1 · delivered**: currency, goods, producer kinds,
   town and village markets with authored starting indices, the four-phase
   price tick on its own `market` stream, links, merchants who must be
   present, buying and selling with a spread, per-unit index steps and an
   engine-computed preview, and currency conditions and effects;
   `scripts/combat_sim/economy.py` mirrors the tick and computes warm-up
   prices for authors; `examples/marches` trades. Wares, items a market
   sells at a fixed price, arrived early with consumables.
   **M6a-2 · delivered**: prosperity that drifts daily towards
   an ideal lowered by scarcity and scales demand and stock; merchants'
   stock and purse, restocked from their own `stock` stream, limiting
   trade both ways; villages that feed their market town; workshops bought
   and sold in dialogue and settled weekly at local prices, with their
   overhead as upkeep beside the retinue's wages; and the trading
   proficiency, which brings the proficiency mechanism: points from the
   level table, ranks taught by effects, and a `proficiency` condition.
   `examples/marches` sells a weavery. Still to come, with the
   capabilities they need: party proficiencies (companions), studying
   items over world time, a wider spread for unneeded goods and disliked
   merchants (M6b), and caravan and village-trade prosperity (M6f).
2. **M6b — factions and standing.** War and peace between factions, authored
   standing tracks such as renown and relation, and the conditions and effects
   that read and change them. Authored personality traits steer dialogue and
   drive reactions to player deeds, either detected by the engine or reported
   by an authored `ReportDeed` effect. Proficiencies such as trading,
   leadership and surgery belong to the capability that uses them.
3. **M6c — retinue.** Troop definitions with wages and upgrade paths, a roster
   of counts and saved XP pools per troop type, companions as unique
   characters who wear their own gear from a shared stash, recruiting, an
   engine-computed roster limit, an upkeep tick with its own RNG domain for
   wages, provisions, wounded recovery, morale and desertion, recruit pools
   that refill on a schedule, and travel speed from the slowest troops, the
   party's mounts and its load.
4. **M6d — mass battle.** Army-against-army resolution from rosters, leaders and
   ground (authored on locations and roads): stacks of troops fight in rounds
   with an authored frontage, class matchups, morale and a rout, with seeded
   casualties and an optional champion duel. The player chooses to autoresolve
   a battle or command it round by round with a closed set of orders (charge,
   hold, flank, retreat); both run
   the same round rule. The formula is mirrored in `scripts/combat_sim`
   before tests pin its numbers. Losses split into killed and wounded, and
   wounded troops recover on the retinue's upkeep tick or, for agent parties,
   the world-agent tick. Battles take prisoners: troop prisoners to sell or
   recruit, captured lords to ransom, escapes on a schedule, and player
   captivity as a setback rather than an ending. Holding prisons arrive with
   M6e. Victories grant XP, standing and a loot pool drawn from authored loot
   tables; defeats may inflict authored lasting injuries.
   **M6c-1 and M6d-1 · delivered**: troop classes and lines
   whose soldiers level up in squads, renamed where authored, with branch
   upgrades from a line's last level; recruiting from pools that refill;
   an upkeep schedule for wages, desertion when unpaid and recovery of the
   wounded; mass battles against authored armies, autoresolved or commanded
   with charge, hold, flank and retreat under the round rule
   `scripts/combat_sim/battle.py` mirrors; allies who join while a
   condition holds, ahead of factions; wounded and killed losses, the
   player knocked out rather than killed, and victory XP shared with the
   squads that fought. `examples/marches` raises levies and beats the fen
   outlaws. Still to come: companions, provisions, persistent morale,
   travel speed, leaders and companions as individuals, ground, prisoners,
   injuries and loot tables.
5. **M6e — holdings and sieges.** Ownership that changes during play, income,
   garrisons, authored buildings, sieges as mass battles after a preparation
   time, and raiding enemy villages for loot at the cost of their income.
   Villages add rent collected in person with a risk of unrest, demanding
   supplies, herds of livestock, and bandit trouble cleared through offers.
6. **M6f — world agents.** Party instances on the road graph that apply a
   closed set of engine policies on recurring world ticks, with priorities
   that may test the leader's traits, routing by shortest travel time with ties
   broken by authored road order, spawners with caps, interception of the
   player, off-screen battles, weighted diplomacy events, and an
   `AGENT_BOUND`. Caravans trade along authored links. The player learns of
   off-screen change through notable events, a saved news journal, and
   remembered prices and whereabouts.
7. **M6g — faction strategy.** A faction tick with its own RNG domain;
   defend, gather, campaign, raid and rest stances chosen by authored
   priorities; a
   marshal whose army members follow; campaign and raid targets on the road
   graph; fief grants; defection; truces and war-weariness inputs to diplomacy;
   feasts; and claimants backed through dormant factions. These are closed
   engine rules, not a planner or a model.
8. **M6h — politics and orders by composition.** Faction membership ranks for
   vassalage and knightly orders, chapter houses as buildings, courtship and
   marriage on relation tracks and dialogue, and a player-founded kingdom as an
   authored dormant faction.

**Done when:** a small original fixture (`examples/marches`) plays 30 or more
in-world days in which parties move, prices drift with production, and a
marshal's campaign takes a holding and the ruler grants it, all without the
player, while the player trades, recruits, wins a mass battle and
receives a fief. In the fixture the fief is the top ambition, so it records the
route's completed, non-terminal outcome, and play continues. Lower rungs of the
ambition ladder are ordinary main-questline quests, because a playthrough
records at most one outcome. The
same seed and commands reproduce it exactly, including across save and load.
Trimmed variants of the fixture that drop capabilities, such as one with only
travel, time and the economy, and one without world agents, still validate and
play, and a world missing a hard prerequisite is rejected with a stable
diagnostic.

## M7 — Authoring feedback and deterministic simulation

- Extend typed authoring operations where real content workflows need them.
- Add structured diagnostics for unreachable objectives, unavailable required
  items, unsatisfied prerequisites and invalid story/dialogue links.
- Keep structural validation, bounded reachability analysis and executable
  simulation distinct. Hard structural/invariant failures are errors; apparently
  unreachable optional/secret content is normally a warning, while required
  progression that the available analyzer proves unreachable is an error.
- Expose deterministic progression simulation through the real engine with
  explicit starting state, route, player policy, RNG seed/state and assumptions.
  Add combat simulation only for combat-enabled worlds. Report outcomes, costs
  and blocking conditions.
- Treat simulation as evidence, not a theorem: one failed policy/seed is not proof
  of impossibility, and one successful path does not prove every branch.
- Add an authoring CLI or MCP adapter over the same core when there is a consumer.

**Done when:** an external agent can author, validate, simulate, read structured
feedback, repair, and export a playable package without editing serialized text
as its primary API. A small combat simulator may be brought forward to tune M3.

## M8 — Source-grounded world generation

- Provide world-builder instructions for external agents; keep model providers
  optional and outside gameplay.
- Introduce canon IR only as source adaptation needs it: identities, chronology,
  relationships and evidence, separate from runtime NPCs and quests.
- Retain source references for reviewing fidelity and regenerating selected content.
- Ask the user to choose a canonical protagonist route, an original-character
  route, or both. In `both` mode, New Game offers the choice immediately; one
  route does not have to unlock the other.
- Reuse the same shared world and canonical timeline across routes where possible.
  Keep player-route-specific main questlines, starting state and saves separate.
  In the original route, canonical protagonists remain world entities/NPCs and
  continue through protected canon anchors.
- For each PlayerRoute, author exactly one route-owned main questline. A
  canonical route derives its main questline from the relevant canonical story
  while preserving characterization and atmosphere; an original-character route
  receives its own authored main questline within the same canon constraints and
  shared story phases. Generate side-story seeds from canonical NPCs, locations,
  factions, relationships, occupations, conflicts and unresolved details; expand
  those seeds into shared or route-gated side questlines before inventing
  unrelated generic content.
- Gate side questlines by explicit main-story/story-phase progress. Side quest
  outcomes may alter later main dialogue, routes, assistance, objectives and
  bounded outcomes through authored state, but never through runtime generation.
- Do not turn every named character into a monster.
- Select the game's capabilities from the source. A detective novel may compile
  to exploration, interviews, evidence, deductions and accusation branches with
  no combat system. Do not add fights merely to satisfy an RPG convention.
- Enable optional gameplay systems such as forging and enchanting only when the
  source mentions or supports them. Absence in the source produces absence in the
  package and player interface, rather than generic RPG filler.
- Generate every player-facing string in the source language, including the menu
  action labels added in M1. Review actual text, not just language metadata.

**Done when:** a short source produces an inspectable world whose provenance,
language and fidelity can be reviewed, whose main progression is tested, and
which remains playable after removing all generation tools and source files.
Normal source-backed play is available through at least one client whose generic
UI text supports the package/source language, or through explicit package-provided
setting-specific UI overrides; missing fixed-interface translations must not
silently fall back to English. Every packaged PlayerRoute has at least one
successful real-engine simulation to a completion outcome. For a two-route
fixture, both routes are selectable from New Game, share the intended world/canon
data, and validate and simulate independently from their own starts.

## M9 — Shared play and authority handoff · optional later

Richer player-facing clients are no longer parked in this milestone: the
[presentation track](#presentation-track--in-progress) owns the embeddable
client/map work needed by the offline original game.

M9 is specifically the later **shared play** path of the original game. Solo
play remains fully local and playable without a server. When shared play becomes
active work, introduce an authoritative server as an adapter around the same
deterministic command/event rules. The server owns canonical shared state, RNG
resolution, authoritative command ordering and durable persistence; clients submit
commands rather than outcomes.

Serialize commands that touch the same mutable scope, while allowing independent
world/player/session/encounter scopes to progress concurrently. A pending human
turn pauses only its relevant encounter/session scope, not the whole server.
Timeouts/disconnect handling may submit predefined fallback commands but wall-clock
waiting does not itself advance gameplay time. Reconnect from current server
state; never rewind a persistent shared world from a client snapshot.

The local-to-shared transition is an explicit trust boundary. The first prototype
should prefer a versioned deterministic replay/import artifact (package revision,
initial seed/state and committed local commands, or an equivalently reproducible
record) that lets the server validate the state accepted at Ascension instead of
trusting a mutable client save. This proves mechanical reachability, not that a
human personally played every command; stronger anti-cheat is only justified if
competitive stakes later require it.

Choosing shared play forks the local history. Offline solo play and the
server-owned shared history may both continue from their common ancestor, but
they are never automatically merged. Returning from shared play uses state
owned by that server-side history and its recovery scope.

Add storage, authentication, networking and server crates only when this
milestone becomes active work. Steam/itch/web identity and hosting policy are
product concerns, not RealmKit core abstractions.

**Done when:** an offline playthrough can cross a tested authority-handoff
boundary into a server-owned shared character/world state; clients cannot bypass
engine rules; concurrent authoritative scopes have reproducible ordering tests;
reconnect restores current server state; and no multiplayer feature requires
runtime AI.
## Decisions to discuss next

Resolve decisions immediately before the original game's slice that consumes them. The
complete cross-project register remains in
[docs/open-decisions.md](docs/open-decisions.md).

1. **Presentation P0 · settled**: placement and kinds as
   authored positions, places known by an authored `known_when` condition,
   and the host boundary as in-memory loading, serializable commands, events,
   map and journal views, and JSON snapshots. Revisit only when the real
   client shows which queries it needs.
2. **Authored progression P0 · settled for the MVP**: phases as
   an ordered list moved by effects, `main` and `requires` on quests, and
   outcomes recorded once. Several languages are settled too: one base
   language plus complete overlays, and a rules-only save revision. Settle failure, deadlines and terminal outcomes
   when the original game's content beyond the MVP needs them.
3. **Politics P0:** settle M6b faction membership/war-peace representation and
   standing-track scopes/thresholds before authoring the game world's political entities,
   schools and orders.
4. **Party P1:** settle companion progression/gear, roster limit, provisions,
   morale and travel-speed rules required by the first full party of the original game.
5. **Living world P1:** settle only the holding, world-agent and faction-strategy
   details needed by the representative sandbox; defer ornamental systems
   such as courtship or champion duels until authored content demands them.
6. **Shared play P2:** only when the server milestone starts, settle the
   replay/import artifact, account binding, persistence schema and session/party
   ownership rules for the local-to-authoritative handoff.
