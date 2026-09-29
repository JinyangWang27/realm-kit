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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Condition {
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
    #[serde(default)]
    pub requires: Vec<Condition>,
    pub effect: Option<DialogueEffect>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DialogueEffect {
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
}
