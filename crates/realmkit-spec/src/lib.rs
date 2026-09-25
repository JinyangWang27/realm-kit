//! Versioned, static world content. No game state or gameplay rules execute here.

use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::Path};

mod validation;
pub use validation::{Diagnostic, Severity, SpecError};

pub type Id = String;
pub const FORMAT_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WorldSpec {
    pub world: World,
    pub locations: Vec<Location>,
    pub npcs: Vec<Npc>,
    pub monsters: Vec<Monster>,
    pub items: Vec<Item>,
    pub quests: Vec<Quest>,
    pub dialogues: Vec<Dialogue>,
    pub narrative: Narrative,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct World {
    pub format_version: u32,
    pub id: Id,
    pub name: String,
    /// Language tag for authored player-facing content (for example "en" or "zh-Hans").
    pub language: String,
    pub start: Id,
    pub player_name: String,
    pub levels: Vec<Level>,
    #[serde(default)]
    pub flags: Vec<Id>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Level {
    /// Cumulative experience required for this level; level one starts at zero.
    pub xp: u64,
    pub hp: u32,
    pub attack: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    North,
    South,
    East,
    West,
    Up,
    Down,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Location {
    pub id: Id,
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub exits: BTreeMap<Direction, Exit>,
    #[serde(default)]
    pub npcs: Vec<Id>,
    #[serde(default)]
    pub monsters: Vec<Id>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Exit {
    pub destination: Id,
    #[serde(default)]
    pub requires: Vec<Condition>,
    pub blocked_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Npc {
    pub id: Id,
    pub name: String,
    pub description: String,
    pub dialogue: Id,
    #[serde(default)]
    pub requires: Vec<Condition>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Monster {
    pub id: Id,
    pub name: String,
    pub description: String,
    pub hp: u32,
    pub attack: u32,
    pub xp: u64,
    #[serde(default)]
    pub loot: Vec<ItemStack>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ItemStack {
    pub item: Id,
    pub quantity: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Item {
    pub id: Id,
    pub name: String,
    pub description: String,
}

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
    pub reward_xp: u64,
    #[serde(default)]
    pub reward_items: Vec<ItemStack>,
    #[serde(default)]
    pub completion_flags: Vec<Id>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum QuestObjective {
    Defeat { monster: Id },
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
    Flag { flag: Id },
    Quest { quest: Id, status: QuestStatus },
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
    AcceptQuest { quest: Id },
    CompleteQuest { quest: Id },
    SetFlag { flag: Id },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(transparent)]
pub struct TextTemplate(pub String);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Narrative {
    pub attack: Vec<TextTemplate>,
    pub hurt: Vec<TextTemplate>,
    pub victory: TextTemplate,
    pub death: String,
}

impl WorldSpec {
    pub fn load(directory: impl AsRef<Path>) -> Result<Self, SpecError> {
        let directory = directory.as_ref();
        let world = Self {
            world: read_json(directory, "world.json")?,
            locations: read_json(directory, "locations.json")?,
            npcs: read_json(directory, "npcs.json")?,
            monsters: read_json(directory, "monsters.json")?,
            items: read_json(directory, "items.json")?,
            quests: read_json(directory, "quests.json")?,
            dialogues: read_json(directory, "dialogues.json")?,
            narrative: read_json(directory, "narrative.json")?,
        };
        world.validate()?;
        Ok(world)
    }

    pub fn validate(&self) -> Result<(), SpecError> {
        let diagnostics = self.diagnostics();
        if diagnostics.iter().any(|d| d.severity == Severity::Error) {
            Err(SpecError::Validation(diagnostics))
        } else {
            Ok(())
        }
    }

    /// Collect all content errors for an author/validate/repair loop.
    pub fn diagnostics(&self) -> Vec<Diagnostic> {
        validation::diagnostics(self)
    }

    // ponytail: linear lookup suits small authored worlds; index IDs if profiling warrants it.
    pub fn location(&self, id: &str) -> Option<&Location> {
        self.locations.iter().find(|v| v.id == id)
    }
    pub fn npc(&self, id: &str) -> Option<&Npc> {
        self.npcs.iter().find(|v| v.id == id)
    }
    pub fn monster(&self, id: &str) -> Option<&Monster> {
        self.monsters.iter().find(|v| v.id == id)
    }
    pub fn item(&self, id: &str) -> Option<&Item> {
        self.items.iter().find(|v| v.id == id)
    }
    pub fn quest(&self, id: &str) -> Option<&Quest> {
        self.quests.iter().find(|v| v.id == id)
    }
    pub fn dialogue(&self, id: &str) -> Option<&Dialogue> {
        self.dialogues.iter().find(|v| v.id == id)
    }
}

fn read_json<T: serde::de::DeserializeOwned>(directory: &Path, file: &str) -> Result<T, SpecError> {
    let path = directory.join(file);
    let bytes = std::fs::read(&path).map_err(|source| SpecError::Io {
        path: path.clone(),
        source,
    })?;
    serde_json::from_slice(&bytes).map_err(|source| SpecError::Json { path, source })
}
