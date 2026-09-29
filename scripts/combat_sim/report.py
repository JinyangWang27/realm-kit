"""A readable table of how the current rules and content play."""
from __future__ import annotations

from .model import Kind
from .simulator import Outcome, Simulator, XpRules, hp_lost_percent


def show(outcome: Outcome | None) -> str:
    return str(outcome) if outcome else "   LOSS  "


def report(sim: Simulator) -> None:
    print(f"formula: {sim.rules.formula.value}, cross share {sim.rules.cross_share}%; actions/HP lost per fight")
    for foe in sim.content.monsters:
        print(f"\n======== against {foe.name} ========")
        report_against(sim.against(foe))


def report_against(sim: Simulator) -> None:
    warrior, mage = sim.content.warrior, sim.content.mage
    for build in sim.content.builds:
        print(f"\n{build.name} vs monster level offset" + " " * 18 + "fights/rest")
        print("  lvl " + "".join(f"{d:>+9} " for d in (-3, 0, 1, 2, 4)))
        for p in (1, 3, 5, 10, 20, 30):
            row = " ".join(show(sim.outcome(build, p, [sim.monster(p + d)])) if p + d >= 1 else "      -  "
                           for d in (-3, 0, 1, 2, 4))
            print(f"  {p:<3} {row}   {sim.fights_per_rest(build, p, p)}")

    print(f"\nspeed: {warrior.name} L5, HP lost vs normal / boss L5")
    for speed in (80, 100, 110, 120, 140, 160, 200):
        player = sim.player(warrior, 5, speed)
        cells = []
        for kind in (Kind.NORMAL, Kind.BOSS):
            result = sim.fight(player, [sim.monster(5, kind)])
            cells.append(f"{hp_lost_percent(player, result.hp)}%" if result.won else "LOSS")
        print(f"  speed {speed:>3}: {cells[0]:>5} / {cells[1]:>5}")

    print("\npacks at player L5, HP lost")
    for build in sim.content.builds:
        packs = [f"{n}x {kind.value}: {show(sim.outcome(build, 5, [sim.monster(5, kind)] * n)).strip()}"
                 for kind, n in ((Kind.NORMAL, 2), (Kind.MINION, 2), (Kind.MINION, 3), (Kind.MINION, 4))]
        print(f"  {build.name:<7} " + " | ".join(packs))

    print("\nlowest player level that beats a level-B boss")
    for b in (3, 5, 10, 20):
        print(f"  B={b:<3} {warrior.name} L{sim.boss_level_needed(warrior, b)}, "
              f"{mage.name} L{sim.boss_level_needed(mage, b)}")

    print("\ngrinding from L1 to L10")
    for xp in (XpRules(decay=True), XpRules(decay=False)):
        for build in sim.content.builds:
            print(f"  decay {'on ' if xp.decay else 'off'} {build.name:<7} "
                  f"best monster: {sim.grind(build, 10, xp)} | farm L1 only: {sim.grind(build, 10, xp, farm=1)}")
