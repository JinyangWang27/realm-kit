//! Versioned, static world content. No game state or gameplay rules execute here.

use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::Path};

mod validation;
pub use validation::{Diagnostic, Severity, SpecError};

pub type Id = String;
pub const FORMAT_VERSION: u32 = 2;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WorldSpec {
    pub world: World,
    pub locations: Vec<Location>,
    pub characters: Vec<Character>,
    pub items: Vec<Item>,
    pub quests: Vec<Quest>,
    pub dialogues: Vec<Dialogue>,
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
    /// The player-controlled character.
    pub player: Id,
    #[serde(default)]
    pub flags: Vec<Id>,
    /// Absent in a world without fighting; then there is no XP or level either.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub combat: Option<Combat>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Combat {
    pub levels: Vec<Level>,
    pub narrative: Narrative,
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
    pub characters: Vec<Id>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Exit {
    pub destination: Id,
    #[serde(default)]
    pub requires: Vec<Condition>,
    pub blocked_text: String,
}

/// Anyone in the world. Talking and fighting are optional components.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Character {
    pub id: Id,
    pub name: String,
    pub description: String,
    /// Conditions for the character to be present where it is placed.
    #[serde(default)]
    pub requires: Vec<Condition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dialogue: Option<Id>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub combat: Option<CombatProfile>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CombatProfile {
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
    #[serde(default)]
    pub reward_xp: u64,
    #[serde(default)]
    pub reward_items: Vec<ItemStack>,
    #[serde(default)]
    pub completion_flags: Vec<Id>,
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
        // Check the version before the typed parse, which would fail on older
        // packages' fields with an obscure JSON error.
        let header: serde_json::Value = read_json(directory, "world.json")?;
        let found = header
            .get("format_version")
            .and_then(serde_json::Value::as_u64);
        if found != Some(FORMAT_VERSION.into()) {
            return Err(SpecError::UnsupportedFormat { found });
        }
        let world = Self {
            world: serde_json::from_value(header).map_err(|source| SpecError::Json {
                path: directory.join("world.json"),
                source,
            })?,
            locations: read_json(directory, "locations.json")?,
            characters: read_json(directory, "characters.json")?,
            items: read_json(directory, "items.json")?,
            quests: read_json(directory, "quests.json")?,
            dialogues: read_json(directory, "dialogues.json")?,
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

    /// Identifies this exact content. Saves bind to it, so any edit makes older
    /// saves incompatible until a migration exists.
    // ponytail: 64-bit FNV-1a over canonical JSON detects edits, not tampering;
    // switch to SHA-256 if revisions must be adversarially unique.
    pub fn revision(&self) -> String {
        let bytes = serde_json::to_vec(self).expect("world specs always serialize");
        let hash = bytes.iter().fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3)
        });
        format!("fnv1a64:{hash:016x}")
    }

    // ponytail: linear lookup suits small authored worlds; index IDs if profiling warrants it.
    pub fn location(&self, id: &str) -> Option<&Location> {
        self.locations.iter().find(|v| v.id == id)
    }
    pub fn character(&self, id: &str) -> Option<&Character> {
        self.characters.iter().find(|v| v.id == id)
    }
    pub fn combat(&self) -> Option<&Combat> {
        self.world.combat.as_ref()
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
