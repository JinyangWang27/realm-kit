"""Tuning: which variants of the rules and content still meet every balance target?

`robustness` nudges one tuned value at a time. `search` summarises any grid you
build yourself, for example:

    from dataclasses import replace
    from itertools import product
    from scripts.combat_sim.content import DEFAULT
    from scripts.combat_sim.model import Rules
    from scripts.combat_sim.tune import search

    beast, *others = DEFAULT.monsters
    search((f"beast HP {hp}, share {s}", Rules(cross_share=s),
            replace(DEFAULT, monsters=(replace(beast, hp=hp), *others)))
           for hp, s in product((60, 80, 100), (0, 25, 50)))
"""
from __future__ import annotations

import re
from collections import Counter
from collections.abc import Callable, Iterable, Iterator, Mapping
from dataclasses import replace
from typing import Any, TypeVar

from .content import Content, Tier
from .model import Kind, Profile, Rules, Skill
from .simulator import Simulator
from .targets import failure

Variant = tuple[str, Rules, Content]
# A knob applies a factor and returns the new rules and content plus the value before and after.
Knob = Callable[[Rules, Content, float], tuple[Rules, Content, float, float]]


T = TypeVar("T", Content, Profile, Rules, Skill, Tier)


def with_field(obj: T, name: str, value: Any) -> T:
    """`dataclasses.replace` for a field chosen at run time."""
    changes: dict[str, Any] = {name: value}
    return replace(obj, **changes)


def step(value: float, factor: float) -> int:
    """Scale an integer-valued setting, moving it by at least one."""
    scaled = round(value * factor)
    if scaled == round(value):
        scaled += 1 if factor > 1 else -1
    return max(0, scaled)


# A slot reads one profile out of the content and writes a replacement back.
Slot = tuple[Callable[[Content], Profile], Callable[[Content, Profile], Content]]


def _opponent_slot(i: int) -> Slot:
    def get(content: Content) -> Profile:
        return content.monsters[i]

    def put(content: Content, profile: Profile) -> Content:
        return replace(content, monsters=content.monsters[:i] + (profile,) + content.monsters[i + 1:])
    return get, put


def unique(taken: Mapping[str, object], label: str) -> str:
    """`label`, or `label #2`, `label #3`… — the first not already taken. Labels are display
    only, so any repeat gets a suffix rather than replacing an entry."""
    candidate, n = label, 2
    while candidate in taken:
        candidate, n = f"{label} #{n}", n + 1
    return candidate


def slots(content: Content) -> dict[str, Slot]:
    found: dict[str, Slot] = {
        "warrior": (lambda c: c.warrior, lambda c, p: replace(c, warrior=p)),
        "mage": (lambda c: c.mage, lambda c, p: replace(c, mage=p)),
    }
    for i, foe in enumerate(content.monsters):
        # Keyed by position; the name is only a label, made unique if it repeats.
        found[unique(found, foe.name)] = _opponent_slot(i)
    return found


def _profile_knob(slot: Slot, stat: str) -> Knob:
    get, put = slot

    def apply(rules: Rules, content: Content, factor: float) -> tuple[Rules, Content, float, float]:
        profile = get(content)
        before = getattr(profile, stat)
        after = step(before, factor)
        return rules, put(content, with_field(profile, stat, after)), before, after
    return apply


def _skill_knob(slot: Slot, index: int, field: str) -> Knob:
    """Selects the skill by its position, so same-named tiers are nudged independently."""
    get, put = slot

    def apply(rules: Rules, content: Content, factor: float) -> tuple[Rules, Content, float, float]:
        profile = get(content)
        before = getattr(profile.skills[index], field)
        after = step(before, factor)
        skills = tuple(with_field(s, field, after) if i == index else s for i, s in enumerate(profile.skills))
        return rules, put(content, replace(profile, skills=skills)), before, after
    return apply


def _tier_knob(kind: Kind, field: str) -> Knob:
    """Tier multipliers are fractions, so they scale without rounding."""
    def apply(rules: Rules, content: Content, factor: float) -> tuple[Rules, Content, float, float]:
        tier = content.tiers[kind]
        before = getattr(tier, field)
        after = round(before * factor, 3)
        tiers = {**content.tiers, kind: with_field(tier, field, after)}
        return rules, replace(content, tiers=tiers), before, after
    return apply


