//! Pure combat arithmetic, shared with `scripts/combat_sim`.

use super::*;

/// Damage of one landed hit: the channel's combined attack `A` against its
/// combined defence `D` (each 100 × main + share × other), then
/// `max(1, A × power × A / (100 × 100 × (A + D)))`, rounded down once.
/// Defence equal to the attack halves damage; no scale constant or level enters.
pub fn damage(
    attacker: &Stats,
    defender: &Stats,
    channel: Channel,
    power: u32,
    share: u32,
) -> Result<u32, EngineError> {
    damage_scaled(attacker, defender, channel, power, share, 100)
}

/// [`damage`] scaled by `multiplier` percent (a critical hit) before the
/// single final rounding.
pub fn damage_scaled(
    attacker: &Stats,
    defender: &Stats,
    channel: Channel,
    power: u32,
    share: u32,
    multiplier: u32,
) -> Result<u32, EngineError> {
    damage_modified(
        attacker,
        defender,
        channel,
        power,
        share,
        multiplier,
        (1, 1),
    )
}

/// [`damage_scaled`] with the defender's damage modifier `num / den` for the
/// channel, before the single rounding. Immunity (`num` 0) deals nothing, not
/// the minimum 1.
pub fn damage_modified(
    attacker: &Stats,
    defender: &Stats,
    channel: Channel,
    power: u32,
    share: u32,
    multiplier: u32,
    (num, den): (u64, u64),
) -> Result<u32, EngineError> {
    if num == 0 {
        return Ok(0);
    }
    let a = attacker.combined(channel, share, false);
    let d = defender.combined(channel, share, true);
    let hit = a
        .checked_mul(u128::from(power))
        .and_then(|v| v.checked_mul(a))
        .and_then(|v| v.checked_mul(u128::from(multiplier)))
        .and_then(|v| v.checked_mul(u128::from(num)))
        .and_then(|v| v.checked_div(100 * 100 * 100 * (a + d) * u128::from(den)))
        .ok_or(EngineError::NumericLimit)?;
    u32::try_from(hit.max(1)).map_err(|_| EngineError::NumericLimit)
}

/// XP for defeating an opponent, by level difference `opponent − player`:
/// ±10% per level, capped at ±40%, rounded down, and nothing five or more
/// levels below. Mirrors `scripts/combat_sim` `XpRules.for_kill`.
pub fn xp_for_defeat(xp: u64, player_level: usize, opponent_level: usize) -> u64 {
    let diff = opponent_level as i128 - player_level as i128;
    if diff <= -5 {
        return 0;
    }
    let scaled = u128::from(xp) * (10 + diff.clamp(-4, 4)) as u128 / 10;
    // At most 140% of a u64 value: saturate rather than wrap on absurd content.
    u64::try_from(scaled).unwrap_or(u64::MAX)
}
