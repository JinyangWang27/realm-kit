"""The M6d mass-battle round rule, mirrored from the engine's battle module.

Reads a package's `troops` and `battle` blocks and an army, then fights it:

    python3 -m scripts.combat_sim battle examples/marches --army outlaws \\
        --roster levy:1:6 --roster bowmen:1:2 --orders hold,charge --seed 7

Side 0 is the player (level-table stats at `--level`, no gear) and the
roster, then any `--ally` armies; side 1 is the army. Orders are given round
by round, and once they run out every later round charges. Engine tests pin
numbers this module prints, so a rule change here or there must change both.
"""

from __future__ import annotations

import argparse
import json
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

from .economy import MASK, splitmix64
from .model import ratio_damage

DOMAIN_BATTLE = 0x6261_7474_6C65  # "battle" in ASCII, as in the engine's rng.rs
ORDERS = ("charge", "hold", "flank", "retreat")


@dataclass
class Stream:
    """The engine's `battle` random stream."""

    state: int

    def below(self, n: int) -> int:
        self.state, value = splitmix64(self.state)
        return (value * n) >> 64


@dataclass
class Unit:
    """A stack of one line at one level, or the player as a stack of one."""

    name: str
    stats: dict[str, int]
    count: int
    cls: str | None
    ranged: bool
    mounted: bool
    channel: str
    player: bool = False
    hp: int = 0  # the player's current HP
    remainder: int = 0

    def attack(self) -> int:
        return self.stats["patk"] if self.channel == "physical" else self.stats["satk"]


@dataclass
class Side:
    units: list[Unit]
    morale: int = 100
    morale_remainder: int = 0
    start_size: int = field(init=False)

    def __post_init__(self) -> None:
        self.start_size = sum(u.count for u in self.units)

    def standing(self) -> bool:
        return any(u.count > 0 for u in self.units)


def damage(rules: dict[str, Any], attacker: Unit, defender: Unit) -> int:
    """The personal damage formula at power 100 in the attacker's channel."""
    share: int = rules["combat"].get("cross_share", 25)
    a_stats, d_stats = attacker.stats, defender.stats
    if attacker.channel == "physical":
        a = 100 * a_stats["patk"] + share * a_stats["satk"]
        d = 100 * d_stats["pdef"] + share * d_stats["sdef"]
    else:
        a = 100 * a_stats["satk"] + share * a_stats["patk"]
        d = 100 * d_stats["sdef"] + share * d_stats["pdef"]
    return ratio_damage(a, d, 100)


