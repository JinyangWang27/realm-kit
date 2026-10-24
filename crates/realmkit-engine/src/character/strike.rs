//! What one attack or skill does to its target, however it came to be used:
//! its channel, power and crit, the damage it deals and the HP it takes.

use super::*;
use realmkit_spec::Crit;

/// One attack as the damage rules see it.
#[derive(Clone, Copy)]
pub(crate) struct Attack {
    pub channel: Channel,
    pub power: u32,
    pub share: u32,
    pub crit: Option<Crit>,
}

impl Attack {
    /// `skill`, or a basic attack in `channel` with the basic `crit`.
    pub(crate) fn of(
        world: &WorldSpec,
        skill: Option<&Skill>,
        channel: Channel,
        crit: Option<Crit>,
    ) -> Self {
        let cross_share = world.combat().unwrap().cross_share;
        match skill {
            Some(s) => Self {
                channel: s.channel,
                power: s.power,
                share: s.cross_share.unwrap_or(cross_share),
                crit: s.crit,
            },
            None => Self {
                channel,
                power: BASIC_POWER,
                share: cross_share,
                crit,
            },
        }
    }
}

/// The damage `attack` deals from `attacker` to `defender`, whose worn
/// modifier for the channel is `guard`, and whether it was critical. A crit
/// draws from the combat stream only when an attack that has one resolves.
pub(crate) fn damage_of(
    attacker: &Stats,
    defender: &Stats,
    attack: Attack,
    guard: (u64, u64),
    rng: &mut Option<RngState>,
) -> Result<(u32, bool), EngineError> {
    let critical = match (attack.crit, rng.as_mut().and_then(|r| r.combat.as_mut())) {
        (Some(c), Some(stream)) => rng::chance(stream, c.chance_percent),
        _ => false,
    };
    let multiplier = attack
        .crit
        .filter(|_| critical)
        .map_or(100, |c| c.multiplier_percent);
    let dealt = damage_modified(
        attacker,
        defender,
        attack.channel,
        attack.power,
        attack.share,
        multiplier,
        guard,
    )?;
    Ok((dealt, critical))
}

/// Takes `dealt` damage from `vitals`, never below `floor` HP; the HP taken.
pub(crate) fn take(vitals: &mut Vitals, dealt: u32, floor: u32) -> u32 {
    let taken = dealt.min(vitals.hp - floor);
    vitals.hp -= taken;
    taken
}
