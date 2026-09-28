"""Tuning: which variants of the rules and content still meet every balance target?

`robustness` nudges one tuned value at a time. `search` summarises any grid you
build yourself, for example:

    from dataclasses import replace
    from itertools import product
    from scripts.combat_sim.content import DEFAULT
    from scripts.combat_sim.model import Rules
    from scripts.combat_sim.tune import search

    search((f"HP {hp}, share {s}", Rules(cross_share=s),
            replace(DEFAULT, monster=replace(DEFAULT.monster, hp=hp)))
           for hp, s in product((60, 80, 100), (0, 25, 50)))
"""
from __future__ import annotations

import re
from collections import Counter
from collections.abc import Callable, Iterable, Iterator
from dataclasses import replace
from typing import Any, TypeVar

from .content import Content
from .model import Profile, Rules, Skill
from .simulator import Simulator
from .targets import failure

Variant = tuple[str, Rules, Content]
# A knob applies a factor and returns the new rules and content plus the value before and after.
Knob = Callable[[Rules, Content, float], tuple[Rules, Content, float, float]]


T = TypeVar("T", Content, Profile, Rules, Skill)


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


def slots(content: Content) -> dict[str, Slot]:
    found: dict[str, Slot] = {
        "warrior": (lambda c: c.warrior, lambda c, p: replace(c, warrior=p)),
        "mage": (lambda c: c.mage, lambda c, p: replace(c, mage=p)),
    }
    for i, foe in enumerate(content.monsters):
        found[foe.name] = _opponent_slot(i)
    return found


def _profile_knob(slot: Slot, stat: str) -> Knob:
    get, put = slot

    def apply(rules: Rules, content: Content, factor: float) -> tuple[Rules, Content, float, float]:
        profile = get(content)
        before = getattr(profile, stat)
        after = step(before, factor)
        return rules, put(content, with_field(profile, stat, after)), before, after
    return apply


def _skill_knob(slot: Slot, skill_name: str, field: str) -> Knob:
    get, put = slot

    def apply(rules: Rules, content: Content, factor: float) -> tuple[Rules, Content, float, float]:
        profile = get(content)
        skill = next(s for s in profile.skills if s.name == skill_name)
        before = getattr(skill, field)
        after = step(before, factor)
        skills = tuple(with_field(s, field, after) if s is skill else s for s in profile.skills)
        return rules, put(content, replace(profile, skills=skills)), before, after
    return apply


def _rules_knob(field: str) -> Knob:
    def apply(rules: Rules, content: Content, factor: float) -> tuple[Rules, Content, float, float]:
        before = getattr(rules, field)
        after = step(before, factor)
        return with_field(rules, field, after), content, before, after
    return apply


def knobs(content: Content) -> dict[str, Knob]:
    """Every tuned value worth nudging: opponent stats, skill power and cost, world rules."""
    found: dict[str, Knob] = {}
    all_slots = slots(content)
    for foe in content.monsters:
        for stat in ("hp", "patk", "pdef", "satk", "sdef", "speed"):
            if getattr(foe, stat):
                found[f"{foe.name} {stat}"] = _profile_knob(all_slots[foe.name], stat)
    for name, slot in all_slots.items():
        for skill in slot[0](content).skills:
            found[f"{name} {skill.name} power"] = _skill_knob(slot, skill.name, "power")
            if skill.cost:
                found[f"{name} {skill.name} cost"] = _skill_knob(slot, skill.name, "cost")
    for field in ("cross_share", "mp_regen_percent", "rage_per_max_hp"):
        found[field] = _rules_knob(field)
    return found


def neighbourhood(rules: Rules, content: Content, factors: tuple[float, ...] = (0.85, 1.15)) -> Iterator[Variant]:
    for label, knob in knobs(content).items():
        for factor in factors:
            new_rules, new_content, before, after = knob(rules, content, factor)
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