def split(n: int, counts: list[int]) -> list[int]:
    """n attackers across targets in proportion to their counts: floors, then
    the leftover one each to the largest remainders, ties in target order."""
    total = sum(counts)
    shares = [n * c // total for c in counts]
    left = n - sum(shares)
    order = sorted(range(len(counts)), key=lambda i: (-(n * counts[i] % total), i))
    for i in order[:left]:
        shares[i] += 1
    return shares


class Battle:
    def __init__(self, world: dict[str, Any], sides: list[Side], seed: int) -> None:
        self.world = world
        self.rules: dict[str, Any] = world["battle"]
        self.sides = sides
        self.stream = Stream(seed ^ DOMAIN_BATTLE)
        self.round = 0
        self.log: list[str] = []

    def morale_mod(self, side: Side) -> int:
        morale = self.rules.get("morale")
        if morale is None:
            return 100
        floor: int = morale["floor"]
        return floor + (100 - floor) * side.morale // 100

    def matchup(self, attacker: Unit, target: Unit) -> int:
        if attacker.cls is None or target.cls is None:
            return 100
        value: int = self.rules.get("matchups", {}).get(attacker.cls, {}).get(target.cls, 100)
        return value

    def roll(self) -> int:
        lo, hi = self.rules["roll"]
        return int(lo + self.stream.below(hi - lo + 1))

    def fighters(self, s: int, order: str) -> list[tuple[int, int, bool]]:
        """(unit, fighting count, flanking) for side s under its order."""
        own, enemy = self.sides[s], self.sides[1 - s]
        flanking = order == "flank" and any(u.ranged and u.count > 0 for u in enemy.units)
        room = self.rules["frontage"]
        out = []
        for i, unit in enumerate(own.units):
            if unit.count == 0:
                continue
            if unit.ranged:
                out.append((i, unit.count, False))
            elif flanking and unit.mounted:
                out.append((i, unit.count, True))
            else:
                take = min(room, unit.count)
                room -= take
                if take:
                    out.append((i, take, False))
        return out

    def exposed(self, side: Side, flank: bool) -> list[int]:
        ranged = [i for i, u in enumerate(side.units) if u.ranged and u.count > 0]
        if flank:
            return ranged
        melee = [i for i, u in enumerate(side.units) if not u.ranged and u.count > 0]
        return melee or ranged

    def strike(self, s: int, order: str, holding: bool, pursuit: bool, dealt: dict[int, int]) -> None:
        """Side s's damage on the other side, added into `dealt` by target unit."""
        own, enemy = self.sides[s], self.sides[1 - s]
        roll = self.roll()
        mod = self.morale_mod(own)
        for i, n, flank in self.fighters(s, order):
            attacker = own.units[i]
            targets = self.exposed(enemy, flank)
            if not targets:
                continue
            shares = split(n, [enemy.units[t].count for t in targets])
            for t, k in zip(targets, shares, strict=True):
                if k == 0:
                    continue
                target = enemy.units[t]
                hold = self.rules["hold_percent"] if holding and not attacker.ranged else 100
                flanked = self.rules["flank_percent"] if flank else 100
                chase = (
                    200
                    if pursuit and attacker.cls is not None and attacker.cls == self.rules.get("pursuit_class")
                    else 100
                )
                total = (
                    k
                    * damage(self.world, attacker, target)
                    * self.matchup(attacker, target)
                    * mod
                    * roll
                    * hold
                    * flanked
                    * chase
                ) // 100**6
                dealt[t] = dealt.get(t, 0) + total

    def apply(self, side: Side, dealt: dict[int, int]) -> int:
        """Losses from the damage dealt to a side's units; returns how many fell."""
        losses = 0
        for t, amount in sorted(dealt.items()):
            unit = side.units[t]
            if unit.player:
                if unit.count and amount >= unit.hp:
                    unit.hp, unit.count = 0, 0
                    losses += 1
                elif unit.count:
                    unit.hp -= amount
                continue
            unit.remainder += amount
            hp = unit.stats["hp"]
            lost = min(unit.count, unit.remainder // hp)
            unit.remainder -= lost * hp
            unit.count -= lost
            if unit.count == 0:
                unit.remainder = 0
            losses += lost
        return losses

    def demoralize(self, side: Side, losses: int) -> None:
        morale = self.rules.get("morale")
        if morale is None:
            return
        num = losses * morale["factor"] + side.morale_remainder
        side.morale -= min(side.morale, num // side.start_size)
        side.morale_remainder = num % side.start_size

    def broke(self, side: Side) -> bool:
        morale = self.rules.get("morale")
        return morale is not None and side.morale < morale["rout"]

    def strength(self, side: Side) -> int:
        total = sum(u.count * (u.hp if u.player else u.stats["hp"]) * u.attack() for u in side.units)
        return total * self.morale_mod(side) // 10_000

    def pursue(self, winner: int) -> None:
        dealt: dict[int, int] = {}
        self.strike(winner, "charge", False, True, dealt)
        lost = self.apply(self.sides[1 - winner], dealt)
        self.log.append(f"pursuit by side {winner}: side {1 - winner} lost {lost}")

    def fight(self, order: str) -> str | None:
        """One round under the player's order; the outcome once it ends."""
        if order == "retreat":
            self.pursue(1)
            return "defeat"
        holding = order == "hold"
        dealt: list[dict[int, int]] = [{}, {}]
        self.strike(0, order, holding, False, dealt[1])
        self.strike(1, "charge", holding, False, dealt[0])
        losses = [self.apply(self.sides[s], dealt[s]) for s in (0, 1)]
        for s in (0, 1):
            self.demoralize(self.sides[s], losses[s])
        self.round += 1
        self.log.append(
            f"round {self.round}: "
            + " | ".join(
                f"side {s} strength {self.strength(self.sides[s])} lost {losses[s]} morale {self.sides[s].morale}"
                for s in (0, 1)
            )
        )
        gone = [self.broke(side) or not side.standing() for side in self.sides]
        if gone[0] and gone[1]:
            return "draw"
        if gone[0] or gone[1]:
            winner = 0 if gone[1] else 1
            self.pursue(winner)
            return "victory" if winner == 0 else "defeat"
        if self.round == self.rules["rounds"]:
            return "victory" if self.strength(self.sides[0]) > self.strength(self.sides[1]) else "defeat"
        return None


def load(path: Path) -> dict[str, Any]:
    world: dict[str, Any] = json.loads((path / "world.json").read_text())
    world["characters"] = json.loads((path / "characters.json").read_text())
    return world


def army_units(world: dict[str, Any], army_id: str) -> list[Unit]:
    character = next(c for c in world["characters"] if c["id"] == army_id)
    return [stack_unit(world, s["line"], s.get("level", 1), s["count"]) for s in character["army"]["troops"]]


def stack_unit(world: dict[str, Any], line_id: str, level: int, count: int) -> Unit:
    troops = world["troops"]
    line = next(line for line in troops["lines"] if line["id"] == line_id)
    cls = next(c for c in troops["classes"] if c["id"] == line["class"])
    name = next(lv["name"] for lv in reversed(line["levels"][:level]) if "name" in lv)
    return Unit(
        name=name,
        stats=line["levels"][level - 1]["stats"],
        count=count,
        cls=cls["id"],
        ranged=cls.get("ranged", False),
        mounted=cls.get("mounted", False),
        channel=line.get("channel", "physical"),
    )


def player_unit(world: dict[str, Any], level: int) -> Unit:
    stats = dict(world["combat"]["levels"][level - 1]["stats"])
    return Unit(
        name="You",
        stats=stats,
        count=1,
        cls=None,
        ranged=False,
        mounted=False,
        # The engine's player attacks in the authored basic channel (no gear here).
        channel=world["combat"].get("player_basic_channel", "physical"),
        player=True,
        hp=stats["hp"],
    )


def roster_order(world: dict[str, Any], roster: list[tuple[str, int, int]]) -> list[tuple[str, int, int]]:
    """The engine's order: lines as authored, levels descending."""
    lines = [line["id"] for line in world["troops"]["lines"]]
    return sorted(roster, key=lambda r: (lines.index(r[0]), -r[1]))


def main(argv: list[str]) -> None:
    parser = argparse.ArgumentParser(prog="python3 -m scripts.combat_sim battle")
    parser.add_argument("world", type=Path)
    parser.add_argument("--army", required=True)
    parser.add_argument("--ally", action="append", default=[])
    parser.add_argument("--roster", action="append", default=[], help="line:level:count")
    parser.add_argument("--orders", default="", help="comma-separated: charge, hold, flank, retreat")
    parser.add_argument("--level", type=int, default=1)
    parser.add_argument("--seed", type=lambda v: int(v, 0), default=0)
    args = parser.parse_args(argv)
    world = load(args.world)
    roster = [(p[0], int(p[1]), int(p[2])) for p in (r.split(":") for r in args.roster)]
    ours = [player_unit(world, args.level)]
    ours += [stack_unit(world, line, level, count) for line, level, count in roster_order(world, roster)]
    for ally in args.ally:
        ours += army_units(world, ally)
    battle = Battle(world, [Side(ours), Side(army_units(world, args.army))], args.seed & MASK)
    orders = [o for o in args.orders.split(",") if o]
    if any(o not in ORDERS for o in orders):
        parser.error(f"orders are {', '.join(ORDERS)}")
    outcome = None
    while outcome is None:
        outcome = battle.fight(orders[battle.round] if battle.round < len(orders) else "charge")
    for line in battle.log:
        print(line)
    print(f"outcome: {outcome}")
    for s, side in enumerate(battle.sides):
        left = ", ".join(f"{u.name} {u.hp if u.player else u.count}" for u in side.units)
        print(f"side {s}: {left}")
