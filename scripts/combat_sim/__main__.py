"""Command line: python3 -m scripts.combat_sim [report|check|tune] [--formula ...], or
python3 -m scripts.combat_sim economy <world> [--seed N] [--ticks N] [--prices], or
python3 -m scripts.combat_sim battle <world> --army ID [--roster line:level:count] [--orders ...] [--seed N]"""

from __future__ import annotations

import argparse
import sys

from . import battle, economy
from .content import DEFAULT
from .model import Formula, Rules
from .report import report
from .simulator import Simulator
from .targets import TargetMissed, check
from .tune import robustness


def main() -> None:
    if sys.argv[1:2] == ["economy"]:
        economy.main(sys.argv[2:])
        return
    if sys.argv[1:2] == ["battle"]:
        battle.main(sys.argv[2:])
        return
    parser = argparse.ArgumentParser(
        prog="python3 -m scripts.combat_sim", description="Balance simulator for the proposed M3 combat rules."
    )
    parser.add_argument(
        "command",
        nargs="?",
        choices=("report", "check", "tune"),
        default="report",
        help="report tables (default), assert balance targets, or nudge tuned values",
    )
    parser.add_argument("--formula", choices=[f.value for f in Formula], default=Formula.RATIO.value)
    args = parser.parse_args()
    rules = Rules(formula=Formula(args.formula))
    if args.command == "tune":
        robustness(rules, DEFAULT)
        return
    sim = Simulator(rules, DEFAULT)
    if args.command == "report":
        report(sim)
        return
    try:
        check(sim)
    except TargetMissed as error:
        sys.exit(f"balance target failed: {error}")
    print("all balance targets hold")


if __name__ == "__main__":
    main()
