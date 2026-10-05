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
    /// Part of the route's main questline rather than a side story.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub main: bool,
    /// The condition to take the quest up, such as an earlier quest done.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requires: Option<Condition>,
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

/// A quest only ever moves forward through these, in order.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum QuestStatus {
    Available,
    Active,
    Ready,
    Completed,
}

/// A pure query over the playthrough: typed leaf predicates composed with
/// `all`, `any`, `at_least` and `not`. Evaluating one never changes anything.
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
    /// At least `count` of the conditions hold, such as any two of four clues.
    AtLeast {
        count: usize,
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
    /// The player has discovered this evidence.
    Evidence {
        evidence: Id,
    },
    /// The story has reached this phase, or a later one.
    Phase {
        phase: Id,
    },
}

impl Condition {
    /// Whether this condition can only hold while `leaf` holds: `leaf` is
    /// required on every branch. A `not` is offered to `leaf` whole; what is
    /// inside it is never required.
    pub fn requires(&self, leaf: &dyn Fn(&Condition) -> bool) -> bool {
        match self {
            Self::All { of } => of.iter().any(|c| c.requires(leaf)),
            Self::Any { of } => !of.is_empty() && of.iter().all(|c| c.requires(leaf)),
            // Every `count` of them include one that requires `leaf` when more
            // than `len - count` do.
            Self::AtLeast { count, of } => {
                let requiring = of.iter().filter(|c| c.requires(leaf)).count();
                *count > 0 && requiring > of.len().saturating_sub(*count)
            }
            _ => leaf(self),
        }
    }

    /// Whether this and `other` can never hold together: on every branch,
    /// one requires a condition the other requires to fail, or a different
    /// status of a quest the other requires.
    pub fn excludes(&self, other: &Condition) -> bool {
        // Whether `b` cannot hold alongside anything `a` requires.
        let refutes = |a: &Condition, b: &Condition| {
            let required: Vec<&Condition> = a
                .leaves()
                .into_iter()
                .filter(|l| a.requires(&|c| c == *l))
                .collect();
            b.requires(&|c| {
                required.iter().any(|l| match (c, l) {
                    (Self::Not { condition }, _) => **condition == **l,
                    (
                        Self::Quest { quest, status },
                        Self::Quest {
                            quest: q,
                            status: s,
                        },
                    ) => quest == q && status != s,
                    _ => false,
                })
            })
        };
        refutes(self, other) || refutes(other, self)
    }

    /// Every leaf predicate, depth first.
    pub fn leaves(&self) -> Vec<&Condition> {
        match self {
            Self::All { of } | Self::Any { of } | Self::AtLeast { of, .. } => {
                of.iter().flat_map(|c| c.leaves()).collect()
            }
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
    /// The player recognises a piece of evidence; once known, it stays known.
    DiscoverEvidence {
        evidence: Id,
    },
    /// The story moves on to a later phase; an earlier or the current one
    /// changes nothing.
    EnterPhase {
        phase: Id,
    },
}

/// A period of the story, such as the opening journey. Phases come in
/// authored order, the first current at the start; only story effects move
/// them on, never world time.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Phase {
    pub id: Id,
    pub name: String,
}

/// Something the player can learn in an investigation: an observation,
/// testimony or a physical clue. Knowing it is investigation state, separate
/// from carrying any item.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct EvidenceDefinition {
    pub id: Id,
    pub name: String,
    pub description: String,
    /// Where the player learns it, such as a ledger or a witness.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub source: String,
    /// What was observed or said. Later knowledge never changes these; it
    /// changes only how they are read.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub facts: Vec<String>,
    /// Ways of reading the facts, in the order understanding deepens. The
    /// current reading is the last whose `when` holds.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub interpretations: Vec<Interpretation>,
    /// The physical item this evidence concerns, if any. Carrying the item
    /// does not make the evidence known, nor the reverse.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item: Option<Id>,
}

/// One way of reading a piece of evidence, available once `when` holds.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Interpretation {
    /// Absent means from the moment the evidence is known. Only evidence,
    /// flags and phases may appear, never under `not`, so a reading once
    /// reached is never lost.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<Condition>,
    pub text: String,
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

/// An authored ending of the route, reached the moment its condition first
/// holds. A playthrough records at most one, and play may go on after it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RouteOutcome {
    pub id: Id,
    pub name: String,
    pub text: String,
    pub when: Condition,
}
