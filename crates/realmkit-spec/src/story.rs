//! Quests, dialogue, and the conditions content is gated on.

use crate::*;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Quest {
    pub id: Id,
    pub name: String,
    pub giver: Id,
    pub objective: QuestObjective,
    pub introduction: String,
    pub progress: String,
    pub completion: String,
    #[serde(default)]
    pub reward_xp: u64,
    #[serde(default)]
    pub reward_items: Vec<ItemStack>,
    #[serde(default)]
    pub completion_flags: Vec<Id>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reward_techniques: Vec<TechniqueGrant>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum QuestObjective {
    Defeat { character: Id },
    Flag { flag: Id },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum QuestStatus {
    Available,
    Active,
    Ready,
    Completed,
}

/// A pure query over the playthrough: typed leaf predicates composed with
/// `all`, `any` and `not`. Evaluating one never changes anything.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Condition {
    /// Every condition holds.
    All {
        of: Vec<Condition>,
    },
    /// At least one condition holds.
    Any {
        of: Vec<Condition>,
    },
    Not {
        condition: Box<Condition>,
    },
    Flag {
        flag: Id,
    },
    Quest {
        quest: Id,
        status: QuestStatus,
    },
    /// The player has learned `technique` at least to `rank` (1-based).
    Technique {
        technique: Id,
        rank: usize,
    },
    /// The player carries at least `quantity` of a counted item.
    Item {
        item: Id,
        quantity: u64,
    },
    /// The minute of the day is in `from..to`; past midnight when `from > to`.
    TimeOfDay {
        from: u64,
        to: u64,
    },
    /// The player holds at least this much currency.
    Currency {
        amount: u64,
    },
    /// The player owns a workshop of this kind, at `location` if given.
    Workshop {
        workshop: Id,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        location: Option<Id>,
    },
    /// The player's rank in a proficiency is at least `rank`.
    Proficiency {
        proficiency: Proficiency,
        rank: u32,
    },
}

impl Condition {
    /// Whether this condition can only hold while `leaf` holds: `leaf` is
    /// required on every branch, never under a `not`.
    pub fn requires(&self, leaf: &dyn Fn(&Condition) -> bool) -> bool {
        match self {
            Self::All { of } => of.iter().any(|c| c.requires(leaf)),
            Self::Any { of } => !of.is_empty() && of.iter().all(|c| c.requires(leaf)),
            Self::Not { .. } => false,
            _ => leaf(self),
        }
    }

    /// Every leaf predicate, depth first.
    pub fn leaves(&self) -> Vec<&Condition> {
        match self {
            Self::All { of } | Self::Any { of } => of.iter().flat_map(|c| c.leaves()).collect(),
            Self::Not { condition } => condition.leaves(),
            _ => vec![self],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Dialogue {
    pub id: Id,
    pub start: Id,
    pub nodes: Vec<DialogueNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DialogueNode {
    pub id: Id,
    pub text: String,
    #[serde(default)]
    pub choices: Vec<DialogueChoice>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DialogueChoice {
    pub text: String,
    pub next: Option<Id>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requires: Option<Condition>,
    /// Applied in order; if one fails, the choice changes nothing.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub effects: Vec<Effect>,
}

/// A typed state change. Effects in one list apply in authored order, and the
/// whole transition commits or fails together.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Effect {
    AcceptQuest {
        quest: Id,
    },
    CompleteQuest {
        quest: Id,
    },
    SetFlag {
        flag: Id,
    },
    /// A master, manual or chance encounter teaches or deepens a technique.
    GrantTechnique(TechniqueGrant),
    /// The player receives items; equipment arrives as individual pieces.
    GrantItems {
        items: Vec<ItemStack>,
    },
    /// The player hands over counted items; without enough, nothing happens.
    TakeItems {
        items: Vec<ItemStack>,
    },
    GrantCurrency {
        amount: u64,
    },
    /// The player pays; without enough, nothing happens.
    PayCurrency {
        amount: u64,
    },
    /// The player buys a workshop of this kind in the town where they stand.
    BuyWorkshop {
        workshop: Id,
    },
    /// The player sells back a workshop of this kind where they stand.
    SellWorkshop {
        workshop: Id,
    },
    /// A teacher or a book raises a proficiency without spending points.
    RaiseProficiency {
        proficiency: Proficiency,
        ranks: u32,
    },
}

/// A question the route asks at New Game, before the first turn, such as the
/// player's background. Every question is asked, in order.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct StartQuestion {
    pub id: Id,
    /// A short label for the answer once given, such as "Background".
    pub name: String,
    pub text: String,
    pub options: Vec<StartOption>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct StartOption {
    pub id: Id,
    pub text: String,
    /// Applied in order to the starting state; nothing re-runs them.
    #[serde(default)]
    pub effects: Vec<Effect>,
}
