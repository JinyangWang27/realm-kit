"""Balance simulator for the proposed M3 combat rules (ROADMAP.md, M3).

Models the proposal, not the engine: two damage channels, physical and special
(a world names special as magic, 内力, mana...), where the matching stats
dominate and the other channel's stats add a smaller share; gradual defence with
one final integer floor and a minimum of 1; timeline delays
ceil(action_cost / speed) after an opening delay, ties by (time, side,
participant order), and "attack the first living opponent" for every side. Each
fighter uses its strongest affordable skill every turn. Skills spend MP
(regenerates over encounter time, fully by resting) or rage (starts at 0 each
fight; builds from acting and from damage taken). HP recovers only by resting.

Run from the repository root:

    python3 -m scripts.combat_sim                        # report, K-free ratio formula
    python3 -m scripts.combat_sim check                  # assert the balance targets
    python3 -m scripts.combat_sim tune                   # nudge each tuned value ±15%
    python3 -m scripts.combat_sim --formula k-scaled     # compare: K grows with attacker level

Modules: model (rules, formulas, skills, characters), content (tuned values to
swap when prototyping), encounter (the timeline), simulator (experiments),
targets (balance assertions), report, tune.

Every number here is a tuning candidate, not an agreed balance constant.
"""
