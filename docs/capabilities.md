# Optional world capabilities

RealmKit's universal core separates a shared world from one or more player
routes. The world owns the shared location graph, characters/NPCs, factions,
narrative content, story-phase definitions, conditions/effects and world state.
Each player route owns its PlayerSpec-like player-control binding to a shared
Character, starting state, exactly one main questline and one or more authored
outcomes. Side questlines are
optional content. Inventory, combat, investigation, equipment and similar
mechanics are optional capabilities used by quests when justified by the source.

This document catalogs those optional capabilities. A capability may be absent
without placeholder data, but every player route still has a main questline as
its story-progression spine.

## When something deserves a capability

A recurring source element may deserve a capability when it has all three:

1. State that changes during play.
2. Rules that apply in more than one authored scene.
3. Choices whose consequences the engine must calculate or enforce.

A single locked door can use a condition and flag. Repeated locks involving
tools, skill, risk and consequences may justify lockpicking. A single courtroom
scene can be a dialogue tree; recurring claims, evidence and credibility may
justify a courtroom capability. Prefer authored events until reusable rules add
real expressive value.

Capabilities are presence-driven. A complete absence is valid and creates no
state, validation requirements, screens or commands. A present capability must
be internally complete and machine-valid. Do not add placeholder data to make a
world appear compatible with systems it does not use. Quest progression itself is
core; capability-specific objective kinds are available only when their
capability is present.

## Capability catalog

| Capability | State it may own | Typical deterministic actions |
| --- | --- | --- |
| Investigation | Clues, evidence, hypotheses, contradictions | Examine, interview, connect, deduce, accuse |
| Relationships | Trust, fear, affection, loyalty, commitments | Confide, persuade, threaten, reconcile |
| Reputation and factions | Standing, rank, alliances, hostility | Join, negotiate, endorse, betray, request aid |
| Stealth and infiltration | Awareness, suspicion, concealment, access | Hide, distract, trespass, pick a lock |
| Pursuit and escape | Relative distance, route, obstacles, stamina | Chase, flee, intercept, take a shortcut |
| Survival | Hunger, fatigue, temperature, injury, supplies | Eat, rest, shelter, forage, treat injury |
| Travel and time | Calendar, journey progress, schedules, deadlines | Travel, wait, camp, choose a route |
| Economy and trade | Currency, prices, merchant stock, debts | Buy, sell, bargain, borrow, repay |
| Inventory | Carried fungible/non-equipment items and their quantities/possession | Take, drop, give, transfer |
| Equipment | Equipment instances, their ownership/possession, equipped slots and effective modifiers | Acquire, equip, compare, unequip, transfer |
| Crafting | Recipes, crafting-resource quantities, proficiency, stations | Forge, improve, enchant, brew |
| Combat | HP/MP, stats, initiative, effects, opponents | Attack, use skill, defend, flee |
| Magic and rituals | Known rites, costs, preparation, curses | Cast, prepare, dispel, perform a ritual |
| Puzzles and mechanisms | Mechanism state, known facts, attempts | Inspect, manipulate, combine, answer |
| Debate and courtroom | Claims, admitted evidence, credibility | Present, challenge, defend, rule |
| Politics and intrigue | Influence, offices, secrets, alliances | Scheme, expose, support, blackmail |
| Party and companions | Membership, role, morale, approval | Recruit, assign, counsel, dismiss |
| Romance | Affection, boundaries, commitments | Court, respond, commit, separate |
| Leadership and settlement | Population, safety, resources, assignments | Assign, build, ration, defend |
| Business or estate | Staff, stock, income, upkeep | Hire, invest, price, expand |
| Collection and scholarship | Texts, artifacts, translations, knowledge | Study, translate, catalogue, compare |
| Performance | Repertoire, audience mood, renown | Perform, rehearse, improvise, compete |
| Transformation | Form, identity, corruption, disguise | Transform, resist, conceal, revert |
| Time loop | Cycle, reset state, retained knowledge | Reset, exploit knowledge, prevent an event |

Equipment does not require the general Inventory capability merely to keep an
unequipped weapon or armour piece. Equipment owns the identity and
ownership/possession state of equipment instances, including unequipped ones.
Inventory owns general carried/fungible item state.

Crafting likewise does not require Inventory merely to model material-consuming
recipes. If Inventory is absent, Crafting may own the quantities of
crafting-specific resources it consumes. If Inventory is present and a recipe
uses ordinary carried items, those inputs remain Inventory-owned and Crafting
references/consumes them explicitly. A given resource has one authoritative state
owner; capabilities must not duplicate the same quantity/ownership state.

When capabilities share presentation, a client may show one unified carried-items
or resources view without merging their authoritative state models.

Names in this table describe design areas, not compulsory Rust modules. Closely
related capabilities can share domain types where that makes the actual code
smaller. Do not create one crate per row or a generic plugin registry.

