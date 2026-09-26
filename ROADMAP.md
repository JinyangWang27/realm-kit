# RealmKit roadmap

Status: discussion draft. Milestones describe playable outcomes, not release
dates. M0 and M1 are implemented; later milestones are proposed. Combat pacing and
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
  PlayerSpec-like player definition, start state/location, exactly one main
  questline and one or more authored outcomes. Side questlines are optional, but
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

## M2 — Continue an adventure across sessions

Treat saves as storage-neutral `SaveSnapshot` data. The engine serializes and
validates deterministic state; CLI/mobile/server layers choose files, SQLite,
databases or other durable storage.

- Save/load the playthrough state for the selected player route plus only the
  capabilities enabled by that world—for example investigation evidence,
  inventory, quests, combat stats or defeated enemies. Keep authored definitions separate from mutable saves; do
  not require empty quest/inventory state in worlds that omit those capabilities.
- Version saves and identify the world package they belong to. Reject incompatible
  saves clearly; add migrations when an actual format change requires them.
- Write saves atomically and keep the previous save safe if writing fails.
- Support manual saves and deterministic auto-saves in one chronological recovery
  history. Auto-save at route start and major stable authored progression
  boundaries such as story-phase transitions; allow additional authored/capability
  checkpoints where useful, but do not auto-save on every ordinary movement step
  or presentation command.
- Ordinary recoverable death restores the newest valid manual/auto-save snapshot.
  Restoring replaces the current playthrough state with the saved deterministic
  snapshot; it does not "undo" commands piecemeal. Explicit authored terminal
  death/failure outcomes bypass this recovery behavior.
- Save creation consumes no story/world/encounter time and must not perturb
  narrative variant selection.
- Preserve every deterministic counter; later combat scheduling must also survive
  save/load. Treat loaded saves as input that needs validation.

**Done when:** saving mid-quest, quitting and resuming produces the same subsequent
events as uninterrupted play; ordinary death restores the newest valid recovery
snapshot; story-phase auto-saves are reproducible; and a broken or mismatched save
cannot corrupt a world or silently reset progress.

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
| Magical attack | Strength of magical damage |
| Magical defence | Mitigation of magical damage |
| Speed | Action timing; exact model discussed below |

Use these direct combat stats first. Strength, intelligence, dexterity, elemental
resistance, accuracy and critical chance can come later when builds need them.

- Apply the same stat and damage rules to players and monsters.
- Add a basic physical attack and one MP-consuming magical skill, sufficient to
  test both damage channels. Define safe-location HP/MP recovery explicitly.
- Select physical attack/defence for physical skills and magical attack/defence
  for magical skills. Keep skill power separate from the character's attack stat.
- Centralize damage calculation in the engine; content supplies bounded numeric
  parameters, never executable formula strings.
- Specify rounding, minimum damage, resource costs, death and action cancellation.
  Use checked integer arithmetic and reject invalid stats or parameters.
- Keep the first combat slice simple, but support stochastic combat mechanics such
  as critical hits through RealmKit's explicit seeded RNG facility. Basic attacks
  may remain certain initially; randomness is an authored rule rather than a
  requirement for every combat action.
- When stochastic mechanics are implemented, use explicit/versioned PRNG behavior
  with saved RNG state. Keep semantically unrelated random domains independent
  where incidental draw coupling would produce surprising gameplay changes.

**Done when:** a small duel demonstrates distinct physical/magical builds, MP
expenditure and recovery, and a measurable benefit from increased speed. Tests
cover formulas, scheduling, ties, death, resource rejection, replay and save/load.
A combat-free fixture also validates and plays without combat data or menus.

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

Before implementation, settle deterministic tie-breaking, when MP is consumed,
and how later speed changes affect already-scheduled turns. Dead actors must lose
pending actions, rejected commands must not advance time, and save/load must
preserve the schedule exactly. The initial version need not include speed buffs.

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

### Damage: gradual defence reduction · direction agreed

Use gradual defence scaling rather than subtracting defence directly from attack.
The exact scale, starter stats and balance targets remain proposals. Explicit
immunity and vulnerability are a separate layer from ordinary defence.

Proposed calculation for a positive-power damaging hit that has landed:

```text
A = physical or magical attack, matching the skill
D = matching defence
P = skill power as an integer percentage; basic attack = 100
K = positive world-level defence scale; example = 100
M_num / M_den = resolved incoming-damage multiplier for this damage type
               normal = 1/1; immunity = 0/1; vulnerability = 2/1

if M_num == 0:
    damage = 0
else:
    damage = max(1, floor(A × P × K × M_num
                         / (100 × (K + D) × M_den)))
```

Require nonnegative defence/multiplier numerators, a positive multiplier
denominator, and valid positive attack/power for this damaging-hit calculation.
Round once, at the end, with checked wide intermediate arithmetic. Cap actual HP
loss at remaining HP. Healing, non-damaging skills and any future misses use
separate rules. **Immunity overrides minimum damage:** an immune target takes zero,
never the fallback one point.

