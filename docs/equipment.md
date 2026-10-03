# Equipment and crafting

Status: M4 equipment, forging/improvement and enchanting are delivered; this
document records the architectural boundaries behind the implementation. Balance
values remain world-authored. The design builds on selective runtime instances,
typed capability-owned modifiers, gradual defence, separate damage-type
modifiers, the armour/speed trade-off, and capped effective speed.

Equipment and crafting are separate capabilities. A world may contain weapons
and armour without allowing the player to forge, improve or enchant them. For a
source-derived world, include each crafting capability only when the source
material supports it. If forging or enchanting is not present or reasonably
implied by the source, omit its recipes, stations, progression and player actions.
RealmKit must not inject a generic crafting loop merely because the engine can
execute one.

## Inspiration and boundaries

Skyrim separates forging new equipment, improving weapons/armour at appropriate
stations, and applying learned enchantments. Materials and progression gate those
activities. See the smithing and enchanting sections of the
[official Skyrim manual](https://assets.ctfassets.net/rporu91m20dc/1poliwondakI2QwKQMY2QI/b79ecee4541dd1ba7c7a85b453f0e9b2/manual_skyrim-le_pc_en-us.pdf).

RealmKit should use that separation, with its own rules and world-specific
materials, crafts, names and lore. Soul gems, a particular perk tree, and Skyrim's
balance formulas are not engine requirements. Everything needed to craft is
already in the package; crafting never asks an AI to invent an item or its prose.

These are optional building blocks, not a required imitation of Skyrim. A world
can enable forging without enchanting, enchanting without player forging, both,
or neither. Improvement may exist only where the source supports craftspeople or
maintenance. Unique equipment can still be found, awarded or purchased when all
player crafting is absent.

## One definition, many individual items

| Concept | Purpose |
| --- | --- |
| Item definition | Authored identity, name, description and base equipment properties |
| Item instance | One owned piece of equipment, with a unique instance ID, improvement tier and enchantment |
| Material resource | Crafting input such as an ingot or leather strip; owned by Crafting when craft-specific, or referenced from Inventory when it is a general carried item |
| Recipe | Inputs, station, requirements and a specified output or transformation |
| Improvement tier | Bounded, authored modifiers applied to an item's base properties |
| Enchantment definition | Authored effect, allowed equipment and display text |

Two iron swords share a definition but have different instance IDs. Improving
one changes that instance only. Equipment owns each equipment instance's
ownership/possession state whether equipped or unequipped, so Equipment does not
require the general Inventory capability. Equipment slots, improvement commands
and saves refer to instance IDs.

Crafting-specific interchangeable materials may be stored as Crafting-owned
definition + quantity state when Inventory is absent. When Inventory is present,
recipes may instead consume Inventory-owned ordinary carried items through
explicit typed references; the same resource quantity is never owned by both
capabilities. Keep modified equipment individually addressable. Equipment
instances have since arrived as their own saved state while counted inventory
remains separate; neither capability owns the other's mutable state.

## Authoring equipment

Authors describe a weapon's category, compatible slots, physical/special attack
bonuses and the action cost of its basic attack. Armour describes its slots,
physical/special defence, speed penalty and any explicit damage-type modifiers.
Items may also grant bounded HP/MP or other supported stat bonuses.

Equipment-enabled worlds define the small set of slot IDs their content needs.
An equipment definition declares one or more occupied slots. A two-handed weapon,
for example, may occupy both main-hand and off-hand slots; validation prevents a
shield from occupying a conflicting slot. Do not hard-code a universal slot list
until multiple worlds prove that one is useful. Dual-wield attack rules can wait
for a dedicated design.

Materials are authored resources and recipe inputs rather than a universal
runtime progression hierarchy. Authors may define iron, steel and source-specific
rare materials with any trade-offs the world needs. Material/archetype helpers
may resolve properties during authoring, but export concrete item definitions so
runtime never re-derives or double-counts material bonuses. No automatic
all-materials × all-weapons catalogue is needed.

Conceptual typed authoring operations can grow from the existing WorldDraft:

```text
create_item(item_definition)
create_recipe(recipe)
create_enchantment(enchantment_definition)
validate_world()
```

These operations belong in worldgen. The resulting definitions belong in spec,
and the engine executes player actions against them. Future authoring CLI/MCP
tools wrap the same core.

Prefer presence-driven capabilities over a second set of booleans: no forging
recipes/stations means no forging system; no enchantment definitions/stations
means no enchanting system. Validation should diagnose half-defined capabilities,
such as a recipe requiring a missing station, while accepting their complete
absence. Presentation derives available menus from valid world content and state,
so an omitted capability leaves no empty panel or disabled global command.

## Three distinct player operations

The following operations exist only in worlds that author the corresponding
content. They are independent rather than an all-or-nothing crafting package.

| Operation | Input | Result |
| --- | --- | --- |
| Forge | Known recipe, materials, appropriate station and crafting ability | A new equipment instance |
| Improve | An eligible existing instance, improvement materials and requirements | The same instance at a higher authored tier |
| Enchant | An eligible instance, known effect, catalyst and requirements | The same instance with an allowed enchantment |

Proposed first rules:

- Recipes are explicit. A player selects a known recipe rather than combining
  arbitrary ingredients and receiving invented outcomes.
- Recipe knowledge and crafting proficiency are separate gates. Teachers, found
  plans, quests and authored discoveries grant recipes; smithing proficiency
  determines whether the character can use a known recipe. Raising proficiency
  alone does not automatically reveal recipes in the initial system.
- Proficiency is capability/world-specific. A first fixture may use one smithing
  proficiency and one enchanting proficiency, but RealmKit does not impose a
  universal scale or track. Recipes/tier transitions use authored thresholds;
  flags or learned recipes can provide special unlocks. No full perk tree is
  needed initially.
- Preview material costs and resulting properties only at the detail level
  permitted by the world/engine disclosure policy. Whatever representation is
  shown must derive from the same engine calculation used for the committed
  result; clients must not receive hidden exact values merely to render a
  qualitative/no-preview policy.
- Given valid inputs, the result is guaranteed. No random failure or hidden
  quality roll in the first system.
- Improvement selects an authored target tier, such as ordinary → fine → superior.
  It replaces the previous tier contribution; it does not repeatedly add a bonus.
  Require a strictly higher tier and its explicit material cost.
- Start with one enchantment per instance. Check effect compatibility with the
  item and reject enchanting an already enchanted item. Replacement, removal and
  learning by destroying items are separate later decisions.
- Keep refining and enchanting independent: improving an enchanted item preserves
  its enchantment unless an explicit recipe restriction forbids the operation.
- Start with simple passive stat enchantments. On-hit damage, charges, recharge,
  procs and multi-effect enchantments need their own combat rules when introduced.
- Crafting happens at an available station outside combat. Do not introduce
  real-time crafting timers or consume combat turns just to browse recipes.

Crafting ability can unlock better recipes and tiers without changing the same
recipe's output unpredictably. If crafting XP is added, award it only after a
successful state-changing operation. Rejected or repeated no-op upgrades give
neither progress nor rewards.

## Connection to combat

Compute an item's properties from its definition, current improvement tier and
enchantment. Then compute the character's effective stats from base progression
and currently equipped instances. Recalculate rather than permanently adding
bonuses on each equip operation.

- Weapon attack bonuses contribute to the character attack used by the damage
  formula. Do not also add those bonuses as a second damage packet.
- Weapon action cost controls recovery for the attack that uses it. A heavy weapon
  can hit harder and recover more slowly without also imposing an implicit speed
  penalty on unrelated actions.
- Equipped armour can raise defence while lowering speed, affecting action timing
  according to the agreed speed model. Merely carrying armour does not slow the
  character in this proposal; inventory weight is a separate future system.
- Resolve all speed modifiers before applying the agreed effective-speed cap.
- Damage-type immunity/vulnerability remains distinct from defence. The earlier
  physical-immunity/special-vulnerability armour can be an explicitly authored
  artifact; ordinary forging need not grant access to every exceptional property.

Before multiple resistance sources ship, settle stacking and immunity precedence.
No crafting route may silently change the damage formula or bypass the speed cap.

## Concrete first journey

Illustrative recipe and balance values, not finalized content:

```text
2 iron ingots + 1 leather strip
    → forge an Iron Sword
    → physical attack bonus +8, basic attack cost 10,000

Iron Sword + 1 iron ingot + sufficient smithing
    → Fine Iron Sword (same instance)
    → physical attack bonus +10 total, not +8 plus +10

Fine Iron Sword + an authored catalyst + learned focus enchantment
    → Fine Iron Sword of Focus (same instance)
    → retains +10 physical attack; adds +5 maximum MP while equipped
```

Pair that with leather and iron body armour, where the iron armour offers more
physical defence but a larger speed penalty. Before equipping, show only the
stat/action-timing preview permitted by the world/engine disclosure policy, derived
from the same calculation that will be committed. All names, descriptions,
station text and menu labels
come from the authored world, in the source language. If names are assembled,
use authored language-specific templates; never assume English suffix order.

## Validation and acceptance

Validate material/output references, positive quantities, station references,
requirements, slot compatibility, effect parameters, improvement transitions and
finite authored tier limits. Protect unique/quest artifacts through explicit
recipe eligibility; do not make every item craftable, destructible or enchantable.

Crafting commands must atomically check ownership, station access, knowledge,
proficiency, materials and instance eligibility before changing anything. Failure
consumes nothing. Success consumes inputs once and creates or updates the intended
instance once. Allocate instance IDs deterministically and preserve their sequence
across saves and replay. Repeated valid forge commands may create multiple items
only by consuming a new set of materials each time.

The first acceptance journey is forge → equip → improve → enchant → save/load.
It must prove that two copies stay independent, old improvements do not stack,
equipment bonuses do not duplicate, unavailable recipes explain their unmet
requirements, and invalid commands preserve resources. Arrow and numbered menus
invoke the same commands as the deterministic command interface.

## Delivery and open decisions

M4c: equipment definitions, individual instances, equip/unequip and comparisons.
M4d: materials, stations and forging/improvement recipes with one smithing track.
M4e: a learned enchantment, a catalyst and one compatible effect per instance.
(M4a and M4b are player-allocated stat points and technique ranks.)
Each step should produce a playable, tested loop before adding more content.
These milestones build reusable optional capabilities and a fixture world that
demonstrates them; they do not make crafting mandatory in every RealmKit world.

Recipe learning through teachers, found plans, quests and authored discoveries is
agreed. Each world chooses which source grants each recipe or enchantment.
Material costs, improvement strengths, proficiency thresholds, armour penalties
and speed-cap values are fixture/world balance decisions rather than architecture.
Defer durability/repair, arbitrary affix rolling, crafting-boost feedback loops,
disenchanting/replacement, dual wielding and an economy until their gameplay is
requested.
