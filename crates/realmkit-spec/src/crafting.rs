//! Forging new equipment from recipes, and improving a piece tier by tier.

use crate::*;

/// Forges one new piece of `output` from `inputs` at a station. Hidden until
/// `known_when` holds; then shown, and usable once `requires` holds too.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Recipe {
    pub id: Id,
    /// A station some location offers, for example an anvil.
    pub station: Id,
    pub inputs: Vec<ItemStack>,
    /// A wearable item; each forge makes one new piece of it.
    pub output: Id,
    /// Knowledge: a teacher, a found plan or a quest makes it true.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub known_when: Vec<Condition>,
    /// Ability, such as a smithing rank; unmet requirements are shown.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requires: Vec<Condition>,
    /// Technique XP each successful forge gives; teaches the technique if unknown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trains: Option<TechniqueGrant>,
}

/// One step up from the tier before: its bonuses (and speed penalty, if set)
/// replace the previous ones rather than adding to them.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Tier {
    /// The improved piece's name; `{item}` is the item's own name.
    pub name: TextTemplate,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub bonuses: BTreeMap<Stat, u32>,
    /// Replaces the item's speed penalty at this tier; kept when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speed_penalty: Option<u32>,
    pub station: Id,
    pub cost: Vec<ItemStack>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requires: Vec<Condition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trains: Option<TechniqueGrant>,
}

impl Equipment {
    /// Stat bonuses at `tier` (0 is the item as defined).
    pub fn bonuses_at(&self, tier: usize) -> &BTreeMap<Stat, u32> {
        tier.checked_sub(1)
            .and_then(|i| self.tiers.get(i))
            .map_or(&self.bonuses, |t| &t.bonuses)
    }

    /// Speed penalty at `tier`: the latest tier that sets one, else the item's.
    pub fn speed_penalty_at(&self, tier: usize) -> u32 {
        self.tiers[..tier.min(self.tiers.len())]
            .iter()
            .rev()
            .find_map(|t| t.speed_penalty)
            .unwrap_or(self.speed_penalty)
    }
}
