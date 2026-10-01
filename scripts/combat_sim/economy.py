"""The M6a price tick and trade prices, mirrored from the engine's rules/economy.rs.

Reads a package's `economy` block and runs the price tick offline:

    python3 -m scripts.combat_sim economy examples/marches --seed 7 --ticks 10
    python3 -m scripts.combat_sim economy examples/marches --seed 0 --ticks 15 --prices

The first prints each market's indices and trade prices after every tick; the
second prints the indices after the warm-up as JSON `prices` maps to author as
starting prices, since the engine never warms up at runtime. Engine tests pin
numbers this module produces, so a rule change here or there must change both.
"""

from __future__ import annotations

import argparse
import json
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

MASK = (1 << 64) - 1
BASE_INDEX = 1_000
DOMAIN_MARKET = 0x6D61_726B_6574  # "market" in ASCII, as in the engine's rng.rs


def splitmix64(state: int) -> tuple[int, int]:
    """One SplitMix64 step: the advanced state and its output."""
    state = (state + 0x9E37_79B9_7F4A_7C15) & MASK
    z = state
    z = ((z ^ (z >> 30)) * 0xBF58_476D_1CE4_E5B9) & MASK
    z = ((z ^ (z >> 27)) * 0x94D0_49BB_1331_11EB) & MASK
    return state, z ^ (z >> 31)


@dataclass
class Stream:
    """The engine's `market` random stream."""

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


@dataclass
class Economy:
    """An economy block as authored, plus every market's current indices."""

    raw: dict[str, Any]
    goods: list[str] = field(init=False)
    markets: list[str] = field(init=False)
    prices: dict[str, dict[str, int]] = field(init=False)

    def __post_init__(self) -> None:
        self.goods = [g["item"] for g in self.raw["goods"]]
        self.markets = [m["location"] for m in self.raw["markets"]]
        self.prices = {
            m["location"]: {g: m.get("prices", {}).get(g, BASE_INDEX) for g in self.goods} for m in self.raw["markets"]
        }

    def good(self, item: str) -> dict[str, Any]:
        return next(g for g in self.raw["goods"] if g["item"] == item)

    def market(self, location: str) -> dict[str, Any]:
        return next(m for m in self.raw["markets"] if m["location"] == location)

    def spread(self, location: str) -> int:
        market = self.market(location)
        return int(market.get("spread_percent", self.raw["spread_percent"]))

    def supply(self, location: str, item: str, index: int) -> tuple[int, int]:
        """What a market makes and uses up of a good on one tick at price `index`; producers
        use less of a dear good, scaled by 1,000 / index above the base."""
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
        return made, int(good.get("demand", {}).get(market["kind"], 0)) + industry

    def tick(self, stream: Stream) -> None:
        """One price tick in four phases, exactly as the engine runs it."""
        rules = self.raw["tick"]
        low, high = self.raw["index_bounds"]
        step = rules["supply_step"]
        damp_below = rules["damp_below"]
        index = {(m, g): self.prices[m][g] for m in self.markets for g in self.goods}
        # 1. Supply: draws in market, then goods, order, only where unbalanced.
        for m in self.markets:
            for g in self.goods:
                value = index[(m, g)]
                made, used = self.supply(m, g, value)
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


def load(world: Path) -> Economy:
    header = json.loads((world / "world.json").read_text(encoding="utf-8"))
    return Economy(header["economy"])


def table(economy: Economy) -> str:
    """Each market's index, buying and selling price for every good."""
    lines = []
    for m in economy.markets:
        spread = economy.spread(m)
        cells = []
        for g in economy.goods:
            index = economy.prices[m][g]
            price = int(economy.good(g)["price"])
            cells.append(f"{g} {index} ({buy_price(price, index, spread)}/{sell_price(price, index, spread)})")
        lines.append(f"  {m}: " + ", ".join(cells))
    return "\n".join(lines)


def main(argv: list[str]) -> None:
    parser = argparse.ArgumentParser(
        prog="python3 -m scripts.combat_sim economy", description="Run a package's price tick offline."
    )
    parser.add_argument("world", type=Path, help="package directory with an economy block")
    parser.add_argument("--seed", type=int, default=0)
    parser.add_argument("--ticks", type=int, default=10)
    parser.add_argument("--prices", action="store_true", help="print the final indices as authored prices")
    args = parser.parse_args(argv)
    economy = load(args.world)
    stream = Stream(args.seed ^ DOMAIN_MARKET)
    for tick in range(1, args.ticks + 1):
        economy.tick(stream)
        if not args.prices:
            print(f"tick {tick}:\n{table(economy)}")
    if args.prices:
        print(json.dumps(economy.prices, indent=2))