With attack 40, power 100%, K = 100 and a normal 1× multiplier:

| Defence | Damage |
| --- | --- |
| 0 | 40 |
| 20 | 33 |
| 100 | 20 |
| 300 | 10 |

At defence K, incoming damage is approximately halved. K should be an explicit
balance parameter, not a hidden constant or a scripting language. Select it using
expected stat ranges and target battle lengths; these numbers are examples.

#### Exceptional armour: immunity and vulnerability

An armour can combine ordinary defence with explicit damage-type modifiers:

| Incoming type | Multiplier | Meaning |
| --- | --- | --- |
| Physical | 0× | Completely immune to physical damage |
| "Special" | 2× | Twice the damage that would otherwise pass through its matching defence |
| Other | 1× | Ordinary defence reduction |

For example, a special hit with an exact post-defence value of 40 deals 80 under
the vulnerability. A physical hit deals zero regardless of its attack value.
Multiply before the single final rounding step, rather than rounding the
post-defence value first.

"Special" is a placeholder, not an agreed damage category. It might mean magical
damage or a separate spiritual/other channel. Decide which defence it uses before
implementing it. **Double damage and defence bypass are distinct properties**:
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
- Give skills authored descriptions, MP costs, damage channels and power.
  Introduce cooldowns/status effects only alongside skills that require them.
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

**Done when:** at least two meaningfully different builds can finish a short
adventure, with tested equipment/resource rules and readable combat feedback.
The forge → equip → improve → enchant → save/load journey preserves individual
item identity and consumes resources atomically without duplicating bonuses.

## M5 — Longer authored adventures and source-specific mechanics

- Separate monster definitions from encounter instances; support multiple enemies
  without reusing one global HP record. Use instances only where distinguishable
  copies require independent mutable state; do not instance every authored entity.
- Generalize the core quest model into main and side questlines with typed
  multi-target objectives, prerequisites and chains. Side questlines are unlocked
  by explicit main-story/story-phase conditions and may feed explicit state back
  into later main quests.
- Define encounter reset/respawn and retreat rules before relying on repeatable
  combat. Preserve quest progress and prevent duplicate completion rewards.
- Expand authored dialogue and branches, with consistent NPC availability and
  understandable journal entries.
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
  elapsed time, appointments or deadlines. Represent it as monotonic minutes from
  an authored epoch, advance it only explicitly, and resolve crossed scheduled
  events chronologically with stable authored order for ties. Do not assign
  universal durations to ordinary commands.
- Support authored stochastic world-event opportunities when a representative
  world needs rare encounters/discoveries. Trigger rolls only at explicit gameplay
  transitions and select only among pre-authored outcomes.

**Done when:** longer hand-authored fixtures demonstrate branching progression
and at least one non-combat interaction path, can be saved/resumed, and have
tested paths to completion. A dungeon is useful only for a world that needs one.

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
- Derive the main questline from the source's canonical story and preserve
  characterization and atmosphere. Generate side-story seeds from canonical NPCs,
  locations, factions, relationships, occupations, conflicts and unresolved
  details; expand those seeds into side questlines before inventing unrelated
  generic content.
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
For a two-route fixture, both routes are selectable from New Game, share the
intended world/canon data, and validate independently from their own starts.

## M8 — Additional clients and shared play · optional later

Add a richer TUI or web/mobile client over the same command model when useful.
Introduce an authoritative server and party play as a separate milestone once
the single-player rules are stable. The server owns canonical state, RNG
resolution and durable persistence; clients submit commands rather than outcomes.
Single-player checkpoint rewind does not imply rewinding a persistent shared
world—multiplayer recovery is scoped to player/session/instance state. Shared combat needs an explicit policy for
waiting on several players; a paused single-player timeline does not answer that
question automatically. Add storage/networking crates only at that point.

**Done when:** clients cannot bypass engine rules, and the chosen multiplayer
scheduling and persistence policies have reproducible tests. No runtime AI.

## Decisions to discuss next

This is the immediate combat-oriented subset. The complete cross-project list is
maintained in the [open-decisions register](docs/open-decisions.md).

1. Confirm the paused timeline, including whether very fast actors can take
   several consecutive turns as in the example above. Choose the speed cap and
   armour penalties; the need for both a cap and the armour trade-off is agreed.
2. With gradual defence selected, establish starter stat ranges, the defence
   scale and a target number of actions for an ordinary fight.
3. Define "special" damage and its matching defence, then specify how equipment
   modifiers stack.
4. Decide how much to show in the combat menu: exact damage/turn previews, or
   simpler qualitative descriptions backed by an optional detailed log.

Recommended next implementation: M2 save/load, while these combat choices are
discussed.
