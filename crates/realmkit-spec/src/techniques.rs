//! Techniques mastered rank by rank, and the grants that teach them.

use crate::*;

/// A technique mastered rank by rank. Each rank is named by the author.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Technique {
    pub id: Id,
    pub name: String,
    pub ranks: Vec<TechniqueRank>,
    /// Percent of the character XP from each victory this technique gains;
    /// small values make internal arts rise slowly.
    #[serde(default)]
    pub xp_share_percent: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TechniqueRank {
    /// Shown to the player instead of a number, in the source's own terms.
    pub name: String,
    /// Cumulative technique XP for this rank; the first rank's is 0.
    pub xp: u64,
    /// The skill the technique is used as at this rank; passive arts have none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skill: Option<Id>,
    /// The technique's whole stat bonus at this rank, replacing the previous rank's.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub passive: BTreeMap<Stat, u32>,
    /// A breakthrough gate: technique XP waits at this rank's threshold until it holds.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requires: Vec<Condition>,
}

/// Teaches a technique if it is unknown, raises it to at least `rank`
/// (1-based; teaching passes gates), then adds `xp`, which gates can hold back.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TechniqueGrant {
    pub technique: Id,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rank: Option<usize>,
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    pub xp: u64,
}

fn is_zero_u64(value: &u64) -> bool {
    *value == 0
}