def _growth_knob(rules: Rules, content: Content, factor: float) -> tuple[Rules, Content, float, float]:
    """Scale the per-level growth rate (1.10 → 1.085), not the multiplier itself."""
    before = rules.growth
    after = round(1 + (before - 1) * factor, 4)
    return replace(rules, growth=after), content, before, after


def _basic_knob(slot: Slot, field: str) -> Knob:
    get, put = slot

    def apply(rules: Rules, content: Content, factor: float) -> tuple[Rules, Content, float, float]:
        profile = get(content)
        before = getattr(profile.basic, field)
        after = step(before, factor)
        return rules, put(content, replace(profile, basic=with_field(profile.basic, field, after))), before, after
    return apply


def _rules_knob(field: str) -> Knob:
    def apply(rules: Rules, content: Content, factor: float) -> tuple[Rules, Content, float, float]:
        before = getattr(rules, field)
        after = step(before, factor)
        return with_field(rules, field, after), content, before, after
    return apply


def add(found: dict[str, Knob], label: str, knob: Knob) -> None:
    """Store a knob under a unique label, so a clash (such as a skill named "basic attack")
    cannot replace another knob."""
    found[unique(found, label)] = knob


def knobs(content: Content) -> dict[str, Knob]:
    """Every tuned value worth nudging: player and opponent stats, skill power and cost, world rules."""
    found: dict[str, Knob] = {}
    all_slots = slots(content)
    for name, slot in all_slots.items():
        for stat in ("hp", "mp", "patk", "pdef", "satk", "sdef", "speed"):
            # Zero attacks are nudged too (upward only; see neighbourhood): 0 → 1 can matter.
            if getattr(slot[0](content), stat) or stat in ("patk", "satk"):
                add(found, f"{name} {stat}", _profile_knob(slot, stat))
    for name, slot in all_slots.items():
        skills = slot[0](content).skills
        for i, skill in enumerate(skills):
            shared = sum(s.name == skill.name for s in skills) > 1
            label = f"{name} {skill.name}" + (f" [{i}]" if shared else "")
            add(found, f"{label} power", _skill_knob(slot, i, "power"))
            add(found, f"{label} time", _skill_knob(slot, i, "time"))
            add(found, f"{label} cost", _skill_knob(slot, i, "cost"))  # a free skill is nudged 0 → 1
        for basic_field in ("power", "time"):
            add(found, f"{name} basic attack {basic_field}", _basic_knob(slot, basic_field))
    for kind in content.tiers:
        for tier_field in ("hp", "attack"):
            add(found, f"{kind.value} tier {tier_field}", _tier_knob(kind, tier_field))
    for field in ("cross_share", "mp_regen_percent", "rage_per_action", "rage_per_max_hp"):
        add(found, field, _rules_knob(field))
    add(found, "growth", _growth_knob)
    return found


def neighbourhood(rules: Rules, content: Content, factors: tuple[float, ...] = (0.85, 1.15)) -> Iterator[Variant]:
    for label, knob in knobs(content).items():
        for factor in factors:
            new_rules, new_content, before, after = knob(rules, content, factor)
            if after != before:  # a zero value has no downward nudge
                yield f"{label} {before:g} → {after:g}", new_rules, new_content


def robustness(rules: Rules, content: Content) -> None:
    """Print which single-value nudges keep every target and which target breaks."""
    variants = list(neighbourhood(rules, content))
    held = 0
    for label, variant_rules, variant_content in variants:
        broken = failure(Simulator(variant_rules, variant_content))
        held += broken is None
        print(f"  {label:<32} {'ok' if broken is None else 'breaks: ' + broken}")
    print(f"{held} of {len(variants)} single-value nudges keep every target")


def search(variants: Iterable[Variant], show: int = 20) -> list[str]:
    """Run every variant; print the passing labels and a tally of the first broken target."""
    passing: list[str] = []
    tally: Counter[str] = Counter()
    for label, rules, content in variants:
        broken = failure(Simulator(rules, content))
        if broken is None:
            passing.append(label)
        else:
            tally[re.sub(r"\(.*|\[.*|-?\d+", "#", broken)] += 1
    print(f"{len(passing)} passing of {len(passing) + sum(tally.values())}")
    for label in passing[:show]:
        print(f"  {label}")
    for message, count in tally.most_common(6):
        print(f"  {count:>5} × {message}")
    return passing
