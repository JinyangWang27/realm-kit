# Living sandbox worlds

Status: design direction. The rules below are proposals. This document does not
change the current package format or the active milestone. The roadmap schedules
the work as [M5](../ROADMAP.md#m5--longer-authored-adventures-and-source-specific-mechanics)
foundations and [M6](../ROADMAP.md#m6--living-sandbox-worlds--proposed)
capabilities, and [Section 18](open-decisions.md#18-living-sandbox-worlds) of the
open-decisions register tracks the questions this document leaves open.

RealmKit's fixtures so far are authored adventures: the player moves through
places, talks, fights and follows a questline. A *living sandbox* is a different
kind of world. The story is mostly what the player makes of it, and the world
keeps moving whether or not the player is watching. Like the
[non-spatial simulation note](simulation-worlds.md), this document is an
architecture test. It asks what RealmKit must add so that a sandbox is one more
world composition, not a separate engine.

## Reference scenario: a realm of rival kingdoms

The player starts with a horse, a sword and a few coins. They travel an overland
map of towns, castles and villages. They buy goods cheaply in one market and
sell them dearly in another. They hire village recruits and train them into
veterans, take jobs from guild masters and lords, and hunt the bandits that prey
on caravans. They earn renown, swear fealty to a king and receive a village as
a fief. Later they may join a knightly order, build its chapter house, marry
into a noble house or crown themselves.

Meanwhile lords ride out with their own war parties. Kingdoms declare war and
make peace. Castles fall to sieges that the player never sees, and invaders raid
the borders. A later visit finds a new banner over a town the player once knew.

No part of this needs a concrete source game. The scenario only pushes
RealmKit's abstractions and tests.

## Architecture test

A mature RealmKit should play this scenario with the guarantees every world
keeps:

- The runtime is deterministic and offline. The same package, seed and commands
  produce the same events and state, including everything the world did
  without the player.
- Save and load capture every piece of state needed to continue exactly, such as
  roaming parties, prices and pending schedules.
- Randomness comes from explicit, saved RNG streams and selects only among
  authored possibilities.
- Every capability below, and every part inside one, is optional. A world that
  uses none of them carries no state, commands or screens for them. See
  [Modularity](#modularity).
- Content supplies bounded numbers and typed rules, never formula strings or
  scripts.

## Modularity

The sandbox is not one feature. It is a set of small capabilities that a world
picks from, and the full realm of rival kingdoms is just the world that picks
all of them. A trading game might use only travel, world time and the economy;
a mercenary story might add a retinue and mass battles but no holdings; a
court drama might use factions, personalities and marriage with no fighting at
all. Four rules keep that true:

1. **Presence decides.** A capability exists only when the world authors its
   block. Absent, it has no state, commands, menus, events or save fields.
2. **Few hard dependencies.** A capability requires another only when it cannot
   mean anything without it, as listed below. Validation rejects a world that
   authors a capability without its prerequisites, with a stable diagnostic
   code.
3. **Soft interactions only when both are present.** Where two capabilities
   touch, for example morale and provisions, the interaction happens only if
   both exist. With one missing, that input or effect simply does not occur;
   nothing is faked with neutral placeholder data.
4. **Parts inside a capability are optional too.** Each part left out has a
   defined behaviour, listed below, so a world can take a capability's core and
   skip its extras.

The M5 condition tree and effect lists are package syntax rather than a
capability: a world that writes one condition or one effect behaves exactly as
it does today. Dialogue roles and text variants are syntax too: a world without
roles gives each character its own dialogue, and a plain string is a text with
one variant. Every other foundation (world time, recurring schedules, roads,
quest deadlines, offers, runtime instances) is optional.

| Capability | Hard requirements | Optional parts, and what happens without them |
| --- | --- | --- |
| World time | none | Roads without durations take no time; without wait and rest, only travel passes time. |
| Recurring schedules | world time | None; everything periodic below needs them. |
| Offers | none | Without world time, offers have no deadlines; without schedules, they refresh only on arrival. |
| Economy | none | Without schedules, prices move only through the player's own trade, and prosperity, restocks and workshop income do not exist; without producers, prices only revert to base; without world agents, trade links converge on the price tick and no caravans run; without a trading proficiency, the spread is fixed. Workshops are a separate part. |
| Factions and standing | none | Diplomacy and standing tracks are separate parts; a world may author either. |
| Personalities | none | Traits alone can gate dialogue. Reactions need standing; companion friction needs standing and a retinue, and its morale effect needs morale; agent behaviour needs world agents. |
| Retinue | none | Wages, provisions, wounded recovery, morale drift and desertion run on the retinue's upkeep tick, so they need recurring schedules; wages and provisions also need the economy. Morale, wounded troops, upgrades and travel speed (which needs world time) are each separate parts. Without them nobody is paid or eats, morale is not tracked, every loss is killed, troops never upgrade, and roads take their authored time. |
| Companion gear | retinue, equipment | None. Companions fight in encounters only where personal combat exists. |
| Mass battle | retinue | Without morale or proficiencies, those terms leave the formula; without prisoners, losses are never captured; champion duels need personal combat. Rewards, loot and lasting injuries are separate parts; looted gear needs equipment. |
| Prisoners | retinue, and mass battle or personal combat | Ransom needs the economy; escapes run on a prison tick, so they need recurring schedules; holding prisons need holdings. |
| Holdings | none | Income needs the economy and schedules; garrisons need the retinue; sieges need mass battle; buildings need the economy and world time; raiding needs factions at war; village unrest needs mass battle; demanding supplies and livestock need the economy; bandit trouble needs recurring schedules and offers. Without factions, owners are characters. |
| Politics and orders | factions and standing | Membership ranks, orders, marriage and a founded kingdom are separate parts. Marriage needs per-character relation tracks. |
| Knowledge and news | none | Without world time, remembered prices and whereabouts carry no age; without the economy, nothing is remembered about prices; without characters who move, whereabouts are always current. |
| World agents | recurring schedules | Without a retinue, parties carry no rosters and cannot fight; without mass battle, they never fight each other; without factions, there is no diplomacy; without the economy, no caravans trade. |
| Faction strategy | factions and standing, world agents | The marshal, fief grants, defection, feasts and claimants are separate parts. Campaigns need holdings and mass battle; fief grants and feasts need holdings; defection needs per-character relation tracks. Without campaigns, factions only defend, rest and, where holdings exist, raid. |

Every periodic rule runs on a recurring schedule that belongs to its own
capability (the retinue's upkeep tick, the prison tick, the world-agent tick),
and each draws from its own RNG domain. Adding or removing one capability
therefore never shifts another's timing or random draws.

Each capability section below describes the capability with all its parts
present.

## What already fits

- **Places.** Towns, castles and villages are locations. A location's
  `characters` list places lords, merchants and guild masters.
- **People.** Lords and companions are characters with dialogue. Conversations
  already branch on flags, quest states and technique ranks.
- **Personal fights.** Bandit ambushes and duels are encounters. Authored
  groups, fleeing, yielding and critical hits all exist
  ([Encounters](world-format.md#encounters)).
- **Gear.** Equipment instances, forging, improvement and enchanting exist
  ([Equipment](equipment.md)).
- **Chance and continuity.** A seeded, saved RNG exists, and so do saves and
  recovery.
- **Endings.** Route outcomes are designed with separate `completion` and
  `terminal` properties
  ([Section 3](open-decisions.md#3-outcomes-failure-and-replay)), so a sandbox
  can mark an ambition as achieved and keep playing.

## Foundations

These are general improvements that a sandbox needs first. Most of them are
already planned or decided for authored adventures. The roadmap delivers them as
M5 slices.

1. **Condition tree** (delivered in M5a). Conditions are `All / Any / Not`
   over typed leaf predicates, and a `requires` list became one condition. A sandbox adds many
   numeric predicates: currency held, standing with a faction, whether two
   factions are at war, who holds a location, the world time. Each predicate
   stays domain-typed, for example `CurrencyAtLeast`, `StandingAtLeast`,
   `AtWar`, `HoldsLocation` or `WorldTimeWithin`. There is still no generic
   numeric comparison and no variable bag
   ([Section 6](open-decisions.md#6-conditions-and-effects)).
2. **Effect lists** (delivered in M5a). A dialogue choice carries `effects`, a
   list applied in authored order to staged state, in place of the single
   optional `effect` it had before.
   The whole transition commits or fails together. Each effect variant arrives
   with the capability that owns its state: paying coin, changing standing,
   adding troops, granting a holding.
3. **World time and travel** (delivered in M5b). Optional world time follows
   [Section 9](open-decisions.md#9-time-models): minutes from an authored epoch,
   advanced only by explicit actions. The sandbox needs three additions:
   - Roads carry an authored travel duration.
   - Exits are no longer limited to the six compass directions, because a
     crossroads town may have more roads than that.
   - Actions that pass time: wait for a duration and rest until an hour.

   One-shot scheduled events are already specified. The new proposal is
   **recurring schedules**, such as "every 1,440 minutes from minute 360", with
   a period of at least one minute. The
   dispatch cursor expands them lazily, one due occurrence at a time, in the
   same chronological order as one-shot events. Wages, taxes, market restocking
   and world-agent ticks all run on recurring schedules. The engine never polls
   a clock.
4. **Quest lifecycle.** The current quests are one-shot and cannot fail. A
   sandbox needs quests that fail, carry a deadline in world time and can be
   abandoned. Failure is an ordinary authored state change, not an outcome,
   unless the route says otherwise. It also needs **offers**: repeatable job
   templates such as "deliver cloth to a town" or "clear the bandits near a
   village".
   - An offer's parameters are drawn by the saved RNG from authored candidate
     lists (target location, goods, reward band). The draw happens only at an
     explicit gameplay transition, such as the giver's recurring refresh or the
     player's arrival, and the drawn offer is saved. Opening a menu or talking
     only reads offers that already exist, so inspecting jobs never consumes
     randomness ([Section 7](open-decisions.md#7-randomness-and-reproducibility)).
   - The drawn values fill the offer's text with the existing single-pass
     template substitution.
   - Offers are activities, not side questlines. They do not break the rule
     that main progress unlocks side questlines in bounded waves.
5. **Runtime instances for spawned copies and parties.** A roaming war party, a
   bandit gang or a caravan needs mutable identity that outlives one encounter,
   and several copies of one template may be alive at once. Unique authored
   characters keep their authored IDs: a wounded lord's wounds, a captured
   companion's captivity and a lord's whereabouts are state keyed by the
   character ID, not a second identity. This follows
   [Section 10](open-decisions.md#10-definitions-instances-and-identity), and
   instances for combatant copies are already planned for M5.
6. **Sandbox routes.** A route still owns exactly one main questline, and a
   playthrough records at most one outcome
   ([Section 3](open-decisions.md#3-outcomes-failure-and-replay)). In a sandbox,
   the main questline is a ladder of ambitions: quests such as "hold a fief"
   and "found an order" are ordinary quest progression. Only the top rung, such
   as "rule a kingdom", is the route's **completed, non-terminal** outcome, so
   play continues after it.
7. **Dialogue by role.** A realm has dozens of lords, ladies, guild masters and
   village elders, and most of what they say is the same for everyone in the
   role: asking about the war, seeking service, requesting a fief. Writing it
   once per character would multiply the package and drift out of step.
   - A world declares **roles** in its own language, and each character lists
     the roles it holds, in order. Roles are authored and do not change during
     play; what a member can say still changes, because conditions read state.
   - A role may author **dialogues** and **topics**. Talking to a character
     opens its own dialogue when it has one whose opening condition holds,
     otherwise the first matching dialogue of its roles, in order. A role's
     topics are choices appended to every member's opening node, after the
     character's own choices, when their conditions hold. A topic belongs to
     the role: its `next`, and every `next` below it, names a node in the
     role's own node list, so a shared topic can run several exchanges
     without touching the member's dialogue.
   - Inside a conversation, conditions, effects and templates may refer to the
     **speaker**, the character being talked to, wherever they accept a
     character: `RelationAtLeast { with: Speaker }`, `HasTrait(Speaker, …)`,
     `ChangeStanding { target: Speaker }`, `JoinRetinue(Speaker)`, and
     `{speaker}` or `{speaker_faction}` in text. `Speaker` is a typed reference
     resolved when the choice is evaluated, not a variable.
   - A role may declare **line slots**, named texts that each member supplies,
     such as a companion's introduction, backstory, request to join and
     objection to a deed. Shared nodes show a slot in place of fixed text.
     Validation requires every member to author every slot the role's
     dialogue can reach, or the role to author a default for it.
   - An offer's giver may be a role, so every guild master offers the same job
     templates, drawn for each one separately.
8. **Text variants.** Some text depends on who the player is or how things
   stand: a form of address that follows the player's chosen identity, a
   greeting that follows a relation. A player-facing text field may be a list of
   variants, each with a condition, ending in one without; the first whose
   condition holds is shown. Choosing a variant reads state only, so it draws
   no randomness and changes nothing. Templates keep their single-pass
   substitution: there is no conditional syntax inside a string.
9. **Start choices.** A sandbox player usually begins by answering a few
   questions about their past: a noble's child, a merchant's apprentice, a
   deserter. A route may author a short sequence of start questions shown at
   New Game, before the first turn. Each option applies an ordered list of
   ordinary effects to the route's initial state: stat points, gear, currency,
   standing, flags, techniques. Later text and conditions read those results
   like any other state, which is how a form of address follows the player's
   chosen identity. The answers themselves are saved only as the state they
   produced and a list of chosen option IDs for display. Start choices are
   not runtime identity editing: once play begins, nothing re-runs them.
10. **Characters who move** (delivered in M5b). Lords travel with their
    parties, but other characters move too: a travelling storyteller, a ransom broker or an
    unhired companion who drifts between taverns. A character may author a set
    of locations and a recurring schedule; on each occurrence the engine moves
    it to one of them, drawn from a `world` RNG domain. Its location is saved
    state keyed by its character ID, and it is present only where it
    currently is. While the character rides in a party, including the
    player's retinue, or is held captive, its occurrences are skipped; when it
    is released or leaves the party, it stays where that happened until its
    next occurrence.

Proficiencies such as trading, leadership and field surgery are not a
foundation. They follow [Section 8](open-decisions.md#8-checks-and-proficiencies):
each capability that needs one owns its ranks and checks. The economy owns
trading, the retinue owns leadership, and mass battle owns surgery and looting.
A technique rank is not such a proficiency. M4d's Smithing technique, which
gates recipes, already blurs that line;
[Section 18](open-decisions.md#18-living-sandbox-worlds) records the question.

- **Growth.** Proficiency ranks are bought with proficiency points, which a
  world grants per level in the same level table that grants stat points
  (M4a), and saved as an allocation per character. A world may cap each rank
  by an authored stat, so a character must be strong to be a good surgeon.
  Effective ranks are derived, never saved twice.
- **Personal or party.** Each proficiency is authored as personal (only its
  holder's rank counts) or party (the retinue uses the best rank among the
  player and the companions riding with them). Leadership is typically
  personal; surgery, trading and pathfinding are typically party, which makes
  companions worth recruiting for their skills.
- **Study.** An item may author a proficiency or technique it teaches. Studying
  it is an action that passes authored world time, spread over rests or done
  at once, and then grants the rank or technique XP. An item may instead grant
  a rank bonus while carried. Without world time, study completes at once.

## Sandbox capabilities

Each capability below states what it owns, what the player can do, and what it
adds to conditions and effects. Like every capability, it is present only when
the world authors it, and it arrives with a fixture that proves it.

### Economy

Prices come from what each place makes and needs. A town surrounded by
vineyards sells wine cheaply; a town with looms but no flocks pays well for
wool. Caravans carrying goods between towns pull their prices together, and
trade makes places prosper. The authored numbers describe production, not
prices: the engine derives every price from them.

- **Goods.** Each trade good authors a base price and its demand in each kind of
  market (town, village). A processed good may author a recipe: one primary and
  at most one secondary input good, the input used per run, the output per run
  and an overhead cost. Goods may also author a provisions value (see
  [Retinue](#retinue)).
- **Producers.** A world declares a closed list of producer kinds in its own
  language (grain fields, herds, mills, looms, smithies, …). Each kind authors
  how much of which goods one unit yields and consumes per price tick. Every
  market authors its counts of each kind. A village names its market town, and
  its production and demand count towards that town's trade as well as its own.
- **State.**
  - The player's currency, a fungible quantity.
  - A **price index** for each good in each market, in thousandths of the base
    price: 1,000 is the base price. It stays within authored bounds, typically
    100 to 10,000, so a price moves between a tenth and ten times its base.
  - Each market's **prosperity**, a bounded integer.
  - Each merchant's current stock of trade goods and currency.
- **Price tick.** On the economy's recurring price schedule, every market
  updates every good's index in four phases. Each phase finishes for all markets
  before the next begins, and draws come from the economy's own `market` RNG
  domain in authored market order, then authored goods order:
  - The net supply is production minus consumption. A surplus lowers the index
    by a draw below an authored multiple of the surplus, damped once the index is
    already under an authored level; a shortage raises it the same way.
  - The index then reverts towards 1,000 by an authored share of the gap.
  - A processed good whose input is dearer than the good itself is pulled up by
    an authored share of the difference, so a recipe's output never stays cheaper
    than its input for long.
  - Linked markets, a village and its market town or two towns joined by an
    authored trade link, each move an authored share of the gap towards the
    other. Every gap is measured on the values the third phase left, and each
    market applies the sum of its links' moves at once, so the result does not
    depend on link order. Validation keeps the shares of one market's links at
    or below 100% in total, so convergence never overshoots.
- **Initial prices.** The package authors each market's starting indices. An
  authoring tool may compute them by running the price tick offline for a number
  of warm-up rounds; the engine never warms up at runtime, so New Game draws no
  randomness for prices.
- **Caravans.** With [world agents](#world-agents), a caravan is a party whose
  trade policy travels an authored trade link. On arrival it moves every good's
  index in the destination an authored share of the way towards the origin's,
  pays the destination's owner a tariff proportional to the price gaps it
  closed and to the destination's prosperity, and may raise that prosperity.
  Without world agents, trade links still converge on the price tick.
- **Prosperity.** On the economy's daily schedule, each market's prosperity
  moves one step towards an ideal value: an authored base, lowered for each
  demanded good whose index sits well above base (scarcity), raised by the
  market's buildings where [holdings](#holdings) exist. Caravan arrivals and
  village trade raise it; raids, sieges and bandit trouble lower it by authored
  amounts. Prosperity scales holding income, tariffs, merchant stock and
  recruit pools by authored percentages.
- **Merchants.** A merchant character trades at a market. On the market's
  restock schedule its stock is redrawn from the `market` RNG domain: goods
  weigh by the size of their net supply relative to their price, and the amount
  grows with prosperity. Opening a trade screen never draws.
- **Buying and selling.**
  - The buying price is the base price × the index ÷ 1,000, raised by a spread;
    the selling price is lowered by the same spread. One rounding, in integers.
  - The spread is an authored percentage that the economy's trading
    proficiency narrows. It widens by authored amounts at villages, for goods
    the market neither makes nor needs, and when the merchant's relation with
    the player is negative.
  - Each unit bought raises that market's index for the good by an authored
    step, and each unit sold lowers it, so the player's own trade moves prices
    at once. Dumping one cargo in one town stops paying.
  - Selling is limited by the merchant's currency, which the restock refills.
- **Workshops.** The player may buy a workshop in a town, at most an authored
  number per town, through a dialogue effect with the town's authored seller. A
  workshop runs one processed good's recipe. On the economy's weekly schedule it
  pays the output's value × runs, minus the inputs' values and the overhead,
  which can be a loss. Values use the local base price × index ÷ 1,000, with no
  spread, since the workshop trades in its own town rather than with the player,
  and its runs do not move the index. Currency never goes below zero: a loss
  takes at most what the player holds, and an event reports any shortfall.
  Selling or closing it is another dialogue effect. Workshops are owned
  property, not [holdings](#holdings): they have no garrison and never change
  hands in a war.
- **Commands.** Buy and sell. The engine previews the price, and the change the
  trade makes to it, before the player confirms. Buying and selling a workshop
  happen in dialogue.
- **Conditions and effects.** `CurrencyAtLeast`, `OwnsWorkshop`; `PayCurrency`,
  `GrantCurrency`, `BuyWorkshop` and `SellWorkshop`.
- **Bounds.** Validation caps indices, prosperity, producer counts, stock, the
  player's and merchants' currency and every authored rate so that engine
  arithmetic stays small, following the existing `*_BOUND` pattern. Save
  validation checks every saved index, prosperity, stock and currency against
  the same bounds.
- **Simulator parity.** The price tick is a formula over authored numbers, like
  damage. `scripts/combat_sim` gains an economy model that mirrors it before
  tests pin its numbers, and authors use it to check that trade routes pay
  without one route dominating.

### Factions and standing

- **Relationship to the core.** Factions already exist as world entities
  ([Section 1](open-decisions.md#1-minimum-universal-world-core)). This
  capability adds mutable state to them.
- **Diplomacy.** A war/peace relation between each pair of factions. It starts
  as authored and changes only through effects: `DeclareWar` and `MakePeace`.
- **Standing.** Authored standing tracks, each a bounded integer with named
  thresholds. A world might author renown (global), honour (global), standing
  with each faction, and relation with each lord. There is no universal list of
  tracks.
- **Conditions and effects.** `StandingAtLeast`, `AtWar`; `ChangeStanding`.

### Personalities

Lords and companions differ in temperament, and that temperament shows in what
they do, without scripting each character.

- **Traits.** A world declares a closed list of traits with names in its own
  language (honourable, cruel, cautious, ambitious, quarrelsome, …), and each
  character authors a few. Traits are authored data and do not change during
  play.
- **Behaviour.** A world agent's authored policy priorities may test its
  leader's traits (`LeaderHasTrait`). A cautious lord declines battle below
  authored odds, a cruel one raids villages, and an ambitious one besieges
  first. Diplomacy event weights may test a ruler's traits the same way. The
  engine still offers only its closed set of policies; traits only choose
  among them.
- **Reactions to deeds.** Deeds come from two sources:
  - The engine reports a closed set it can detect itself: raiding a village,
    demanding its supplies or driving off its livestock (with holdings),
    releasing or ransoming a prisoner, and leaving wages unpaid. A deed
    whose capability is absent never occurs.
  - The world declares its own deed IDs, such as winning a tournament or
    breaking a promise, and reports them with an authored `ReportDeed` effect
    wherever its dialogue or quests decide the deed happened.

  The world authors reaction rules of the form trait × deed → standing change,
  applied to the characters with that trait whom the rule names by scope (the
  retinue's companions, the lords of an affected faction). Reactions are
  ordinary effects, applied in authored order and reported as events.
- **Companion friction.** A companion whose relation with the player falls
  below an authored threshold leaves in the same transition that lowered it,
  taking nothing by default, so no tick is needed. Pairs of traits may author a
  mutual dislike that lowers morale on each upkeep tick while both companions
  ride together.
- **Dialogue.** `HasTrait` conditions let conversations and offers differ by
  temperament.

### Retinue

- **Troops.** Troop definitions optionally carry wages, an upgrade path
  (recruit → footman → sergeant), a travel speed where the world uses travel
  speed, a mass-battle strength where the world has mass battles, and a
  personal-combat profile only where troops also join encounters.
- **Upkeep tick.** The retinue's periodic rules run on one recurring schedule
  that the world authors for it, typically daily: wages, provisions, wounded
  recovery, morale drift and desertion, in that fixed order. Its random draws
  come from its own `retinue` RNG domain, so world agents, battles and prisons
  never shift them. Without recurring schedules, none of these periodic rules
  exist.
- **Roster.** The player's roster holds, per troop definition, a count of
  healthy troops, a count of wounded troops and an XP pool: troops are
  fungible, not instances. All three are saved.
- **Wounded troops.** Wounded troops do not fight, still draw wages and count
  towards the size limit. On each upkeep tick an authored share recovers, more
  at a location that authors rest or healing, and the retinue's surgery
  proficiency raises it.
- **Companions.** Companions are unique characters who join the retinue through
  authored dialogue effects and leave the same way. Each keeps its authored ID
  and its own state: wounds, technique ranks and, in worlds with levels, its
  own XP and level. See [Companion gear](#companion-gear).
- **Recruiting and wages.** Recruiting happens at locations that offer it.
  Each such location keeps a saved pool: a count per troop definition that
  recruiting takes from. On an authored recurring schedule the pool refills
  towards an authored size, scaled by the location's prosperity where the
  economy exists and gated by the player's standing with the location's owner
  where standing exists. Without schedules, a pool never refills.
  Wages fall due on the upkeep tick. What happens when they go unpaid
  (desertion, lost standing) is authored.
- **Size limit.** The engine computes the roster limit as an integer: an
  authored base plus authored contributions from standing tracks and the
  retinue's leadership proficiency. Companions count towards it; prisoners have
  their own limit (see [Prisoners and ransom](#prisoners-and-ransom)). A command
  that would exceed the limit is refused. That includes a dialogue choice whose
  join effect would exceed it: the transition is atomic, so the whole choice is
  refused and nothing commits. Authors gate such a choice with a `RosterHasRoom`
  condition so that it shows as unavailable instead. If the limit later drops,
  for example after lost renown, nobody leaves; recruiting is refused until the
  roster is back under it. Agent party templates author their own fixed limits,
  and a validation bound caps every roster.
- **Upgrades.** Battles add XP to the pool of each surviving troop type, and
  recruits join with no XP. Every troop that leaves a type, whether by upgrade,
  death, capture or desertion, takes its share of that type's pool: the pool
  divided by the type's healthy and wounded count, rounded down. A troop can
  upgrade once its share reaches the next step's authored XP. It pays that XP
  from its share and carries the rest into its new type's pool. Any other
  leaver's share is lost, and a type whose last troop leaves also drops any
  rounding remainder. XP is only moved or spent, never created outside battle,
  so a pool never outlives its troops and upgrades available after a save are
  the same as before it.
- **Provisions.** Trade goods may author a provisions value. On each upkeep
  tick the retinue eats provisions per head, prisoners included, from its
  carried goods in authored goods order. A hungry retinue
  loses morale, and starvation is authored (desertion, wounds).
- **Morale.** A bounded integer for the whole retinue, raised and lowered by
  authored amounts for meals, variety of food, paid or missed wages, victories
  and defeats, companion friction and the leadership proficiency. Low morale
  lowers mass-battle strength, and below an authored threshold troops desert on
  upkeep ticks, drawn from the `retinue` RNG domain. Agent parties use their
  template's fixed morale unless a world authors more.
- **Travel speed.** A road's authored duration is scaled by the party's speed:
  the slowest of the world's authored base party speed (which covers the player
  and companions) and each healthy troop type's speed, minus authored penalties
  for roster size, wounded troops and prisoners, plus a pathfinding proficiency.
  A party with no healthy troops moves at the base speed. The engine computes it
  in integers with one rounding. A road with an authored duration never takes
  less than one minute; a road without one still takes no time. Agent parties
  use the same rule, so light raiders can catch a slow caravan on the road.
- **Mounts.** Where the world has equipment, a piece may author a travel speed
  for its wearer, as a riding animal does. The base party speed is then the
  slowest of the player's and each companion's mount speeds, falling back to
  the authored base for anyone without one. In an encounter a mount is ordinary
  gear: its stat grants, such as speed, apply like any other piece's.

### Companion gear

Equipment today is worn by the player alone. With a retinue, every retinue
character can wear gear:

- **Wearer.** Each equipment instance has at most one wearer, the player or a
  companion, and the retinue shares one stash for unworn pieces. The saved
  instance records its wearer; moving a piece between characters keeps its
  instance ID, tier and enchantment.
- **Equipping.** The equip command names the wearer. Slots, swaps and the
  before → after preview work exactly as for the player, against that
  character's slots.
- **Derived stats.** Every retinue character's effective stats are derived by
  one function, like `rules::player_stats` today: the character's base
  (combat profile, or level table plus allocated points where the world gives
  companions levels), technique passives and worn gear, minus any
  [lasting injuries](#mass-battle). They are never saved.
- **In fights.** Companions join personal encounters as allies on the player's
  side, which the encounter state already supports, and count as leaders in a
  mass battle. They are wounded rather than killed unless an authored rule says
  otherwise.
- **Leaving.** A companion who leaves takes nothing with them by default; their
  worn pieces return to the stash unless an authored effect says otherwise.

### Prisoners and ransom

- **State.** A prisoner roster held by the player, by an agent party or by a
  holding's prison: a count per troop definition, plus unique characters whose
  captivity is state keyed by their character ID (held by whom, since when).
  Prisoners have their own authored limit per holder.
- **Taking prisoners.** When a mass battle ends, an authored share of the
  losing side's casualties, drawn from the battle's RNG domain, become the
  winner's prisoners instead of dying, up to the winner's limit. A defeated
  unique leader is captured, not killed, unless an authored rule says
  otherwise. A personal encounter can capture a yielding opponent the same way.
- **Using prisoners.**
  - Sell troop prisoners to a ransom broker at a location, for an
    engine-computed price per troop definition (economy).
  - Recruit prisoners into the retinue when an authored condition allows it,
    which counts against the roster limit.
  - Ransom a captured lord: an authored offer from the lord's faction, drawn at
    a gameplay transition like any offer, pays currency for the release.
  - Release a prisoner, with authored standing effects.
- **Escape.** On a prison tick, a recurring schedule the world authors for
  prisoners, they may escape with an authored chance drawn from their own
  `captivity` RNG domain; a holding's garrison and buildings lower it.
- **The player captured.** Losing a mass battle may capture the player instead
  of ending the route. Captivity is an ordinary setback
  ([Section 3](open-decisions.md#3-outcomes-failure-and-replay)): travel and
  most actions are refused until an authored release (time served, ransom paid,
  or escape) ends it. The retinue is scattered or captured as authored.
- **Conditions and effects.** `HoldsPrisoner`, `IsCaptive`; `CapturePrisoner`,
  `ReleasePrisoner`, `RansomPrisoner`.

### Mass battle

The [capability catalog](capabilities.md#combat-across-sources) keeps mass
battles out of personal combat. This capability resolves army against army.

- **Resolution.** The engine computes each side's strength from its healthy
  roster, its leaders, its morale and the ground. It draws from the RNG and
  distributes losses over several rounds.
- **Losses.** Each loss is killed or wounded by an authored share that the
  side's surgery proficiency raises. Wounded winners stay in their roster; the
  losing side's losses may instead become the winner's prisoners (see [Prisoners
  and ransom](#prisoners-and-ransom)). The player sees the result and the
  losses, not a blow-by-blow fight.
- **Rewards.** A won battle grants the defeated side's authored XP to the pools
  of surviving troop types and, in worlds with character levels, to the player
  and each companion who fought, split by authored shares. Without character
  levels, those shares are simply not granted. It changes an authored standing
  track, such as renown, by an amount that grows with the defeated side's
  strength relative to the player's. Each defeated troop definition authors a
  loot table; the battle's RNG domain draws from it, with more draws for a
  better looting proficiency, into a loot pool the player takes from before
  leaving. Anything left is lost. Improvement tiers only rise from an item's
  base ([Equipment](equipment.md)), so a worn or rusted copy is its own authored
  definition, which a loot table names like any other item.
- **Lasting injuries.** A world may author injuries, each with stat
  penalties. Defeat in a battle, or an authored effect, may inflict one on the
  player or a companion, drawn from the battle's RNG domain with an authored
  chance. Injuries are saved as IDs keyed by the character; effective stats
  subtract their penalties, like gear in reverse, so nothing is saved twice.
  Subtraction saturates at each stat's lower bound (1 for HP and speed, 0 for
  the others), as armour's speed penalty already does, so no combination of
  injuries makes a stat invalid. An authored effect, such as a physician's
  treatment, removes one.
- **Ground.** Locations and roads both author a ground modifier, neutral when
  omitted, so a battle has ground wherever parties can meet: at a location or
  on a road between two.
- **Champions.** Optionally, the author can let the player fight a personal
  encounter against the enemy commander at a chosen moment. The result of that
  duel modifies the battle.
- **Simulator parity.** The formula must be mirrored in `scripts/combat_sim`
  before its numbers are pinned in tests, exactly as the personal-combat
  formulas are.

### Holdings

- **Ownership.** A holding is a location with an owner, a faction or a
  character, that can change during play.
- **Income.** A holding produces income and supplies on a recurring schedule.
  A holding that is also a market scales it by its own
  [prosperity](#economy); one that is not, such as a castle, takes its
  income unscaled.
- **Garrison.** It keeps a garrison roster.
- **Prison.** It holds prisoners within an authored limit that buildings can
  raise, and a captured lord taken there waits for ransom or escape.
- **Buildings.** Authored improvements (a mill, walls, a chapter house) cost
  currency and world time and change the holding's numbers.
- **Sieges.** A siege is a mass battle against the garrison after an authored
  preparation time. The walls improve the defenders.
- **Raiding.** The player may raid a village holding whose owner's faction is
  at war with the player's. The raid takes authored world time, is resisted by
  the garrison through mass battle where the world has one, yields the
  holding's authored loot of goods and currency, and stops its income for an
  authored period. It is the engine-detected deed "raiding a village". An
  agent's raid policy applies the same rule.
- **Villages.** A village holding offers more than raiding:
  - Where its owner authors it, income accrues in the holding and must be
    collected in person. Collecting may provoke unrest with an authored chance
    that low relation with the villagers raises, ending in a small mass battle
    or a lost share of the income.
  - Where the economy exists, a hostile player may demand supplies or drive
    off livestock, which take trade goods. Both are engine-detected deeds that
    lower relation and prosperity, lighter than a raid.
  - Livestock, also only with the economy, is a trade good that authors itself
    as a herd: carried herds slow travel by an authored penalty per head and can
    be slaughtered into an authored provisions good.
  - Bandit trouble is a village state that the holdings' own village schedule
    sets with an authored chance, drawn from the `villages` RNG domain that
    collection unrest also uses, so worlds with and without agents draw the
    same. It lowers prosperity and empties the recruit pool until an offer from
    the village clears it, such as hunting the bandits down or training the
    villagers to resist.
- **Conditions and effects.** `HoldsLocation`, `VillageTroubled`;
  `GrantHolding`, `TransferHolding`.

### Politics and orders

Politics composes the capabilities above rather than adding a new one.

- **Membership ranks.** A faction may author ranks, each with requirements
  (standing, a holding, currency). The same shape covers vassalage and
  membership of a knightly order.
- **Orders.** An order is a faction with ranks and its own troop definitions.
  Its chapter house is a building on a holding.
- **Courtship and marriage.** Built on the relation tracks above and authored
  dialogue, not a separate capability.
  - An unmarried character may author that it can be courted, and names a
    guardian, typically a parent or the head of the house.
  - Courtship is authored dialogue gated on the player's relation with that
    character and standing with the guardian; visits, gifts and feasts raise
    that relation through ordinary effects.
  - Marriage is an effect, `Marry`, that records at most one spouse for each
    side, keyed by character ID. A spouse's house then counts as kin through
    authored standing changes, and the spouse may live at, and help run, one of
    the player's holdings through authored modifiers.
  - Condition: `IsSpouse`. Children, heirs and dynasties are not modelled;
    marriages between other characters are authored state or world events.
- **Culture is not a runtime concept.** Culture decides which troops a village
  offers, the troop line a faction fields, and its names and prose, but none of
  that changes during play. Locations and factions reference their troop
  lines and recruit pools directly; an authoring tool may group those
  references as a "culture" before export. Culture becomes runtime state only
  if a world lets it change, such as a conquered town slowly taking its new
  owner's culture; [Section 18](open-decisions.md#18-living-sandbox-worlds)
  keeps that question open.
- **Founding a kingdom.** The player's own kingdom is an authored, dormant
  faction that an effect activates. Nothing is created at runtime beyond
  instances, so validation still sees every faction that can ever exist.

### World agents

World agents are what make the world move without the player.

- **Parties.**
  - A party is a runtime instance of an authored party template: a roster, an
    owning faction, a position and a goal.
  - A position is either a location or a point on a road between two
    locations.
  - Lords' war parties, bandit gangs, caravans and invading hosts are all
    parties.
- **Ticks.** On each world tick, which is a recurring schedule, every party
  applies one policy from a closed, engine-defined set: patrol, raid, escort,
  besiege, follow, trade and return home. Follow serves a
  [faction's](#faction-strategy) marshal; trade serves a caravan's route in the
  [economy](#economy).
  - Authored priorities choose the policy. For example, a lord at war besieges
    the nearest enemy holding with a weak garrison, and otherwise patrols home
    lands.
  - "Nearest" and every route a party takes are measured on the road graph by
    total authored travel time. Ties break by authored road order, so routing
    is deterministic.
  - Parties act in a stable order.
  - Their random choices draw from their own RNG domain, so adding a bandit
    gang does not change the player's critical hits, desertions or prison
    escapes.
- **Agent wounded.** An agent party's roster keeps healthy and wounded counts
  like the player's. Its wounded recover on each world-agent tick by the
  template's authored share, with no randomness, so battles between agents
  wear parties down without leaving them crippled for good.
- **Spawners.** Locations spawn parties from templates, with authored caps and
  intervals. A destroyed party is removed; its instance ID is never reused
  within the same history.
- **Interception.** Travel along one road is a single step, except when a world
  tick falls during the journey while a hostile party is on the same road. The
  player then stops at the road's midpoint, and any battle there uses that
  road's ground. Arriving at a location where a hostile party stands also
  intercepts. Either way, the player chooses to fight, talk or flee.
- **Off-screen battles.** Parties that meet away from the player resolve through
  mass battle.
- **Diplomacy.** Kingdoms change diplomacy through authored, weighted world
  events that are checked on ticks. Lords are not scripted individually; their
  [personalities](#personalities) choose among authored rules.
- **Bounds.** A new `AGENT_BOUND` caps the number of live parties, so ticks
  and saves stay small.

### Faction strategy

World agents move one party at a time. Faction strategy lets a kingdom act as a
whole: gather its lords into an army, march on an enemy castle, hand out what it
takes and make peace when the war goes badly. Like agent policies, it is a
closed set of engine rules chosen by authored priorities. There is no planner,
no language model and no per-faction script.

- **Faction tick.** A recurring schedule, typically daily, that falls before
  the world-agent tick at the same minute. Factions act in authored order and
  draw from their own `faction` RNG domain.
- **Stance.** Each faction holds one stance from a closed set: defend, gather,
  campaign (with a target holding), raid (with a target village) and rest.
  Authored priority rules over typed conditions choose it on each tick. A world
  might gather when at war and rested for an authored time, campaign once the
  gathered strength reaches an authored multiple of the target's garrison, and
  rest when the army falls below an authored share of its gathered strength or
  the campaign runs past an authored length.
- **Marshal.** A faction may author that its ruler appoints a marshal. When the
  office is empty, the ruler picks the eligible member with the highest value
  of an authored standing track, such as renown, with ties broken by relation
  with the ruler, where the world has a per-character relation track, and
  then authored order. A player member can be appointed.
  While the faction gathers or campaigns, the marshal's party leads and
  members' parties take the `follow` policy towards it, unless one of their own
  priorities wins, such as relieving their besieged holding.
- **Targets.** A campaign targets the nearest enemy holding, by travel time
  from the marshal, whose garrison strength is below the army's by an
  authored margin; a raid targets the nearest enemy village by travel time
  from the faction's ruler. Ties break by authored location order.
- **The player in an army.** A player member receives the marshal's summons as
  an offer, and authored standing changes follow for answering or ignoring it.
  A player marshal chooses the target through commands instead.
- **Fief grants.** A holding the faction takes is granted on its next tick: to
  the player if the player asked for it and meets authored conditions, otherwise
  to the member with the fewest holdings, ties broken by relation with the
  ruler, where that track exists, and then authored order. Members passed over
  lose authored relation with the ruler.
- **Defection.** A lord's faction membership is saved state keyed by the
  character ID. On the faction tick a member whose relation with its ruler is
  below an authored threshold may leave, with an authored chance, for the
  faction whose ruler it likes best, taking its holdings with it.
- **Diplomacy.** The weighted diplomacy events of [world agents](#world-agents)
  gain typed inputs: how long a war has lasted, holdings and battles lost in
  it, and how many wars each side fights. Making peace starts an authored truce
  during which neither side can declare war. Authored border incidents are
  world events that raise the weight of war for a while.
- **Feasts.** In the rest stance a ruler, or a member with enough holdings, may
  hold a feast at one of its holdings for an authored time, and members'
  parties travel there. The player finds the realm's lords and ladies in one
  place, and authored standing effects reward attending.
- **Claimants and rebellion.** A claimant is a character with an authored claim
  to a faction. Backing the claim activates an authored dormant faction, like
  a player-founded kingdom, and lords persuaded through dialogue defect to it.
  No faction is created at runtime.
- **News.** Stance changes, appointments, grants and defections are notable
  world events (see [Knowledge and news](#knowledge-and-news)).

### Knowledge and news

The world changes while the player is elsewhere, and the player should learn
of it the way a traveller would, not see everything at once.

- **Notable events.** A closed set of world events is notable: war declared,
  peace made, a faction changes stance, a holding changes owner, a siege
  begins, a village is raided, a lord changes faction, a marshal is
  appointed. When a command crosses one, the
  engine reports it after the command's own events. A world authors which of
  them the player hears at once (for example those involving the player's
  faction, holdings or companions); the rest reach the player on arrival at a
  town. Recent notable events are kept in a bounded, saved journal.
- **Remembered prices.** Where the economy exists, the player remembers each
  market's prices as last seen, with the minute seen. Visiting refreshes them;
  so does an authored `RevealPrices` effect, such as buying a merchant's report.
  Price previews for other markets show the remembered values and their age.
- **Whereabouts.** Where characters move, the player remembers each
  character's last known location. Seeing them refreshes it, and an authored
  `RevealWhereabouts` effect, such as paying a traveller, reveals it.
- **Presentation.** Clients show only what the player knows. The engine still
  holds the whole state, so this is information disclosure
  ([Section 14](open-decisions.md#14-presentation-and-information-disclosure)),
  not a second copy of the world.

### Tournaments and other set pieces

Tournaments, arena fights and wagers need no capability. They are encounters
plus currency effects, gated by standing or world time.

## Reference fixture

Every capability ships with a fixture that proves it. The sandbox fixture is a
small, original world, provisionally `examples/marches`:

- three towns, one castle and two villages on a road network;
- two kingdoms and one knightly order;
- one bandit spawner and one caravan route.

It grows one slice at a time, the way `arena` and `smithy` did. Its walkthrough
covers 30 or more in-world days. During them:
- the player trades, recruits, wins a mass battle and receives a fief, which in
  the fixture is the top ambition and so the route's completed, non-terminal
  outcome, and play continues;
- a marshal's campaign changes a holding's owner and the ruler grants it,
  without the player's involvement.

## Non-goals

- a scripting or behaviour language, or per-character AI scripts;
- real-time play, or any gameplay tied to the wall clock;
- terrain, continuous coordinates or free-space pathfinding in the rules (the
  road graph is the map, and routing is shortest travel time on it; clients may
  draw it however they like);
- entities created at runtime other than instances of authored definitions;
- a generic bag of numeric variables;
- importing data from other games.