## Example compositions

A detective adaptation might use:

```text
exploration + dialogue + investigation + relationships + time + accusation
```

It needs no combat, equipment, crafting or inventory unless the source calls
for them. It still has a main questline: its objectives may be to interview,
discover evidence, connect facts, make deductions and accuse rather than fight.
The investigation capability records which evidence the player has legitimately
obtained; authored dialogue and accusation branches inspect that state.

Evidence is not an inventory subtype. An evidence definition may optionally refer
to a physical entity—such as a letter, weapon, receipt or photograph—that can
also be represented as an inventory item. The relation should be typed and
validated, for example conceptually `EvidenceDefinition { subject: Option<EntityRef> }`,
rather than a free-form `"evidence"` tag on arbitrary items. Testimony,
observations and deductions can be evidence without corresponding inventory
objects. Possessing an item and recognizing/obtaining its evidentiary significance
are separate state transitions.

A court-intrigue adaptation might use:

```text
dialogue + factions + reputation + secrets + debate + political consequences
```

A wilderness story might use:

```text
travel + time + survival + weather + injury
```

Crafting belongs only if the source supports characters making or improving
things. Combat belongs only if physical or magical conflict is an important
playable part of the source.

Capabilities compose through explicit shared concepts such as entity IDs,
locations, conditions, flags, commands and events, plus typed cross-capability
references when needed. Optional inventory state can reference the same physical
entity that an investigation evidence definition references without making either
capability depend on the other. For example, examining a carried letter may mark
an evidence definition as obtained; testimony may do the same with no inventory
object. Cross-capability effects must be declared and validated rather than
inferred from generic tags or hidden global state.

## World-generation selection

Select capabilities after canon extraction and before detailed gameplay
generation.

A fictional concept does not automatically imply a mechanic. Require recurring
mutable state, reusable rules and meaningful player-facing consequences before
promoting something into a capability. A village blacksmith alone does not
justify crafting; repeated player forging/material/proficiency decisions may.

Worldgen should propose the smallest useful capability set and explain it with
short rationale plus source references. Existing provenance labels
(`SOURCE / INFERRED / EXPANDED`) are sufficient; do not add numeric confidence
scores.

The author/user may accept the proposal or explicitly include/exclude capabilities.
Automation need not stop for human approval, but overrides should be retained in
the authoring report rather than presented as source-derived evidence.

Finalize the capability set before generating detailed quests, prose and mechanic
content. The resulting authoring report may also explain meaningful omissions,
especially unsupported genre conventions such as combat, crafting, classes or an
economy.

Capabilities are package/world-level mechanics. Individual player routes may use
different subsets of those mechanics without duplicating their definitions.

Generated player-facing text for every enabled capability follows the
source-language rule.

## Validation and runtime expectations

Each capability defines:

- The authored definitions it requires.
- Its mutable state, if any.
- Structured commands and events.
- Validation rules and structured diagnostics.
- Save/load representation.
- Deterministic simulation boundaries.
- Presentation data needed to label its actions in the world's language.

Validation accepts complete absence. When present, it detects missing references,
unreachable required outcomes and incomplete definitions as far as that
capability's analyzer supports. Cross-capability conditions/effects/references are
explicit and invalid when they require an absent or incompatible capability.
Clients build their action menus from the world and current state, so absent
capabilities remain invisible.

Core state should contain only data truly shared by all worlds. Capability state
uses explicit optional typed composition. This does not require runtime-loaded
plugins, scripting, an ECS, one crate per capability, or AI during play. Add the
smallest typed implementation when a representative world needs it.

Player-entry route selection sits above capabilities. A package may contain a
canonical route, an original-character route, or both. When both are present,
New Game offers the choice immediately; neither route must unlock the other.

Routes reuse the same validated world definitions and canonical timeline where
possible, but have their own PlayerSpec-like control binding, starting state, main
questline and mutable save. The binding selects which shared Character is
player-controlled for that route. In the original route, canonical protagonists
remain ordinary world entities/NPCs from the player's perspective and continue through
their protected canonical anchors. Route selection changes who the player controls,
not which source characters exist.

## Suggested implementation order

The current combat demo proves the command/event and static-content seams. The
next non-combat fixture should exercise a main investigation questline while
omitting combat and inventory. A second slice can add a physical evidence object
to prove that inventory and evidence compose through typed references rather than
inheritance or tags. That fixture should also demonstrate that a side questline
can be unlocked by main-story phase and feed explicit state back into a later
main quest. Relationships/reputation and time are then broadly useful. Skill
checks should be introduced only with clear semantics for certainty, difficulty
and any deterministic randomness.

Every new capability should ship with one small playable fixture proving why it
exists. A capability without a representative world is speculative scaffolding.
