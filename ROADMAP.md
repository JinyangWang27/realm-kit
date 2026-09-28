# RealmKit roadmap

Status: discussion draft. Milestones describe playable outcomes, not release
dates. M0, M1 and M2 are implemented; later milestones are proposed. Combat pacing and
balance formulas remain design decisions, not promises about current behavior.
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
  without making the main story irrelevant.
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
bound to package ID, content-digest revision and the `default` route; CLI
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

## M3 — Optional combat capability: stats and meaningful speed

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
| Special attack | Strength of special damage: magic, 内力, mana, as the world names it |
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

**Done when:** a small duel demonstrates distinct physical/special builds, MP
and rage expenditure and recovery, and a measurable benefit from increased speed. In M3,
builds are fixture stat blocks; player-chosen builds arrive with equipment in M4.
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

1. **M3a — combat becomes optional (Format 2).** Merge `npcs.json` and
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
   revision. The demo plays as before.
2. **M3b — stats, damage and skills.** The seven-stat block, gradual defence
   reduction by the damage formula confirmed in decision 3, the skills and MP
   costs listed under
   [Tuned values](#tuned-values), and a `Rest` command at authored safe locations
   that restores HP and MP outside encounters. Rage and MP regeneration during
   encounters need encounter time, so they arrive with M3c. Save only current HP/MP,
   level and XP; derive maximums and attack/defence values from the level so no
   bonus can be saved twice. Decide explicitly whether level-up still restores
   HP/MP fully, as it does today. Enemies keep counterattacking immediately.
3. **M3c — encounters on the timeline.** `Engage`, participants and sides,
   timeline scheduling, the projected turn order, and mid-encounter save/load, as
   described in [Combatants and encounters](#combatants-and-encounters--proposed).
   Grinding arrives here too: repeatable encounter groups and `Flee`, along with
   authored yielding. Fixtures cover a 1v1 duel, a 1v2 pack, a repeatable hunting
   ground, a fled fight and a sparring match that ends in a yield.
4. **M3d — seeded RNG and critical hits (optional).** A small hand-written,
   versioned PRNG such as SplitMix64 or PCG32 with saved state. Do not use
   `rand`'s `StdRng`: its output is not guaranteed stable across versions, which
   would break replay and saves.

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
  Opponents follow the same skill rules as players, so their level-gated skills
  matter; the balance simulation depends on this. Richer policies, such as
  targeting lowest HP, saving resources or random targeting through the RNG, are
  authored enum variants added with content that needs them, never scripts. Multiplayer adds more player controllers; each
  pending human turn pauses only its own encounter (M8).
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
- **Fleeing.** `Flee` is a player action with the normal action time: the player
  escapes when their next turn arrives, if still alive, so opponents act in
  between and a faster player escapes sooner. An authored group may forbid
  fleeing, as a boss or canonical duel might. Fleeing grants nothing and records
  no defeats.
- **Yielding.** A group may be authored to yield: whichever side falls to the
  authored share of maximum HP gives up instead of dying, as in 比武 that stops
  short (点到为止), a joust or a canonical duel. The encounter ends with its
  authored victory or defeat effects, such as flags, and nobody is recorded as
  defeated or dead. A yielding player keeps the remaining HP and play continues,
  so losing a sparring match is not M2 death recovery. Yielding also lets a
  canonical character protected by a canon anchor lose without being killed.
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

The absolute numbers are arbitrary. What matters is where they sit relative to
the defence scale K and the baseline speed, and how many meaningful steps lie
between starting and final values. Stats are not stored in 8 bits, so a cap of
255 would only be a convention borrowed from older games.

- **Engine bounds.** Validation rejects any authored or derived combat stat,
  including HP and MP, above 9,999; skill power outside 1–1,000; and, from M4, a
  damage-multiplier numerator above 10. Together they keep the damage numerator
  `A × P × A × M_num` below 4 × 10^16, far inside `u64`: the combined attack `A`
  is at most 1,999,800 even at a 100% cross share. The stat bound alone is not
  enough, because power is part of the product. Arithmetic stays checked anyway.
  These are safety bounds, not balance targets, and the stat bound also keeps
  displays narrow.
- **Speed cap.** A required world-level parameter because it governs scheduling.
  The starting candidate is 200 with baseline 100.
- **Other stat caps.** None in M3: the level table is authored, so authors already
  bound player stats. Add optional authored caps for other stats in M4 if
  equipment stacking needs them, applied after all modifiers as speed is.
- **Design scale.** Baseline speed is 100. A uniform stat cap of 100 would leave
  speed no room to rise. With 10% growth per level, a 200-HP character reaches the
  9,999 bound at about level 42, so a world with more levels needs a higher bound
  or slower growth.
- **Timeline resolution.** With action cost 10,000, speeds 100–200 produce only
  51 distinct delays; near the cap about four speed points share one delay. Action
  cost 100,000 gives every integer speed from 1 to 255 its own delay, so use
  100,000 as the starting value. The timeline examples below use 10,000 for
  readability.

### Balance simulation · proposed

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
channel. The targets suit a game where grinding for levels (打怪练级) matters:

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
- **In worlds with character levels, MP costs grow with the user's level at the
  same rate as the MP pool.** With flat costs, a uniformly growing pool lets
  casters cast more per rest at every level, and sustain drifts upward.
  Technique ranks (M4) author their own costs instead, so a deep 内力 pool from
  a high-rank internal art genuinely means more casts.
- **Rage never persists outside an encounter** and is not saved between fights.
  It is part of the saved encounter state during one.
- **Outside encounters,** MP refills only by resting in M3. Regeneration over
  World Time can follow when a world enables it (M5).
- Resources are authored per skill as one of a closed set. Do not build a
  general resource framework before a world needs a third resource.

#### Tuned values

Level-1 values that meet every target, with every stat growing 10% per level for
players and monsters alike:

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
| Bolt / fireball / starfall | Mage | Special | 1 / 10 / 20 | 170 / 210 / 250 | 12 MP at level 1 |
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

`tune` nudges every tuned value, player stats and tier multipliers included, and
68 of 114 nudges keep every target. It reports the first target each nudge
breaks. Half of the 46 breaks are boss parity against beasts, each a two-level
gap where one is allowed; allowing two levels (`BOSS_LEVEL_GAP` in `targets.py`)
is a design choice, not a tuning fix. Not every break is a near miss, though:
lowering the warrior's physical attack from 24 to 20 or its HP from 200 to 170,
or raising the mage's HP to 195, leaves the warrior winning only 1 to 4 of 15 or
16 comparisons, so the trade-off collapses toward the mage. Those three values
need the tightest control when balancing resumes. The remaining breaks are
fights that become too short, later skills that stop mattering enough, or too
many fights between rests.

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
- **Grinding needs XP that falls off with level difference.** With ±10% XP per
  level of difference and nothing from monsters five or more levels below,
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
nonnegative defence/multiplier numerators, a positive multiplier denominator, a
share from 0 to 100, and valid positive attack/power for this damaging-hit
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

## M4 — Equipment, skills and character builds

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
  [technique ranks](#technique-ranks--proposed). Introduce cooldowns/status
  effects only alongside skills that require them.
- Add immunity/vulnerability multipliers, specify modifier stacking and immunity precedence, choose armour speed
  penalties, and add optional caps for stats other than speed if stacking needs
  them.
- Define durations in terms of the chosen combat clock; do not casually mix
  wall-clock seconds, actor turns and timeline units.
- Show why stats changed and what an action costs before confirming it.
- Deliver in slices: M4a equipment instances and equip/unequip; M4b stations,
  deterministic forging and authored improvement-state transitions; M4c one
  compatible learned enchantment per item. Runtime instances use deterministic
  saved IDs only when distinguishable copies need independent state; unique
  legendary equipment may still have one mutable instance, while fungible
  identical resources remain definition + quantity. Materials/quality are concrete
  authored data rather than a universal runtime hierarchy. Preserve authored
  source-language names and prose throughout crafting.
- Keep recipe knowledge separate from proficiency: authored teachers, plans,
  quests or discoveries grant recipes, while smithing determines whether a known
  recipe can be used. Proficiency alone does not reveal recipes initially.

### Technique ranks · proposed

Wuxia sources measure power by how deeply a technique is mastered: 龙象般若功 has
ten layers, 九阴真经 is learned layer by layer, and 郭靖 learns 降龙十八掌 one
move at a time. Instead of a separate realm-tier system, each learned technique
has a rank.

- **Rank tables.** Each technique authors one entry per rank: power, cost, action
  time, and optionally passive stat grants and a changed weapon requirement.
  Internal arts (内功/心法) are passive techniques whose ranks grant stats such as
  maximum MP or special attack. A sword art's top rank may drop its weapon
  requirement, as 独孤求败 moves from 木剑 to 无剑. Each rank authors its own costs.
- **Rising through use.** Using a technique in an encounter earns technique XP,
  with the same falloff by level difference as character XP, so practising on
  weak opponents stops paying. Nothing is earned outside encounters.
- **Rising through teaching.** Authored effects from masters, manuals and
  奇遇 grant a technique or set its rank directly.
- **Breakthrough gates.** Reaching a rank may require authored conditions such as
  a flag, quest state or story phase: 九阴真经's later layers need the second
  volume. Technique XP stops at the gated threshold until the gate opens, so the
  per-chapter level cap becomes a per-technique cap.
- **Realms.** A world may name one technique as the character's core internal
  art. Its rank is then displayed as the realm (境界) under the world's own names,
  and conditions can test it. No separate tier system exists.
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
  retaining Format 1 placeholders. An inventory-free world has no inventory state,
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

**Done when:** longer hand-authored fixtures demonstrate branching progression
and tested paths to completion, can be saved/resumed, and include at least one
inventory-free non-combat fixture that loads and plays with no inventory state,
definitions/placeholders, Inventory command or inventory UI. A dungeon is useful
only for a world that needs one.

## M6 — Authoring feedback and deterministic simulation

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

## M7 — Source-grounded world generation

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

## M8 — Additional clients and shared play · optional later

Add a richer TUI or web/mobile client over the same command model when useful.
Introduce an authoritative server and party play as a separate milestone once
the single-player rules are stable. The server owns canonical state, RNG
resolution, authoritative command ordering and durable persistence; clients submit
commands rather than outcomes.

Serialize commands that touch the same mutable scope, while allowing independent
world/player/session/encounter scopes to progress concurrently. A pending human
turn pauses only its relevant encounter/session scope, not the whole server.
Timeouts/disconnect handling may submit predefined fallback commands but wall-clock
waiting does not itself advance gameplay time. Reconnect from current server
state; never rewind a persistent shared world from a client snapshot.

Single-player checkpoint rewind does not imply rewinding a persistent shared
world—multiplayer recovery remains scoped to state exclusively owned by the
relevant player/session/instance. Add storage/networking crates only when this
milestone becomes active work.

**Done when:** clients cannot bypass engine rules, and the chosen multiplayer
scheduling and persistence policies have reproducible tests. No runtime AI.

## Decisions to discuss next

This is the immediate combat-oriented subset. The complete cross-project list is
maintained in the [open-decisions register](docs/open-decisions.md).

1. Confirm the [encounter model](#combatants-and-encounters--proposed): `Engage`
   on any character with a combat profile, authored groups, sides, player or
   policy control, tie order and vitals ownership. Confirm that very fast actors
   can take several consecutive turns as in the timeline example.
2. Confirm speed cap 200 at baseline 100 and action cost 100,000. Armour speed
   penalties move to M4 with equipment.
3. Confirm the [stat ranges](#stat-ranges-and-caps--proposed): the 9,999 engine
   bound and no other stat caps in M3. Confirm the
   [balance targets](#balance-simulation--proposed), the K-free damage formula
   in place of level-scaled K (then retune the simulator), and XP that falls off
   with level difference. Confirm the
   [optional parts](#optional-parts-of-combat--proposed) and that power comes from
   stats, never level. Confirm repeatable groups and `Flee` in M3c for
   grinding.
4. Confirm the [skill resources](#skill-resources): MP regenerating over
   encounter time and by resting, rage built from actions and damage taken, and
   MP costs that grow with level.
5. Confirm the two channels, physical and special, with a world-named special
   channel and a 25% cross share. Immunity, vulnerability and modifier stacking
   are deferred to M4.
6. The M3 combat menu shows exact damage after each action and the projected turn
   order. Qualitative previews wait for a client that needs them.
7. For M4, confirm [technique ranks](#technique-ranks--proposed) in place of
   realm tiers, and extend the balance simulation to ranks before choosing their
   numbers.

Recommended next step: start M3a, which needs none of these decisions. Settle 2
and 3 before M3b, and 1 and 4 before M3c.
