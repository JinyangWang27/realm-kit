# Combat architecture

RealmKit is a deterministic RPG engine, not a turn-based combat engine. A
fighting character's progression and rules hold however a fight is run; the
paused initiative timeline is one way to run one, and a fixed-step real-time
duel is another. This page says which parts belong where, how MP and Rage
work, how real-time duels run and where their authority ends, and what is not
supported yet. Package fields are in
[world-format.md](world-format.md#combat-block-numeric-rules-and-templates).

## Three layers

| Layer | What it owns | Where |
| --- | --- | --- |
| **Character core** (universal) | Levels and XP, stat points, effective stats, gear and techniques, HP, MP and Rage, skill costs, damage and crits, Rage from acting and from damage, resource regeneration and decay, validated vitals | `realmkit-engine/src/character/`, `gear.rs`, `techniques.rs`, `formulas.rs`; spec `stats.rs`, most of `combat.rs` |
| **Turn-based encounters** (optional) | The initiative timeline, action delays, whose turn it is, opponents' policy, attack, skill, flee and item turns, how encounters start and end, and timeline MP regeneration | `realmkit-engine/src/encounter/`; spec `Timeline` |
| **Real-time fights** (optional) | A fight's fixed-step clock, its fighters and teams, joins, targets, attack phases, guard and retreat timing, which intents are legal at each step, engine-controlled fighters' policies, host-reported contact, when the fight ends, and fight-time MP regeneration | `realmkit-engine/src/realtime/`; spec `Realtime`, a profile's `ally`, `targeting` and `guard_hold` |
| **Mass battles** (strategic, separate) | Troop stacks, frontage, orders, morale, pursuit and autoresolve | `realmkit-engine/src/battle.rs`; spec `troops.rs` |

The core knows nothing of initiative, turns, ticks, frames or a host engine.
Either fight layer decides *when* someone acts and asks the core *what* the
action does: whether the skill can be paid for, what it costs, the damage, and
the Rage it earns. World time asks the core what elapsed minutes do. A mass
battle uses the player's own HP from the core, and troops' fixed stats; it is
not personal combat and is not a real-time system.

Field meanings are the core's, with each layer reading them in its own terms:

- `speed` is how quickly a character acts. The timeline turns it into the delay
  between turns; a duel divides each attack phase's length by it.
- A skill's `time` is the action's length relative to a basic attack. The
  timeline delays the user's next turn by it; a duel scales each of the
  attack's phases by it, as a worn weapon's `basic_time` does a basic attack's.
- `combat.resources.mp_regen_percent` belongs to the timeline alone: per
  baseline turn of encounter time. `combat.realtime.mp_regen_percent` is per
  second of duel time. World-time regeneration is `mp_recovery`.

## One owner per character's vitals

`Vitals { hp, mp, rage, rage_remainder }` are a character's current HP and
resources. Maxima are derived from effective stats and never saved.

- The player's vitals live in `CombatState::vitals`, whatever the stance.
  Exploring, an encounter, a duel and a battle all read and change that one copy.
- Every other fighter's vitals, opponent or real-time ally, live in its
  `Participant` (or real-time `Fighter`) for the length of the fight: they are
  whole again next time, as before. The player's participant or fighter
  holds `None` and the save check rejects anything else.
- Troops in a battle are fungible counts in stacks, never characters with
  vitals.

`Stance` (`exploring`, `fighting`, `realtime`, `battle`) only records how the player is
fighting now. A future named companion would get its own `Vitals` beside the
player's in the same way, not a copy inside a stance: its fighter would hold
`None` like the player's, read through the same accessor, and join on team
0 by the same `Join`. Today's allies are characters with an `ally` profile
whose vitals last one fight.

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

World time, encounter time and duel time are separate clocks and never
regenerate the same MP twice:

- An encounter, duel or battle never advances world time: travel, waiting and
  resting are refused during them.
- Encounter time exists only inside an encounter and duel time only inside a
  duel; the player is never in both. Each one's regeneration progress
  (`Participant::mp_remainder`, `Fighter::mp_remainder`) ends with it.
- World-time recovery and decay happen at fixed multiples of `every`, counted
  from minute 0. Crossing boundaries is additive, so a span split anywhere
  (one long wait, several short ones, a journey interrupted by scheduled
  events) crosses the same boundaries, and nothing needs saving between them.
  Boundaries at a minute apply before anything else scheduled then.

## Real-time duels

A world whose combat block has `realtime` runs real-time fights ("duels",
after the command that starts them) on a fixed-step clock inside the engine,
beside the turn-based layer. A fight starts as the player against one
opponent or a group, and can grow while it runs: hostile reinforcements,
friendly NPCs and third parties join without restarting it. Fighters attack
with basic attacks and skills in three phases, guard, and the player can
retreat. Package fields are in
[world-format.md](world-format.md#real-time-duels).

### Fighters and teams

`Stance::Realtime(RealtimeEncounter)` holds the clock (`step`) and the
fighters, in join order: the player first, then the opponents the fight
started with, then everyone who joined, as they did. Nobody is ever removed,
so indices and join order are stable for the whole fight and replays. Each
`Fighter` has:

- `character`, a stable character ID, in the fight at most once;
- `team`: 0 is the player's, the opponents it started with are 1, and a new
  team takes the next unused number. Fighters on different teams are
  hostile; on the same team, friends. There is no other diplomacy: a third
  team fights everyone else;
- `control`: `Player` for the player, whose intents drive them; `Policy` for
  everyone else, ally or foe, whom the engine drives by the same authored
  policy;
- `joined`, the step it arrived at; its `vitals` (`None` for the player,
  whose only copy stays in `CombatState::vitals`); MP regeneration progress;
  its committed action; and whether it yielded.

Every fighter runs on the same rules: the same phases, guard, damage,
Rage, MP and yielding, from the character core. Only the player wears gear
and has a retreat; only engine-controlled fighters follow a policy. Nothing
here is a player-only action model: the player's intents and every policy
call the same per-fighter steps (begin an attack, raise a guard, land a
blow).

### Lifecycle

1. **Start.** `Command::Duel(id)` starts a fight with a fighter here, not
   defeated and not an `ally`, and whoever engaging it would bring in: it
   alone, or every present, undefeated member of its group, all on team 1.
   `EncounterStarted` names them. The player brings their own vitals;
   everyone else's are full and belong to the fight. Rage that belongs to
   one fight starts from 0, as in an encounter.
2. **Play.** During a fight only `Command::Intent(_)` and panels run. Each
   intent applies at the fight's current step; `Intent::Advance(n)` runs the
   clock on.
3. **Join.** `Intent::Join { character, team }` is the host's report that a
   character arrives (see [Joining](#joining)).
4. **End.** The fight goes on while anyone off the player's team still
   fights. When the last of them falls or yields it is a `Victory`, settled
   in the command whose blow did it; the first foe to fall ends nothing. The
   player yielding is `Yielded` and a completed retreat `Fled`, whoever else
   still fights: allies leave with the player. The player's death leaves the
   fight open, as it does an encounter, and every later command but a panel
   is refused. This is deliberate: the player's death is the game's end, so
   allies still standing do not fight on to a victory.
5. **Settle once.** Turn-based encounters and real-time fights end in the
   same `settle` (crate `settle.rs`): the stance returns to `Exploring`
   first, then loot, XP, recorded defeats, defeat objectives, technique
   shares and group flags are granted, all inside the one command that ended
   the fight. A victory rewards every opponent (anyone off team 0) that fell,
   in join order, whoever struck the last blow; allies and yielded opponents
   give nothing. Each fallen opponent is recorded as defeated unless its own
   group is `repeatable`; flags and yielding come from the group the fight
   started with. With the stance gone, no later command can settle it again.

**Out of the fight.** A fighter other than the player who falls (0 HP) or
yields is out: whatever it was doing stops (`Ready`), it regenerates
nothing, its policy no longer acts, nobody can aim at it, and a contact by
or on it is refused. Its vitals stay in the fight until it ends. An ally who
falls reports `AllyDown`; a foe, `EnemyDefeated`. Engine-controlled fighters
never retreat in this slice.

**Reinforcements after the victory.** There is no "apparent" victory: the
fight ends in the very command whose blow leaves nobody off the player's
team fighting, and settles there. A join reported after that finds no fight
(`NotFighting`) and the newcomer has to be fought in a new one; nothing is
reopened or settled twice. A host that wants a reinforcement to arrive in
time must report the join before the step or contact that would end the
fight. An `Advance` stops at the step a fight ends, so steps it would have
run later never happen.

### Joining

`Intent::Join { character, team }` adds a fighter at the current step,
after every check passes; a refused join changes nothing:

- a fight is running (`NotFighting`); the character is not already in it,
  fallen or not, and is not the player (`AlreadyJoined`);
- it is placed here, present under its conditions, and has a combat profile
  (`NotHere`), and is not recorded as defeated (`AlreadyDefeated`);
- an `ally` joins only team 0; anyone else an existing opponent team or the
  next unused number, never team 0 (`InvalidTeam`).

The newcomer enters whole, `Ready`, under `Control::Policy`, and acts from
the next step. The clock, everyone else's actions, vitals, Rage and
regeneration progress are untouched. `Joined { character, team }` is
reported. Arrival is the host's report, like contact: the engine checks
eligibility, not how the character got there.

### Targets

Every attack is aimed when it begins and carries its `target`:

- **The player** names one with `Intent::Attack { target }` or
  `Intent::Skill { skill, target }`. It must be on another team and still
  fighting (`InvalidTarget`; a teammate is `NotHostile`). `None` aims at the
  only foe still fighting, and is refused when there are several
  (`NeedTarget`), so a strike never silently picks someone. Menus offer an
  untargeted strike when one foe stands, as before, and one strike per foe
  otherwise.
- **Engine-controlled fighters** aim each attack by their profile's
  `targeting`: `first` (the earliest in join order still fighting on
  another team, so the player while they fight) or `weakest` (the lowest
  current HP, ties to the earliest). The choice depends only on the state.

`Intent::Contact { attacker, target }` reports which fighter a blow
touched; `None` is the one it was aimed at. A host may report it touching
someone else, since only the host knows where everyone stood. The engine
accepts it only while the attacker's attack is active and has not landed,
on a fighter other than the attacker who is still fighting
(`InvalidTarget`), and on another team unless the world sets
`friendly_fire` (`NotHostile`). A blow on a teammate deals its damage and
the target's Rage for being hit, but earns the attacker no Rage for acting
and trains no technique. A blow aimed at someone who fell before it
landed lands on nobody unless the host reports another target.

### The clock

`RealtimeEncounter::step` counts steps of fight time from 0. Only `Advance`
moves it, 1 to `ADVANCE_BOUND` (10,000) steps per command; 0 or more is
refused, and a clock that would pass `u64::MAX` is `NumericLimit`. Each step
runs in one fixed order:

1. the clock moves on by one;
2. every fighter still fighting regenerates MP, in join order:
   `realtime.mp_regen_percent` of maximum MP per `steps_per_second` steps,
   through `resources::regenerate` with the progress kept in
   `Fighter::mp_remainder`;
3. each fighter's committed action moves on if it ends at this step, in join
   order; the player's retreat ending here ends the fight at once;
4. each engine-controlled fighter still fighting acts by its policy, in join
   order (below).

With two fighters this is exactly the one-on-one duel's order. An advance
stops at the step the fight ends. Because every step does the same thing
whatever command it came in, splitting an advance anywhere (one step per
host frame, or many) gives the same state and events. `steps_per_second`
tells hosts how fast to step to show the fight at its authored pace; the
engine never reads a clock.

**Pause and resume.** The fight clock stands still between commands. A host
pauses by sending no `Advance`: menus, a paused game, a minimised window or a
slow frame never move it. World time is frozen for the whole fight (travel,
waiting and resting are refused), so a fight of any length passes no world
minutes, regenerates nothing on world time and runs no world schedules.

### Actions and phases

A fighter is `Ready`, in an `Attack`, raising or holding a `Guard`, or in a
`Retreat`. Each timed state ends when the clock reaches its `until` step.

- **Attack** (`Intent::Attack`, or `Intent::Skill` for a known, unlocked
  skill): allowed only from `Ready` or `Guard` (it drops the guard). A skill's
  costs are checked and paid at once, MP first, through the core's
  `afford`/`spend`; a refused skill changes nothing. The attack then runs
  `Windup → Active → Recovery → Ready`. Phase lengths are the world's
  `windup`, `active` and `recovery` at speed 100 and a basic action's time,
  scaled to `max(1, ceil(length × time / speed))` steps by the attacker's
  effective speed and the skill's `time` (a worn weapon's `basic_time` for a
  basic attack).
- **Commitment.** During any attack phase the player can start nothing else:
  no other attack, guard or retreat (`Committed`). There is no cancel.
- **Guard** (`Intent::Guard`, only from `Ready`): stops hits from
  `guard_raise` steps later until it is lowered (`Intent::LowerGuard`) or an
  attack drops it. A hit landing on a raised guard deals `guard_percent` of its
  damage, rounded down; one landing while it is still rising is a full hit.
  The guard covers every direction: facing and guard arcs are spatial and not
  modelled yet.
- **Retreat** (`Intent::Retreat`, from `Ready` or `Guard`, refused by a
  `no_flee` group): the player is away after `retreat` steps if still fighting,
  and can still be hit meanwhile.
- **Policy.** An engine-controlled fighter, ally or foe, never retreats.
  Without a `guard_hold` it begins an attack whenever it is ready; with one,
  it raises its guard whenever it is ready and attacks once the guard has
  been up `guard_hold` steps. Each attack aims by its `targeting` and uses
  the strongest skill it can pay for, else a basic attack, chosen as in
  turn-based encounters.

A hit does not interrupt the target's own action in this slice: poise,
stagger and interruption are later work.

### Contact and its authority

Whether a blow physically touched is a spatial question, and the engine has
no geometry. The host reports it with `Intent::Contact { attacker, target }`:
"this fighter's attack touched that one now". The engine accepts a report
only when that fighter's attack is in its `Active` phase at the current step
and has not landed yet (else `NoContact`), on a target it may hit (see
[Targets](#targets)); anything else changes nothing. On an accepted report
the engine decides everything that follows:

- damage through `character::strike` from both sides' effective stats, the
  skill's power and channel, worn gear's modifiers on the player, and the
  target's guard, whoever the two fighters are;
- **no random crit and no accuracy roll:** a landed real-time hit deals the
  same damage from the same state, and duels draw nothing from any random
  stream;
- HP loss (stopping at 1 HP in a yielding group), Rage for being hit, Rage
  for acting credited to the attacker only because the attack landed, so
  swinging at the air earns nothing, technique XP for a landed skill, and the
  outcome. Hits the player deals are `DamageDealt`, hits on the player
  `DamageReceived`, and hits between two other fighters `Struck`.

What this bounds: a host can make an attack land at most once, only inside
its active window, only on a fighter still fighting and, without
`friendly_fire`, of another team, and never choose its damage, a kill, loot,
XP or victory. Joins are bounded the same way: a host can bring in only an
eligible character, once, on a team it may take. What it does not: the
engine cannot tell whether the reported contact was geometrically real,
which of several fighters in reach a blow really touched, nor whether a
host withheld a contact or an arrival that happened. A
dishonest or buggy host can therefore make every active window land or none.
This is an explicit trust boundary, not a verified hit. Replays are
deterministic for the same ordered intents including contact reports; they
are not proof that the host's physics produced those reports. Closing the gap
needs engine-side geometry or a validated spatial record (positions, reach
and hit volumes stepped on the same fixed clock), which is open work
(below).

### What a real-time fight reuses

| Rule | From |
| --- | --- |
| HP, MP, Rage | `CombatState::vitals` for the player; full vitals from the profile for everyone else, allies included |
| Effective stats, pools | `character::player_stats`, `resources::player_pools`, `profile_pools` |
| Skill affordability and payment | `resources::afford`, `resources::spend` |
| Rage from acting and from damage, cap | `resources::on_action`, `on_damage`, `at_cap` |
| Damage, worn modifiers, HP loss | `strike::damage_of` (with no crit), `strike::take` |
| MP regeneration | `resources::regenerate` on fight time |
| Skill choice for engine-controlled fighters | `encounter::choose`, as turn-based opponents |
| Yielding, `no_flee`, groups | the group rules turn-based encounters use |
| Victory rewards, flags, objectives | `settle::settle`, shared with encounters |

Nothing in a real-time fight is a second copy of the player. It adds no third resource:
no stamina, posture or poise.

### Saves mid-fight

A fight can be saved between any two commands, since every committed command
leaves it at a step boundary and its whole state (step, fighters in join
order with their teams and join steps, actions with their `until` steps and
targets, everyone's vitals but the player's, regeneration progress) is in the
snapshot. Loading checks that the rules could have left it:

- **Identities.** The player first, on team 0, player-controlled, with no
  vitals of their own in the fight; then fighters who are here, have a
  combat profile, are not defeated, appear once and are engine-controlled
  with vitals. The first of them is on team 1 at step 0, with every present
  member of its group beside it from step 0. Allies are only on team 0 and
  nobody else is; teams are numbered in the order they appeared, so none is
  skipped; join steps never go back and are not after now. The whole
  starting group is on team 1, and a fighter at the step it joined is still
  `Ready` with no regeneration progress.
- **Vitals and progress** within maxima, progress below one point; a
  yielded fighter at or below its threshold (the player may enter a fight
  already that low and has not yielded until hit again).
- **Actions.** Every committed action ends after now and no later than its
  phase allows (exactly, for engine-controlled fighters, whose stats never
  change; at the slowest speed for the player, whom a rank earned mid-fight
  can make faster or slower after a phase began). An attack is aimed at a
  fighter of another team and uses a known, unlocked skill, or an earlier
  rank's skill of a learned technique, since a committed attack keeps the
  skill it began with; no landed wind-up. An engine-controlled fighter still
  fighting is `Ready` only at the step it joined; one out of the fight is
  always `Ready`. A guard is the player's, or a policy fighter's with a
  `guard_hold` it has not yet held to the end; retreats only for the player,
  and only where flight is allowed.
- **Open.** The player and someone off their team still fight, or the
  player is dead.

A resumed fight continues exactly as an uninterrupted one would.

### Determinism

For the same package, seed and ordered commands (contact and join reports
included), real-time fights produce identical events and state, whatever the
host's frame rate or batching. Join order fixes every fighter's index, so
step order, `first` targeting and tie-breaks replay the same. A landed hit's
narrative variant follows the step it landed at, not the command count, so it
too is independent of batching. Refused intents change nothing and draw
nothing; fights draw nothing from any random stream. No wall clock, frame
delta or host physics callback is read.

### Godot and other hosts: what remains

- Spatial authority: positions, facing, reach and hit volumes on the fixed
  step, or a validated spatial record, so contact and arrivals can be
  checked rather than trusted.
- A host bridge: stepping at `steps_per_second`, sending intents between
  steps, rendering from `Engine::realtime()` and the events, interpolating
  between steps for presentation only.
- Engine-controlled intents beyond attacking and holding a guard: spacing,
  retreating, telegraphed patterns.
- Dodges, parries, redirection, interruption and poise, conditional crits,
  light and heavy handling (Gheasin Combat-02 to Combat-04).
- Persistent named companions: their world-level lifecycle (recruiting,
  following the player) and vitals kept between fights; consumables during
  a fight; host control of an ally.

## Not supported yet

- Verified real-time hit detection: duel contact is host-reported and only
  bounded by the engine (see [Contact and its authority](#contact-and-its-authority)).
- Persistent companions in real-time fights (allies' vitals last one
  fight); consumables, gear changes or dodges during a fight;
  engine-controlled fighters that space or retreat; formations;
  interruption, poise, parries and situational crits.
- Committing elapsed host time in batches outside travel, waiting and
  resting (see [host-driven passage of time](spatial-hosts-and-military-battles.md#host-driven-passage-of-time-future-requirement)).
- Named companions with persistent vitals, and allies in turn-based encounters.
- HP regeneration over world time.
- Resource events for recovery and decay: clients read the new vitals.
- Opponents carrying Rage or wounds between encounters; their vitals belong
  to one fight.
- A third fundamental resource.

## Compatibility

Existing packages need no change and keep their rules revision: every new
field (`realtime.friendly_fire`, a profile's `ally`, `targeting` and
`guard_hold`) is optional, absent by default and not serialized when absent.
Encounters, damage, RNG draws, progression and battles are unchanged. A
one-on-one duel plays exactly as before, step for step: every example
walkthrough produces byte-identical fights and save files. The one visible
difference is that a group member can now be dueled, so the practice yard's
menu also offers `Duel The Left Twin` and `Duel The Right Twin`. Turn-based
encounters settle through the same `settle` function as real-time fights;
recording each fallen opponent by its own group's `repeatable` is the same
rule there, since all of an encounter's opponents share one group.

Saves keep format 19, as the [format policy](world-format.md#versions)
allows: every save outside a real-time fight parses and loads exactly as
before. The real-time stance is now `realtime`, holding a list of fighters
with teams, so a save made in the middle of a one-on-one duel by an earlier
build (stance `dueling`) no longer parses and is refused, never
reinterpreted; finish the duel there, or start it again. A world that gains
or loses `realtime` changes its rules revision, so saves made before are
refused. The player's vitals stay in `combat.vitals`; a save in the earlier
layout that kept them in the stance is refused by name, with the advice to
start a new playthrough.
