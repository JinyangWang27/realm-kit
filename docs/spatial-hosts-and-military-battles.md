# Spatial hosts, stationed forces, and battle boundaries

Status: **design proposal, not implemented**. This note describes reusable engine seams for an optional, continuously explorable 3D host and for a military sandbox with persistent stationed and moving troops. It does **not** amend the package/save format, add runtime features, or select a rendering engine.

A concrete motivating consumer is [Gheasin's first-person world and battle design (PR #22)](https://github.com/JinyangWang27/gheasin/pull/22). The consumer's **three ordinary escort guards** are a *game-specific* cap, **not** a RealmKit-wide limit; the cap for named companions is separately undecided. This document is engine-facing and applies equally to a text, 2D, 3D, CLI, or headless host.

See [Roadmap M6](../ROADMAP.md#m6--living-sandbox-worlds--proposed), [Living sandbox worlds](sandbox-worlds.md), [Glossary](glossary.md), and [Overland map](sandbox-worlds.md#overland-map). As those references indicate, the existing engine delivers initial troop stacks, recruitment, upgrades, wounded recovery, monthly payroll, and discrete mass battles; companions, holdings/garrisons, traveling world agents, and autonomous campaigns are not yet fully delivered.

## Invariants and ownership

1. **One canonical playthrough.** The engine owns persistent game facts and validates committed commands, including parties, troop custody, location, WorldTime, permissions, battles, economy, standing, quests, and saves. A host renders and collects player intent; it does not maintain another authoritative troop list or invent results.
2. **Presentation is optional.** No 3D engine, scene format, physics simulator, map tile, or renderer is a RealmKit dependency. Worlds without military capabilities or a spatial host retain their current behavior and do not gain empty state.
3. **Logical geography stays authoritative.** A host may render continuous terrain, but only the engine's authored locations/routes and their conditions authorize movement, strategic travel, interactions, and revelations. An area or spatial layout is a *projection*, not another traversal graph.
4. **Stable identity and deterministic transitions.** Transfers, travel, orders, battle entry, resolution, recovery, and rejection are atomic where they affect canonical state. Given the same package, seed and committed commands, gameplay results and snapshots agree across hosts; mere rendering or inspection cannot advance time or RNG.
5. **Authority is not ownership.** Private troops and institutionally controlled forces must remain distinguishable. Losing an office can revoke a right to command public forces without deleting the player's own roster.

## A presentation host over logical locations

RealmKit remains a deterministic command/event engine. A first-person client may interpolate a character's camera and local transforms many times per second. These are **host presentation details**, not engine ticks or implicit world travel.

An authored client-side manifest may associate an engine location/Area with a world-space region, environment assets, stable interaction anchors, travel portals, and a battle-site visualization. It should reference stable RealmKit IDs, not reinterpret or bypass locked routes. Any eventual engine-side spatial metadata should be added only when a concrete rule or query cannot be expressed using the existing IDs, Areas, spatial layouts, locations, and traversal graph.

For transition-sensitive interactions, the host resolves an input into an offered action or a future validated typed command. If a 3D collision proposes entrance into a restricted place, the engine can refuse it; the visible scene must then respect the refusal. Physical movement within a locally loaded scene need not produce a RealmKit command for every step. Logical location changes and rule-significant discoveries do.

### Host-driven passage of time (future requirement)

WorldTime remains the **only** in-world time source. Existing travel, wait and rest commands already advance it. A continuous host needs a *proposed* bounded, explicit way to commit elapsed in-world time in batches when the player walks or acts locally. Such a command must:

- use integer gameplay duration and a validated reason/context, **never** directly read a wall clock in the engine;
- run all due world schedules in canonical order and perform the same quest, payroll, healing, movement, and agent consequences as other time advancement;
- refuse zero, invalid, overflow, or unauthorized time jumps without partially committing;
- keep viewing menus, panning a camera, hovering over an NPC, and other non-gameplay actions time- and RNG-neutral;
- support replay/snapshot equivalence using the **same committed time commands**, regardless of render frame rate.

The host's sampling rate and game-minutes-per-real-second policy are not decided here. Paused menus, instanced battles and background progression require an explicit policy before implementation, so they cannot create two diverging clocks.

## Military custody and station access (M6c / M6e pressure)

An engine should distinguish the concepts below, without introducing all of them into every world:

| Concept | Proposed semantic responsibility |
| --- | --- |
| **Adventuring party / personal escort** | The player-controlled nearby roster or companions. A game's authored limit may restrict how many ordinary guards are physically deployed, independently from how many troops it owns elsewhere. |
| **Military station** | A location offering authorized hosting, recruitment, training, recovery, assembly, or dispatch. A station need not be a player-owned holding; barracks and mercenary facilities may be rented or institutionally operated. |
| **Stationed force / garrison** | A troop roster held at a location, with an owner, controller, station and applicable upkeep obligations. Existing holding garrisons remain one subtype/use case. |
| **Detached / field force** | A force with its own stable identity and roster, orders, route progress and destination; simulated on the logical road graph when off-screen. It is **not** a permanently rendered 3D army. |
| **Command authority** | Permission to give an order to a force, possibly granted by faction membership, office, contract, station access or ownership. It is not itself a roster or a claim of ownership. |

Keep at least **ownership**, **legal command authorization**, **current location/custodian**, **roster identity**, and, when politically relevant, **controlling institution** conceptually separate. Do not presume that stations require holdings, that every force is owned by the player, or that all garrisons draw wages from the same purse. A station's access rules, costs, capacity, and available services are authored, independently of its visual model.

### Transfer and roster conservation

The existing M6c roster stores counts, wounded counts and shared experience by troop line/level; fungible ordinary soldiers need not become one persistent 3D character instance each. A future transfer between personal escort, station, and field force should preserve the existing squad's level, wounds, carried XP share and relevant equipment/ownership accounting.

A proposed transfer must validate source custody, authorization, available healthy/wounded troops, capacity, destination, and any required co-location before committing. If rejected, **nothing** moves. If accepted, the same soldiers leave the source and enter the destination once; no temporary duplication while a force travels or enters a battle. XP splitting/merging must be deterministic, conserving the pre-transfer pool subject only to the already documented rounding/remainder conventions. No free level gains or erased veteran experience. Named companions remain unique characters with stable IDs, not fungible troop stacks.

The existing player-retinue payroll rules must remain unchanged for worlds without distributed forces. When stations and independent forces exist, any extension must specify the **payer and payable roster exactly once** per due settlement, alongside the different physical upkeep schedules. “Stationed” does not mean wages or food cease to exist, and an institutional army does not automatically spend the player's private account.

### Order, travel, and information

An authorized order can be issued from a remote management UI, but **acceptance is not instant relocation**. The engine stores an order and simulates route progress, duration, interruptions, arrival and world-time consequences through its deterministic road/agent mechanisms. An order to a force the player cannot command is rejected, even if the UI can display that force. Local physical recruitment or escort handoff requires co-location or an explicitly authorized local agent; remote orders never teleport ordinary troops into the player's party.

Where knowledge/news capabilities exist, queries should expose only legitimately known positions and reports, not live omniscience over the world. The presentation client can show an estimated arrival without making such estimates authoritative. A messenger-latency subsystem and dedicated signal traffic are optional future additions, not baseline requirements.

## Battle sites and isolated high-detail battle instances

A **battle site** is an authored *reference to a real world location/region and its strategic context*. A 3D host may visualize it as a bounded subset of the surrounding terrain, complete with deployment, exit, navigation, siege overlays, and collision data. **These assets and continuous coordinates belong to the game/host**, unless an engine rule later demonstrates a need for typed spatial fields.

An **active battle** is a canonical engine-owned conflict over stable participant/force IDs. A **battle rendering instance** is the host's temporary high-detail presentation of that conflict. They are not different worlds with independent economies, rosters, time, or saves. Existing discrete mass battle remains valid, including CLI and off-screen autoresolve; a real-time first-person implementation would require a separately specified, authoritative combat and reconciliation contract. Personal combat already separates the universal character core from the turn-based layer, so such a contract builds on the same vitals and rules ([Combat architecture](combat.md#a-future-real-time-layer)).

### Proposed lifecycle and guarantees

1. **Locate and authorize.** Select a legal battle site in the world, determine attackers/defenders and eligible participant IDs, and validate the player's presence and authority if joining personally.
2. **Reserve participants.** Mark/represent each committed participant as engaged **exactly once** using the existing battle stance where sufficient, or the minimum explicit reservation state justified by the first multi-force fixture. An engaged force cannot simultaneously be transferred, participate in a second battle, or be mutated by an incompatible action.
3. **Present a subregion.** The host loads the relevant bounded actual-world terrain and local entities. Visual loading is not an independent gameplay transition and may be retried without new troop creation.
4. **Execute the authoritative conflict.** Discrete RealmKit battles resolve by its existing rules. A future real-time host must submit validated inputs or an approved deterministic event record; it must not simply send an arbitrary client-declared victory, casualty list, or loot result.
5. **Settle once.** A single canonical resolution applies casualties (healthy/wounded/dead), XP, rewards, ownership/control effects, reputation and any authored world consequences once, with an idempotent/replay-safe guard. The already-existing battle rewards must not be duplicated by a return-to-world transition.
6. **Release and return.** Survivors re-enter their legal custodians/forces, and the player resumes at an authored valid location/exit. Defeat, retreat, aborted rendering and reload must have explicit, valid paths; no teleportation, cloned garrisons, orphaned participants or lost rewards.

A live battle need not literally store per-soldier engine-side transforms; presentation and canonical combat authority should remain separate. Conversely, *if* first-person hit registration becomes gameplay-authoritative, it requires a precise fixed-step/determinism or validated-input design, not reliance on unproven deterministic behavior of host physics.

Active battle and off-screen world schedules need a clear time/pause interaction: especially payroll, reinforcements, wound recovery and simultaneous battles. Save/restore must either preserve a valid active conflict and its deterministic replay data **or** reject saves at unsafe intermediate boundaries with an explicit host policy. The present save format must **not** change merely because this design is recorded.

## Optional capabilities and delivery slices

This is an architectural pressure test, not a direction to implement an all-in-one “3D mode” in RealmKit.

| Slice | Representative rule fixture / acceptance condition | Dependency pressure |
| --- | --- | --- |
| A. Escort and custody | Recruit 10 troops, assign an authored small subset to the nearby party and the rest to a permitted station; save/restore and reconcile count/XP/wounds | Extend M6c only when first fixture needs distinct custody |
| B. Station and remote order | Move a permitted subset to another station through saved travel progress; reject unauthorized, over-capacity and non-co-located instant transfers | M6e station/holding access; M6f moving party state |
| C. Time-driven world | Commit bounded walking-time increments; schedules, payroll and agents advance deterministically, while camera/UI inspection does not | Optional world-time host command; no engine wall clock |
| D. Battle boundary | Reserve two participating forces at an authored road/holding; resolve once; unload/reload a purely visual battle scene without changing results or duplicating forces | Existing M6d as default; future 3D adapter contract only if needed |
| E. Living-world integration | The player investigates elsewhere as an authorized detached army marches and a holding changes hands, with knowledge-filtered reports | M6f/M6g only when an authored scenario needs them |

For every slice, test rejected commands leave state and RNG unchanged, cross-host state replay yields identical outputs, snapshots restore a valid ownership partition, and worlds omitting the new capability remain valid and unchanged. No need to introduce a universal ECS, real-time engine tick, generic 3D terrain API, new troops-as-characters representation, or AI decision-maker.

## Decisions intentionally left open

- Whether force custody attaches to a generic holder abstraction or builds incrementally from M6c player roster, M6e holding garrisons and M6f world-agent rosters.
- Permission and upkeep schema for non-owned/rented stations and institutionally funded forces.
- Whether escort capacity is counted per troop, companion, mount, or another authored rule; the motivating game's three-ordinary-guard limit is not an engine constant.
- The allowed host-time increment size, temporal scale, pause policy and interaction with discrete travel.
- The authoritative contract for live 3D combat, including sampling, input verification, consistency with mass-battle rules and saved mid-battle state.
- Which battle metadata belongs in reusable RealmKit definitions (logical location, modifiers, combat participants) versus external host manifests (terrain polygons, models, spawn anchors).
- How host-local coordinates are stored or reconstructed without conflicting with authoritative logical location, including backwards compatibility.

**Decision gate:** settle each point only when a representative authored fixture and client demonstrate a necessary rule. Keep the existing CLI, package and save behavior stable until then.
