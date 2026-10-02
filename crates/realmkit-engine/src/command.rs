//! What clients ask for and what the engine reports back.

use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Look,
    Move(Direction),
    /// Takes the road from here to a location, passing its travel time.
    Travel(Id),
    /// Lets this many minutes of world time pass.
    Wait(u64),
    /// Buys units of a good at this location's market.
    Buy {
        good: Id,
        quantity: u64,
    },
    /// Sells units of a good at this location's market.
    Sell {
        good: Id,
        quantity: u64,
    },
    /// Shows this location's prices; only at a market.
    Market,
    /// Starts an encounter with a fighter at the current location.
    Engage(Id),
    /// In an encounter: a basic attack in the player's basic-attack channel.
    Attack(Id),
    UseSkill {
        skill: Id,
        target: Id,
    },
    /// In an encounter: spend this turn turning to run; the escape happens
    /// at the player's next turn if they live.
    Flee,
    /// Restores HP and MP at a safe location.
    Rest,
    /// Uses one carried consumable: restores HP and MP; in an encounter, it
    /// is the player's turn.
    Use(Id),
    /// Spends unspent stat points on one stat.
    Allocate {
        stat: Stat,
        points: u32,
    },
    /// Refunds every spent stat point, where the world allows it.
    Respec,
    /// Wears a piece of equipment, returning whatever held its slots to the pack.
    Equip(u64),
    Unequip(u64),
    /// Forges a new piece from a known recipe at its station.
    Forge(Id),
    /// Raises a piece one improvement tier at the tier's station.
    Improve(u64),
    /// Lays a known enchantment on an unenchanted piece it fits.
    Enchant {
        piece: u64,
        enchantment: Id,
    },
    Talk(Id),
    /// One-based index into the currently visible choices.
    ChooseDialogue(usize),
    AcceptQuest(Id),
    CompleteQuest(Id),
    Inventory,
    Status,
    Quests,
    /// Lists learned techniques and their ranks; only in worlds with techniques.
    Techniques,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    LocationViewed {
        location: Id,
    },
    Moved {
        from: Id,
        to: Id,
    },
    /// World time moved on; `now` is the minute it reached. `eventful` says
    /// whether anything the player notices happened meanwhile (a flag being
    /// set does not count); those events come just before.
    TimePassed {
        minutes: u64,
        now: u64,
        eventful: bool,
    },
    /// A character who moves came to the player's location.
    CharacterArrived {
        character: Id,
    },
    /// A character who moves left the player's location.
    CharacterLeft {
        character: Id,
    },
    Bought {
        good: Id,
        quantity: u64,
        cost: u64,
    },
    Sold {
        good: Id,
        quantity: u64,
        earned: u64,
    },
    CurrencyReceived {
        amount: u64,
    },
    CurrencyPaid {
        amount: u64,
    },
    MarketViewed,
    /// `skill` is `None` for a basic attack, which uses narrative `variant`.
    DamageDealt {
        target: Id,
        amount: u32,
        variant: usize,
        skill: Option<Id>,
        critical: bool,
    },
    DamageReceived {
        source: Id,
        amount: u32,
        variant: usize,
        skill: Option<Id>,
        critical: bool,
    },
    ResourceSpent {
        character: Id,
        resource: Resource,
        amount: u32,
    },
    EncounterStarted {
        opponents: Vec<Id>,
    },
    FleeStarted,
    /// A participant in a yielding group stops fighting, alive.
    Yielded {
        character: Id,
    },
    /// Any rewards follow a victory.
    EncounterEnded {
        outcome: Outcome,
    },
    Rested,
    /// One unit of `item` was used; `hp` and `mp` are what it restored.
    Consumed {
        item: Id,
        hp: u32,
        mp: u32,
    },
    PointsAllocated {
        stat: Stat,
        points: u32,
    },
    PointsRefunded,
    TechniqueLearned {
        technique: Id,
    },
    /// `rank` is 1-based; its authored name is shown.
    TechniqueRankUp {
        technique: Id,
        rank: usize,
    },
    TechniqueXpGained {
        technique: Id,
        amount: u64,
    },
    TechniquesViewed,
    Equipped {
        gear: u64,
    },
    Unequipped {
        gear: u64,
    },
    /// Materials used up by forging or improving.
    ItemsSpent {
        item: Id,
        quantity: u64,
    },
    Forged {
        recipe: Id,
        gear: u64,
    },
    Improved {
        gear: u64,
        tier: usize,
    },
    Enchanted {
        gear: u64,
        enchantment: Id,
    },
    EnemyDefeated {
        monster: Id,
    },
    ItemReceived {
        item: Id,
        quantity: u64,
    },
    ExperienceGranted {
        amount: u64,
    },
    /// Levelling up restores HP, and MP when the player has any.
    LevelUp {
        level: usize,
        mp_restored: bool,
    },
    PlayerDied,
    Dialogue {
        npc: Id,
        node: Id,
        choices: Vec<String>,
    },
    DialogueEnded,
    QuestAccepted {
        quest: Id,
    },
    QuestProgressed {
        quest: Id,
    },
    QuestCompleted {
        quest: Id,
    },
    StoryFlagSet {
        flag: Id,
    },
    InventoryViewed,
    StatusViewed,
    QuestsViewed,
}

/// How an encounter ended; the player's death leaves it open instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// Every opponent died or yielded.
    Victory,
    /// The player yielded.
    Yielded,
    Fled,
}

/// A command a client may offer in the current scene. Unavailable actions are
/// shown for explanation; the engine still rechecks legality on execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Action {
    pub command: Command,
    pub available: bool,
}
