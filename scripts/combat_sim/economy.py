"""The M6a economy, mirrored from the engine's rules/economy.rs: the price tick,
prosperity, merchants' stock, workshop settlement and trade prices.

Reads a package's `economy` block and runs its schedules offline:

    python3 -m scripts.combat_sim economy examples/marches --seed 7 --ticks 10
    python3 -m scripts.combat_sim economy examples/marches --seed 0 --ticks 15 --prices

The first prints each market's indices, trade prices, prosperity and stock
after every price tick, plus what each workshop kind would net in each town
at those prices. The second prints the markets' indices (and prosperity)
after the warm-up as JSON to author as starting values, since the engine
never warms up at runtime. Every occurrence up to and including the minute of
the last price tick runs, in the engine's order: the price tick, prosperity,
restocking, then workshop settlement. Engine tests pin numbers this module
produces, so a rule change here or there must change both.
"""

from __future__ import annotations

import argparse
import json
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

MASK = (1 << 64) - 1
BASE_INDEX = 1_000
PROSPERITY_BOUND = 100
STOCK_BOUND = 1_000_000
DOMAIN_MARKET = 0x6D61_726B_6574  # "market" in ASCII, as in the engine's rng.rs
DOMAIN_STOCK = 0x73_746F_636B  # "stock" in ASCII
# Same-minute order of the economy's schedules, as in the engine's rules/time.rs.
ORDER = ("tick", "prosperity", "stock", "workshops")


def splitmix64(state: int) -> tuple[int, int]:
    """One SplitMix64 step: the advanced state and its output."""
    state = (state + 0x9E37_79B9_7F4A_7C15) & MASK
    z = state
    z = ((z ^ (z >> 30)) * 0xBF58_476D_1CE4_E5B9) & MASK
    z = ((z ^ (z >> 27)) * 0x94D0_49BB_1331_11EB) & MASK
    return state, z ^ (z >> 31)


@dataclass
class Stream:
    """One of the engine's random streams: `market` or `stock`."""

    state: int

    def below(self, n: int) -> int:
        """A uniform draw below n, scaled by multiplication rather than modulo."""
        self.state, value = splitmix64(self.state)
        return (value * n) >> 64


def tdiv(a: int, b: int) -> int:
    """Integer division rounding towards zero, as Rust's i64 division does."""
    quotient = abs(a) // b
    return quotient if a >= 0 else -quotient


