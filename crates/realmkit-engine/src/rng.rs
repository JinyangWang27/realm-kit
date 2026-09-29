//! Seeded, versioned randomness. The generator is written out here rather
//! than taken from a crate so its output can never change under a save.

use serde::{Deserialize, Serialize};

/// The algorithm a saved stream uses: 1 is SplitMix64.
pub const RNG_VERSION: u32 = 1;
/// "combat" in ASCII: each domain starts from its own state, so draws in one
/// never shift another.
const DOMAIN_COMBAT: u64 = 0x636f_6d62_6174;

/// Saved generator state, one stream per random domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RngState {
    pub version: u32,
    pub combat: u64,
}

impl RngState {
    pub fn new(seed: u64) -> Self {
        Self {
            version: RNG_VERSION,
            combat: seed ^ DOMAIN_COMBAT,
        }
    }
}

/// One SplitMix64 step: advances `state` and returns the next output.
pub fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// A draw that succeeds `percent` times in 100, scaled by multiplication
/// rather than a biased modulo.
pub(crate) fn chance(state: &mut u64, percent: u32) -> bool {
    ((u128::from(splitmix64(state)) * 100) >> 64) < u128::from(percent)
}
