#!/usr/bin/env python3
"""Balance simulator for the proposed M3 combat rules (ROADMAP.md, M3).

Models the proposal, not the engine: gradual defence with integer floor and a
minimum of 1, timeline delays ceil(action_cost / speed) after an opening delay,
ties by (time, side, participant order), and "attack the first living opponent"
for every side. Each fighter uses its strongest affordable skill every turn.
Skills spend MP (regenerates over encounter time, fully by resting) or rage
(starts at 0 each fight; builds from acting and from damage taken). HP recovers
only by resting.

    python3 scripts/combat_sim.py              # report, level-scaled K
    python3 scripts/combat_sim.py --k fixed    # report, K fixed at 100
    python3 scripts/combat_sim.py --check      # assert the balance targets

Every number here is a tuning candidate, not an agreed balance constant.
"""
from __future__ import annotations

import argparse
import math
import sys
from dataclasses import dataclass, replace
from enum import Enum

# ---------------------------------------------------------------- rules


class KMode(Enum):
    SCALED = "scaled"  # K grows with the attacker's level
    FIXED = "fixed"  # K stays at base_k


@dataclass(frozen=True)
class Rules:
    """World-level combat constants and the formulas that use them."""

    action_cost: int = 100_000
    speed_cap: int = 200
    growth: float = 1.10  # every stat grows 10% per level, players and monsters alike
    base_k: int = 100
    k_mode: KMode = KMode.SCALED
    mp_regen_percent: int = 3  # % of max MP regained per baseline turn of encounter time
    rage_per_action: int = 1
    rage_per_max_hp: int = 20  # rage gained from taking damage equal to max HP

    @property
    def baseline_turn(self) -> int:
        """Encounter time of one basic action at speed 100."""
        return self.action_cost // 100

    def grow(self, value: float, level: int) -> int:
        return round(value * self.growth ** (level - 1))

    def k_for(self, level: int) -> int:
        return self.base_k if self.k_mode is KMode.FIXED else self.grow(self.base_k, level)

    def damage(self, attack: int, power: int, defence: int, attacker_level: int) -> int:
        """Gradual defence: defence equal to K halves damage. Rounds once, minimum 1."""
        k = self.k_for(attacker_level)
        return max(1, (attack * power * k) // (100 * (k + defence)))

    def skill_cost(self, skill: Skill, level: int) -> int:
        """MP costs grow with the user's level like the MP pool, so casts per rest stay level."""
        return self.grow(skill.cost, level) if skill.resource is Resource.MP else skill.cost

    def delay(self, speed: int, time: int = 100) -> int:
        """Recovery before the actor's next turn; `time` is the action's cost in percent."""
        return math.ceil(self.action_cost * time // 100 / min(speed, self.speed_cap))


class Resource(Enum):
    MP = "mp"  # recovers only by resting: limits fights between rests
    RAGE = "rage"  # starts at 0 every fight; builds from acting and from damage taken


@dataclass(frozen=True)
class Skill:
    name: str
    power: int  # percent; a basic attack is 100
    magical: bool
    cost: int = 0  # level-1 cost for MP; flat for rage
    resource: Resource = Resource.MP
    time: int = 100  # action cost in percent of a basic attack; delays the next turn


BASIC_ATTACK = Skill("attack", power=100, magical=False)
BOLT = Skill("bolt", power=170, magical=True, cost=12)
SPARK = Skill("spark", power=80, magical=True)  # free, so an empty MP pool is not helpless
RAGE_STRIKE = Skill("rage strike", power=175, magical=False, cost=5, resource=Resource.RAGE)

# ---------------------------------------------------------------- characters


@dataclass(frozen=True)
class Combatant:
    """A character's combat stats at one level."""

    name: str
    level: int
    hp: int
    mp: int
    patk: int
    pdef: int
    matk: int
    mdef: int
    speed: int
    skills: tuple[Skill, ...]


@dataclass(frozen=True)
class Profile:
    """Level-1 stats. Floats are allowed because monster kinds scale them."""

    name: str
    hp: float
    mp: float
    patk: float
    pdef: float
    matk: float
    mdef: float
    speed: int
    skills: tuple[Skill, ...] = ()

    def at_level(self, rules: Rules, level: int, speed: int | None = None) -> Combatant:
        return Combatant(
            name=self.name,
            level=level,
            hp=rules.grow(self.hp, level),
            mp=rules.grow(self.mp, level),
            patk=rules.grow(self.patk, level),
            pdef=rules.grow(self.pdef, level),
            matk=rules.grow(self.matk, level),
            mdef=rules.grow(self.mdef, level),
            speed=self.speed if speed is None else speed,  # speed does not grow
            skills=self.skills,
        )


class Kind(Enum):
    """Monster tiers as (HP multiplier, attack multiplier)."""

    NORMAL = (1.0, 1.0)
    MINION = (0.5, 0.6)
    BOSS = (4.0, 1.3)

    def __init__(self, hp_multiplier: float, attack_multiplier: float) -> None:
        self.hp_multiplier = hp_multiplier
        self.attack_multiplier = attack_multiplier


WARRIOR = Profile("warrior", hp=200, mp=0, patk=20, pdef=15, matk=5, mdef=10, speed=100, skills=(RAGE_STRIKE,))
MAGE = Profile("mage", hp=160, mp=60, patk=8, pdef=8, matk=20, mdef=15, speed=100, skills=(BOLT, SPARK))
MONSTER = Profile("monster", hp=80, mp=0, patk=12, pdef=10, matk=0, mdef=5, speed=110)
BUILDS = (WARRIOR, MAGE)

# ---------------------------------------------------------------- encounters


@dataclass
class Fighter:
    """A combatant's mutable state inside one encounter."""

    combatant: Combatant
    side: int  # 0 = the player's side
    order: int  # position in the encounter; breaks ties
    hp: int
    mp: int
    next_time: int
    rage: int = 0
    mp_remainder: int = 0  # regeneration carried between ticks so rounding loses nothing

    def regenerate(self, rules: Rules, elapsed: int) -> None:
        """MP regained over encounter time; speed does not change the rate."""
        self.mp_remainder += self.combatant.mp * rules.mp_regen_percent * elapsed
        gained, self.mp_remainder = divmod(self.mp_remainder, 100 * rules.baseline_turn)
        self.mp = min(self.combatant.mp, self.mp + gained)

    @property
    def alive(self) -> bool:
        return self.hp > 0

    def can_afford(self, skill: Skill, cost: int) -> bool:
        return (self.mp if skill.resource is Resource.MP else self.rage) >= cost

    def choose_skill(self, rules: Rules) -> Skill:
        """The strongest affordable skill, falling back to a basic attack."""
        level = self.combatant.level
        affordable = [s for s in self.combatant.skills if self.can_afford(s, rules.skill_cost(s, level))]
        return max(affordable, key=lambda s: s.power, default=BASIC_ATTACK)

    def pay(self, skill: Skill, cost: int) -> None:
        if skill.resource is Resource.MP:
            self.mp -= cost
        else:
            self.rage -= cost


@dataclass(frozen=True)
class FightResult:
    won: bool
    actions: int  # actions the player took
    hp: int  # player HP left
    mp: int  # player MP left


class Encounter:
    """One player against enemies on the paused initiative timeline."""

    def __init__(self, rules: Rules, player: Combatant, enemies: list[Combatant],
                 hp: int | None = None, mp: int | None = None) -> None:
        self.rules = rules
        self.fighters = [
            Fighter(c, side=0 if i == 0 else 1, order=i, hp=c.hp, mp=c.mp, next_time=rules.delay(c.speed))
            for i, c in enumerate([player, *enemies])
        ]
        self.player = self.fighters[0]
        if hp is not None:
            self.player.hp = hp
        if mp is not None:
            self.player.mp = mp
        self.player_actions = 0
        self.now = 0

    def run(self) -> FightResult:
        while self.player.alive and any(f.alive and f.side != 0 for f in self.fighters):
            actor = self._next_actor()
            for fighter in self.fighters:
                fighter.regenerate(self.rules, actor.next_time - self.now)
            self.now = actor.next_time
            self._act(actor)
        return FightResult(self.player.alive, self.player_actions, self.player.hp, self.player.mp)

    def _next_actor(self) -> Fighter:
        return min((f for f in self.fighters if f.alive), key=lambda f: (f.next_time, f.side, f.order))

    def _act(self, actor: Fighter) -> None:
        target = next(f for f in self.fighters if f.alive and f.side != actor.side)
        attacker, defender = actor.combatant, target.combatant
        skill = actor.choose_skill(self.rules)
        actor.pay(skill, self.rules.skill_cost(skill, attacker.level))
        attack, defence = (attacker.matk, defender.mdef) if skill.magical else (attacker.patk, defender.pdef)
        dealt = self.rules.damage(attack, skill.power, defence, attacker.level)
        target.hp -= min(dealt, target.hp)
        actor.rage += self.rules.rage_per_action
        target.rage += self.rules.rage_per_max_hp * dealt // defender.hp
        if actor is self.player:
            self.player_actions += 1
        actor.next_time += self.rules.delay(attacker.speed, skill.time)

# ---------------------------------------------------------------- experience


@dataclass(frozen=True)
class XpRules:
    decay: bool = True  # scale XP by level difference; nothing from monsters 5+ levels below

    @staticmethod
    def to_next(level: int) -> int:
        return 100 * level

    def for_kill(self, player_level: int, monster_level: int) -> int:
        """Base 15 per monster level + 5, then ±10% per level of difference (capped at ±4)."""
        base = 15 * monster_level + 5
        if not self.decay:
            return base
        diff = monster_level - player_level
        return 0 if diff <= -5 else base * (10 + max(-4, min(4, diff))) // 10


@dataclass(frozen=True)
class GrindResult:
    level: int
    kills: int
    rests: int

    def __str__(self) -> str:
        return f"L{self.level}, {self.kills} kills, {self.rests} rests"

# ---------------------------------------------------------------- simulator


@dataclass(frozen=True)
class Outcome:
    actions: int
    hp_lost: int  # percent of maximum HP

    def __str__(self) -> str:
        return f"{self.actions:>2}a/{self.hp_lost:>3}%"


class Simulator:
    def __init__(self, rules: Rules) -> None:
        self.rules = rules

    def player(self, build: Profile, level: int, speed: int | None = None) -> Combatant:
        return build.at_level(self.rules, level, speed)

    def monster(self, level: int, kind: Kind = Kind.NORMAL) -> Combatant:
        scaled = replace(MONSTER, name=kind.name.lower(), hp=MONSTER.hp * kind.hp_multiplier,
                         patk=MONSTER.patk * kind.attack_multiplier)
        return scaled.at_level(self.rules, level)

    def fight(self, player: Combatant, enemies: list[Combatant],
              hp: int | None = None, mp: int | None = None) -> FightResult:
        return Encounter(self.rules, player, enemies, hp, mp).run()

    def outcome(self, build: Profile, level: int, enemies: list[Combatant]) -> Outcome | None:
        """Actions and HP lost on a win from full health; None on a loss."""
        player = self.player(build, level)
        result = self.fight(player, enemies)
        return Outcome(result.actions, hp_lost_percent(player, result.hp)) if result.won else None

    def fights_per_rest(self, build: Profile, level: int, monster_level: int) -> int:
        """Consecutive same-kind fights won from full HP/MP before a loss (at most 50)."""
        player = self.player(build, level)
        hp, mp = player.hp, player.mp
        for won_so_far in range(50):
            result = self.fight(player, [self.monster(monster_level)], hp, mp)
            if not result.won:
                return won_so_far
            hp, mp = result.hp, result.mp
        return 50

    def boss_level_needed(self, build: Profile, boss_level: int) -> int | None:
        boss = self.monster(boss_level, Kind.BOSS)
        return next((p for p in range(1, 60) if self.fight(self.player(build, p), [boss]).won), None)

    def grind(self, build: Profile, target: int, xp: XpRules = XpRules(),
              farm: int | None = None) -> GrindResult:
        """Grind from level 1 to `target`. Fights the highest monster costing <= 35% HP
        (or always `farm`) and rests when the next fight would be lost."""
        level, earned, kills, rests = 1, 0, 0, 0
        while level < target:
            player = self.player(build, level)
            if farm is not None:
                monster_level = farm
            else:
                safe = [m for m in range(1, 60)
                        if (o := self.outcome(build, level, [self.monster(m)])) and o.hp_lost <= 35]
                if not safe:
                    break
                monster_level = max(safe)
            gain = xp.for_kill(level, monster_level)
            if gain == 0:
                break
            hp, mp = player.hp, player.mp
            while earned < xp.to_next(level):
                result = self.fight(player, [self.monster(monster_level)], hp, mp)
                if not result.won:
                    rests += 1
                    result = self.fight(player, [self.monster(monster_level)])
                hp, mp = result.hp, result.mp
                kills += 1
                earned += gain
            earned -= xp.to_next(level)
            level += 1
        return GrindResult(level, kills, rests)


def hp_lost_percent(player: Combatant, hp: int) -> int:
    return 100 - 100 * hp // player.hp

# ---------------------------------------------------------------- report and checks


def show(outcome: Outcome | None) -> str:
    return str(outcome) if outcome else "   LOSS  "


def report(sim: Simulator) -> None:
    print(f"K mode: {sim.rules.k_mode.value}; actions/HP lost per fight")
    for build in BUILDS:
        print(f"\n{build.name} vs monster level offset" + " " * 18 + "fights/rest")
        print("  lvl " + "".join(f"{d:>+9} " for d in (-3, 0, 1, 2, 4)))
        for p in (1, 3, 5, 10, 20, 30):
            row = " ".join(show(sim.outcome(build, p, [sim.monster(p + d)])) if p + d >= 1 else "      -  "
                           for d in (-3, 0, 1, 2, 4))
            print(f"  {p:<3} {row}   {sim.fights_per_rest(build, p, p)}")

    print("\nspeed: warrior L5, HP lost vs normal / boss L5")
    for speed in (80, 100, 110, 120, 140, 160, 200):
        player = sim.player(WARRIOR, 5, speed)
        cells = []
        for kind in (Kind.NORMAL, Kind.BOSS):
            result = sim.fight(player, [sim.monster(5, kind)])
            cells.append(f"{hp_lost_percent(player, result.hp)}%" if result.won else "LOSS")
        print(f"  speed {speed:>3}: {cells[0]:>5} / {cells[1]:>5}")

    print("\npacks at player L5, HP lost")
    for build in BUILDS:
        packs = [f"{n}x {kind.name.lower()}: {show(sim.outcome(build, 5, [sim.monster(5, kind)] * n)).strip()}"
                 for kind, n in ((Kind.NORMAL, 2), (Kind.MINION, 2), (Kind.MINION, 3), (Kind.MINION, 4))]
        print(f"  {build.name:<7} " + " | ".join(packs))

    print("\nlowest player level that beats a level-B boss")
    for b in (3, 5, 10, 20):
        print(f"  B={b:<3} warrior L{sim.boss_level_needed(WARRIOR, b)}, mage L{sim.boss_level_needed(MAGE, b)}")

    print("\ngrinding from L1 to L10")
    for xp in (XpRules(decay=True), XpRules(decay=False)):
        for build in BUILDS:
            print(f"  decay {'on ' if xp.decay else 'off'} {build.name:<7} "
                  f"best monster: {sim.grind(build, 10, xp)} | farm L1 only: {sim.grind(build, 10, xp, farm=1)}")


SAME_LEVEL_ACTIONS = (3, 6)  # player actions to beat a same-level ordinary monster


def check(sim: Simulator) -> None:
    """The balance targets as assertions; each failure names the broken target."""
    # Roadmap damage examples: attack 40, power 100, K = 100 (level 1).
    assert [sim.rules.damage(40, 100, d, 1) for d in (0, 20, 100, 300)] == [40, 33, 20, 10]
    for build in BUILDS:
        for p in (1, 5, 10, 20, 30):
            same = sim.outcome(build, p, [sim.monster(p)])
            assert same and SAME_LEVEL_ACTIONS[0] <= same.actions <= SAME_LEVEL_ACTIONS[1] and 15 <= same.hp_lost <= 35, \
                f"{build.name} L{p} same-level: {same}"
            harder = sim.outcome(build, p, [sim.monster(p + 4)])
            assert harder is None or harder.hp_lost >= 40, f"{build.name} L{p} +4 levels too easy: {harder}"
            assert 2 <= sim.fights_per_rest(build, p, p) <= 5, f"{build.name} L{p} fights per rest"
            assert sim.outcome(build, p, [sim.monster(p, Kind.MINION)] * 2), f"{build.name} L{p} loses to 2 minions"
        for b in (5, 10, 20):
            need = sim.boss_level_needed(build, b)
            assert need is not None and b <= need <= b + 3, f"{build.name} L{b} boss needs L{need}"
        assert sim.grind(build, 10).level == 10, f"{build.name} cannot grind to L10"
        assert sim.grind(build, 10, farm=1).level < 10, f"{build.name} can farm L1 monsters to L10"
    # Build identity: the mage bursts, the warrior sustains; neither dominates.
    for p in (5, 10, 20):
        w, m = sim.outcome(WARRIOR, p, [sim.monster(p)]), sim.outcome(MAGE, p, [sim.monster(p)])
        assert w and m and m.actions < w.actions, f"L{p}: mage should finish faster (mage {m}, warrior {w})"
        assert sim.fights_per_rest(WARRIOR, p, p) > sim.fights_per_rest(MAGE, p, p), \
            f"L{p}: warrior should fight longer between rests"
    for b in (5, 10, 20):
        needs = [sim.boss_level_needed(build, b) or 99 for build in BUILDS]
        assert max(needs) - min(needs) <= 1, f"L{b} boss: builds need levels {needs}"
    boss = sim.monster(5, Kind.BOSS)
    boss_hp = [sim.fight(sim.player(WARRIOR, 5, s), [boss]).hp for s in (100, 160, 200)]
    assert boss_hp[0] < boss_hp[1] <= boss_hp[2], f"speed gives no boss-fight benefit: {boss_hp}"
    print("all balance targets hold")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--k", choices=[m.value for m in KMode], default=KMode.SCALED.value)
    parser.add_argument("--check", action="store_true", help="assert balance targets")
    args = parser.parse_args()
    sim = Simulator(Rules(k_mode=KMode(args.k)))
    try:
        check(sim) if args.check else report(sim)
    except AssertionError as error:
        sys.exit(f"balance target failed: {error}")


if __name__ == "__main__":
    main()