def buy_price(price: int, index: int, spread: int) -> int:
    return max(1, price * index * (100 + spread) // 100_000)


def sell_price(price: int, index: int, spread: int) -> int:
    return price * index * 100 // (1_000 * (100 + spread))


def scaled(value: int, percent: list[int], prosperity: int) -> int:
    """`value` scaled by a percentage running linearly from percent[0] at prosperity 0
    to percent[1] at prosperity 100, rounded down once."""
    p = min(prosperity, PROSPERITY_BOUND)
    return value * (percent[0] * (100 - p) + percent[1] * p) // 10_000


def next_minute(schedule: dict[str, int], minute: int, inclusive: bool) -> int | None:
    """The schedule's first occurrence at or after `minute`, or strictly after it."""
    floor = minute if inclusive else minute + 1
    if schedule["at"] >= floor:
        return schedule["at"]
    every = schedule.get("every")
    if every is None:
        return None
    return schedule["at"] + -(-(floor - schedule["at"]) // every) * every


@dataclass
class Economy:
    """An economy block as authored, plus every market's current state."""

    raw: dict[str, Any]
    goods: list[str] = field(init=False)
    markets: list[str] = field(init=False)
    prices: dict[str, dict[str, int]] = field(init=False)
    prosperity: dict[str, int] | None = field(init=False)
    stock: dict[str, dict[str, int]] | None = field(init=False)
    purse: dict[str, int] | None = field(init=False)

    def __post_init__(self) -> None:
        self.goods = [g["item"] for g in self.raw["goods"]]
        self.markets = [m["location"] for m in self.raw["markets"]]
        self.prices = {
            m["location"]: {g: m.get("prices", {}).get(g, BASE_INDEX) for g in self.goods} for m in self.raw["markets"]
        }
        rules = self.raw.get("prosperity")
        self.prosperity = None
        if rules is not None:
            self.prosperity = {m["location"]: m.get("prosperity", rules["base"]) for m in self.raw["markets"]}
        self.stock, self.purse = None, None
        if "stock" in self.raw:
            # New Game: every stock at its target and every purse full, without a draw.
            self.stock, self.purse = {}, {}
            for m in self.markets:
                self.purse[m], self.stock[m] = self.targets(m)

    def good(self, item: str) -> dict[str, Any]:
        return next(g for g in self.raw["goods"] if g["item"] == item)

    def market(self, location: str) -> dict[str, Any]:
        return next(m for m in self.raw["markets"] if m["location"] == location)

    def villages(self, town: str) -> list[str]:
        return [m["location"] for m in self.raw["markets"] if m.get("town") == town]

    def spread(self, location: str, rank: int = 0) -> int:
        """The spread at a market, narrowed by `rank` in trading."""
        market = self.market(location)
        spread = int(market.get("spread_percent", self.raw["spread_percent"]))
        narrow = int(self.raw.get("trading", {}).get("narrow_percent", 0))
        return spread * max(0, 100 - narrow * rank) // 100

    def own_supply(self, location: str, item: str, index: int) -> tuple[int, int]:
        """What a market itself makes and uses up of a good on one tick at price
        `index`: producers use less of a dear good, scaled by 1,000 / index above the
        base, and with prosperity the kind's demand scales with it."""
        market = self.market(location)
        good = self.good(item)
        made, industry = 0, 0
        producers = {p["id"]: p for p in self.raw.get("producers", [])}
        for producer_id, count in market.get("producers", {}).items():
            producer = producers[producer_id]
            made += count * producer.get("yields", {}).get(item, 0)
            industry += count * producer.get("consumes", {}).get(item, 0)
        if index > BASE_INDEX:
            industry = industry * BASE_INDEX // index
        demand = int(good.get("demand", {}).get(market["kind"], 0))
        if self.prosperity is not None:
            demand = scaled(demand, self.raw["prosperity"].get("demand_percent", [100, 100]), self.prosperity[location])
        return made, demand + industry

    def supply(self, location: str, item: str, index: dict[tuple[str, str], int]) -> tuple[int, int]:
        """A market's own supply at `index`, plus each of its villages' at theirs."""
        made, used = self.own_supply(location, item, index[(location, item)])
        if self.market(location)["kind"] == "town":
            for village in self.villages(location):
                m, u = self.own_supply(village, item, index[(village, item)])
                made, used = made + m, used + u
        return made, used

    def tick(self, stream: Stream) -> None:
        """One price tick in four phases, exactly as the engine runs it."""
        rules = self.raw["tick"]
        low, high = self.raw["index_bounds"]
        step = rules["supply_step"]
        damp_below = rules["damp_below"]
        index = {(m, g): self.prices[m][g] for m in self.markets for g in self.goods}
        # 1. Supply, measured on the pre-tick prices: draws in market, then goods,
        #    order, only where unbalanced.
        before = dict(index)
        for m in self.markets:
            for g in self.goods:
                value = index[(m, g)]
                made, used = self.supply(m, g, before)
                if made > used and step > 0:
                    fall = stream.below((made - used) * step)
                    if value < damp_below:
                        fall = fall * value // damp_below
                    value = max(value - fall, low)
                elif used > made and step > 0:
                    value = min(value + stream.below((used - made) * step), high)
                index[(m, g)] = value
        # 2. Revert towards the base index.
        for key, value in index.items():
            index[key] = BASE_INDEX + tdiv((value - BASE_INDEX) * (100 - rules["revert_percent"]), 100)
        # 3. Inputs pull their outputs up, measured on the reverted prices.
        reverted = dict(index)
        pull = rules.get("input_pull_percent", 0)
        for m in self.markets:
            for g in self.goods:
                source = self.good(g).get("input")
                if source is not None and reverted[(m, source)] > reverted[(m, g)]:
                    index[(m, g)] = reverted[(m, g)] + (reverted[(m, source)] - reverted[(m, g)]) * pull // 100
        # 4. Linked markets converge, measured on the pulled prices, applied together.
        pulled = dict(index)
        for link in self.raw.get("links", []):
            a, b = link["between"]
            for g in self.goods:
                move = tdiv((pulled[(b, g)] - pulled[(a, g)]) * link["percent"], 100)
                index[(a, g)] += move
                index[(b, g)] -= move
        for (m, g), value in index.items():
            self.prices[m][g] = min(max(value, low), high)

    def needs(self, location: str, item: str) -> bool:
        """The market's kind demands the good or its producers use it up."""
        market = self.market(location)
        if self.good(item).get("demand", {}).get(market["kind"], 0) > 0:
            return True
        producers = {p["id"]: p for p in self.raw.get("producers", [])}
        return any(
            count > 0 and producers[p].get("consumes", {}).get(item, 0) > 0
            for p, count in market.get("producers", {}).items()
        )

    def prosper(self) -> None:
        """Each market's prosperity moves one point towards its ideal: the base,
        lowered for each needed good at or above the scarcity level."""
        assert self.prosperity is not None
        rules = self.raw["prosperity"]
        for m in self.markets:
            scarce = sum(1 for g in self.goods if self.needs(m, g) and self.prices[m][g] >= rules["scarce_above"])
            ideal = max(0, rules["base"] - rules["scarcity"] * scarce)
            now = self.prosperity[m]
            self.prosperity[m] = now + (ideal > now) - (ideal < now)

    def targets(self, location: str) -> tuple[int, dict[str, int]]:
        """A full purse and each good's target stock: the scaled units, split by
        what the market (and its villages) makes, more while cheap."""
        rules = self.raw["stock"]
        units, currency = rules["units"], rules["currency"]
        if self.prosperity is not None:
            percent = rules.get("prosperity_percent", [100, 100])
            units = scaled(units, percent, self.prosperity[location])
            currency = scaled(currency, percent, self.prosperity[location])
        index = {(m, g): self.prices[m][g] for m in self.markets for g in self.goods}
        weights = {g: self.supply(location, g, index)[0] * BASE_INDEX // self.prices[location][g] for g in self.goods}
        total = sum(weights.values())
        return currency, {g: units * weights[g] // total if total else 0 for g in self.goods}

    def restock(self, stream: Stream) -> None:
        """Each market's purse fills and its stock is redrawn around its targets."""
        assert self.stock is not None and self.purse is not None
        for m in self.markets:
            self.purse[m], targets = self.targets(m)
            for g in self.goods:
                target = targets[g]
                self.stock[m][g] = stream.below(2 * target + 1) if target > 0 else 0

    def workshop_net(self, kind: dict[str, Any], location: str) -> int:
        """One settlement of a workshop at the town's prices, with no spread."""

        def value(item: str, units: int) -> int:
            return int(self.good(item)["price"]) * self.prices[location][item] * units // 1_000

        net = value(kind["good"], kind["output"]) - int(kind.get("overhead", 0))
        for item, units in kind.get("inputs", {}).items():
            net -= value(item, units)
        return net

    def schedules(self) -> list[tuple[dict[str, int], str]]:
        return [(self.raw[name]["schedule"], name) for name in ORDER if name in self.raw]

    def run(self, start: int, end: int, market: Stream, stock: Stream) -> None:
        """Every occurrence after `start` up to and including `end`, by minute, then
        in schedule order."""
        due = self.schedules()
        cursor = (start, len(due))
        while True:
            candidates = []
            for i, (schedule, _) in enumerate(due):
                minute = next_minute(schedule, cursor[0], i > cursor[1])
                if minute is not None and minute <= end:
                    candidates.append((minute, i))
            if not candidates:
                return
            cursor = min(candidates)
            name = due[cursor[1]][1]
            if name == "tick":
                self.tick(market)
            elif name == "prosperity":
                self.prosper()
            elif name == "stock":
                self.restock(stock)


def load(world: Path) -> tuple[Economy, int]:
    """The package's economy and the minute play starts at."""
    header = json.loads((world / "world.json").read_text(encoding="utf-8"))
    return Economy(header["economy"]), int(header.get("time", {}).get("start", 0))


def table(economy: Economy, rank: int = 0) -> str:
    """Each market's index, buying and selling price, and stock for every good, then
    its prosperity and purse, then what each workshop kind nets in each town."""
    lines = []
    for m in economy.markets:
        spread = economy.spread(m, rank)
        cells = []
        for g in economy.goods:
            index = economy.prices[m][g]
            price = int(economy.good(g)["price"])
            cell = f"{g} {index} ({buy_price(price, index, spread)}/{sell_price(price, index, spread)})"
            if economy.stock is not None:
                cell += f" ×{economy.stock[m][g]}"
            cells.append(cell)
        extra = ""
        if economy.prosperity is not None:
            extra += f" · prosperity {economy.prosperity[m]}"
        if economy.purse is not None:
            extra += f" · purse {economy.purse[m]}"
        lines.append(f"  {m}: " + ", ".join(cells) + extra)
    for kind in economy.raw.get("workshops", {}).get("kinds", []):
        towns = [m for m in economy.markets if economy.market(m)["kind"] == "town"]
        nets = ", ".join(f"{m} {economy.workshop_net(kind, m):+d}" for m in towns)
        lines.append(f"  {kind['id']} weekly: {nets}")
    return "\n".join(lines)


def main(argv: list[str]) -> None:
    parser = argparse.ArgumentParser(
        prog="python3 -m scripts.combat_sim economy", description="Run a package's economy offline."
    )
    parser.add_argument("world", type=Path, help="package directory with an economy block")
    parser.add_argument("--seed", type=int, default=0)
    parser.add_argument("--ticks", type=int, default=10, help="run through the minute of this price tick")
    parser.add_argument("--trading", type=int, default=0, help="the player's trading rank, for prices shown")
    parser.add_argument("--prices", action="store_true", help="print the final indices as authored values")
    args = parser.parse_args(argv)
    economy, start = load(args.world)
    # Only ranks the engine could reach: none without trading, and at most its top.
    top = int(economy.raw.get("trading", {}).get("max", 0))
    if not 0 <= args.trading <= top:
        parser.error(f"--trading is a rank from 0 to {top} in this package")
    market, stock = Stream(args.seed ^ DOMAIN_MARKET), Stream(args.seed ^ DOMAIN_STOCK)
    tick = economy.raw["tick"]["schedule"]
    for n in range(1, args.ticks + 1):
        # Through the nth price tick's minute, so its prosperity, restock and
        # settlement, which come after it at that minute, have happened too.
        end = tick["at"] + (n - 1) * tick.get("every", 0)
        economy.run(start, end, market, stock)
        start = end
        if not args.prices:
            print(f"tick {n}:\n{table(economy, args.trading)}")
    if args.prices:
        markets: dict[str, Any] = {}
        for m in economy.markets:
            markets[m] = {"prices": economy.prices[m]}
            if economy.prosperity is not None:
                markets[m]["prosperity"] = economy.prosperity[m]
        print(json.dumps(markets, indent=2))
