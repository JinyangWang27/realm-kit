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
| Party and companions | Membership, role, morale, approval, troop rosters, wages, companion gear, prisoners, wounded, provisions | Recruit, assign, counsel, dismiss, upgrade troops, equip companions, ransom |
| Romance | Affection, boundaries, commitments | Court, respond, commit, separate |
| Mass battle | Army rosters, strength, casualties | Give battle, retreat, besiege |
| Holdings | Ownership, income, garrisons, buildings | Garrison, build, collect, besiege |
| World agents | Roaming party instances, goals, spawners | None directly; they act on world ticks and can intercept the player |
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

A non-spatial academic-career simulation might use:

```text
time + economy/funding + relationships + reputation + scholarship + authored events
```

It should not acquire combat, inventory or fake spatial traversal merely because
the first RealmKit fixture uses them. Repeated domain rules may justify one small
typed institution/career capability later; isolated cases should remain authored
events. See [non-spatial simulation worlds](simulation-worlds.md).

A living sandbox of rival kingdoms might use:

```text
travel + time + economy + factions/standing + relationships + party (retinue)
+ mass battle + holdings + world agents + combat + equipment
```

Its main questline is a ladder of ambitions whose top rung is one completed,
non-terminal outcome, while lords, bandits and caravans act on world ticks.
Politics and knightly orders compose factions, standing and holdings rather
than adding capabilities of their own. Each of these is optional on its own, so
smaller worlds take only the pieces they need; see
[modularity](sandbox-worlds.md#modularity) for the dependencies between them.

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

## Combat across sources

One combat capability should serve very different sources through authored
names and data. These four worked examples show where the shared model fits and
which typed mechanics each source would add.

| | 神雕侠侣 | 天龙八部 | Arthurian legend | 雪中悍刀行 |
| --- | --- | --- | --- | --- |
| Damage channels | 外功 physical, 内功 special | Same | Arms physical, enchantment special | Arms physical, 气机 and 剑气 special |
| Skill resource | 内力 as MP, restored by 打坐 as rest | 内力, which can be absorbed or dissolved | None for knights; mana for Merlin and Morgan | 气机 as MP; a rage-style 刀意 suits 徐凤年 |
| Growth | Techniques from 秘籍 and 奇遇 | Techniques and absorbed 内力 | Deeds and renown | Realms from 三品 to 陆地神仙 |
| Signature mechanics | 点穴 stun, poison, 双剑合璧 combo, mood-gated 黯然销魂掌 | 斗转星移 reflection, 内力 attacks, unreliable 六脉神剑, 生死符 | Excalibur's scabbard, Gawain's strength until noon, the Green Knight | 金刚 toughness, 指玄 insight, summoned swords |
| How fights end | Often sparring that stops short (点到为止) | Duels and one-against-many sieges | Jousts and duels of honour | Duels and armies |

### Weapons

Weapons are equipment (M4) with world-authored categories rather than a
universal list. A small closed set of typed properties covers these sources. Special attack
and defence carry the world's own name, such as 内力 in wuxia:

- a category that techniques can require, so a 剑法 needs a 剑;
- stat modifiers, including a speed penalty for heavy weapons;
- basic-attack power and action time, so a dagger is quick and light and a
  heavy sword slow and powerful;
- optional defence bypass or a damage channel;
- the authored slots it occupies, so a one-armed 杨过 has one hand slot.

独孤求败's sword tomb (剑冢) is a ready-made progression: a sharp sword with normal
power and time, a faster soft sword, 玄铁重剑 ("重剑无锋，大巧不工") as a slow,
heavy strike that bypasses some defence, then 木剑 and 无剑, where techniques
stop requiring a weapon at all.

Weapons with equal damage per unit of time still change fights at hits-to-kill
breakpoints, so each weapon tier needs the balance simulation's checks, not only
matching damage rates.

### Mechanics by demand

Each mechanic is a typed engine rule with authored parameters, added only with a
world that needs it:

1. **Yielding** (all four): a fight ends at an authored HP share without death.
   Delivered in M3c; also protects canon anchors.
2. **Technique ranks** (the wuxia sources): ranked techniques with breakthrough
   gates and an authored name for every rank, where a realm is the named rank
   of a core internal art. Decided for M4.
3. **Status effects** (three sources): stun from 点穴, poison, enchanted sleep,
   with durations in encounter time. Planned for M4.
4. **Defence bypass and immunity** (雪中悍刀行, Arthurian legend): planned for M4.
   Illusion or mind attacks, such as Morgan's, become special hits with an
   explicit bypass or immunity rule rather than a third channel.
5. **Resource attacks** (天龙八部): skills that absorb or dissolve MP.
6. **Conditional techniques:** Gawain's noon strength needs World Time,
   黯然销魂掌 depends on a story flag, and 六脉神剑's unreliability uses the
   seeded RNG or an authored condition.
7. **One source each:** reflecting attacks (斗转星移), ally combo techniques
   (双剑合璧, which needs allies), mounted charges, and techniques that injure
   their user.

Mass battles, such as 北凉铁骑 or Camlann, are not personal combat. They remain
authored story outcomes or use the separate mass-battle capability proposed for
[living sandbox worlds](sandbox-worlds.md#mass-battle). A siege like 聚贤庄 fits
personal combat as one against many minion-tier opponents.

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
