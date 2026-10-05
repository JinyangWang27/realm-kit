//! What clients ask for and what the engine reports back.

use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
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
    /// Raises soldiers of a line from this location's pool.
    Recruit {
        line: Id,
        quantity: u64,
    },
    /// Turns last-level soldiers of a line into level-1 soldiers of a branch.
    Upgrade {
        line: Id,
        to: Id,
        quantity: u64,
    },
    /// Shows the roster; only in worlds with troops.
    Retinue,
    /// In a battle: fight one round under this order.
    Order(BattleOrder),
    /// In a battle: charge every remaining round.
    Autoresolve,
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
    /// Spends unspent proficiency points on one proficiency.
    Train {
        proficiency: Proficiency,
        points: u32,
    },
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
    /// Shows the overland map; only in worlds whose places have positions.
    Map,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
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
    Recruited {
        line: Id,
        quantity: u64,
        cost: u64,
    },
    Upgraded {
        line: Id,
        to: Id,
        quantity: u64,
        cost: u64,
    },
    /// A squad rose to `level` together.
    Promoted {
        line: Id,
        level: usize,
        count: u64,
    },
    WagesPaid {
        amount: u64,
    },
    /// Soldiers left over unpaid wages.
    Deserted {
        line: Id,
        level: usize,
        count: u64,
    },
    /// Wounded soldiers fit to fight again.
    Recovered {
        line: Id,
        level: usize,
        count: u64,
    },
    RetinueViewed,
    BattleStarted {
        army: Id,
        allies: Vec<Id>,
    },
    /// One round fought: each side's strength, losses and morale after it.
    BattleRound {
        round: u32,
        strengths: [u64; 2],
        losses: [u64; 2],
        morale: [u32; 2],
    },
    /// The winning side, 0 or 1, cut down `losses` of a routed or retreating enemy.
    Pursuit {
        by: usize,
        losses: u64,
    },
    /// The player's losses by line and level, split into wounded and killed;
    /// any rewards follow a victory.
    BattleEnded {
        outcome: BattleOutcome,
        wounded: Vec<(Id, usize, u64)>,
        killed: Vec<(Id, usize, u64)>,
    },
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
    /// The player trained a proficiency up to `rank`.
    ProficiencyTrained {
        proficiency: Proficiency,
        rank: u32,
    },
    /// An effect taught a proficiency up to `rank`.
    ProficiencyRaised {
        proficiency: Proficiency,
        rank: u32,
    },
    WorkshopBought {
        workshop: Id,
        location: Id,
        cost: u64,
    },
    WorkshopSold {
        workshop: Id,
        location: Id,
        earned: u64,
    },
    /// A settlement: the player's workshops earned `amount` in all, and
    /// `forgone` more that would have passed the currency bound.
    WorkshopsEarned {
        amount: u64,
        forgone: u64,
    },
    /// A settlement: the workshops lost money; `amount` was paid and
    /// `shortfall` could not be.
    WorkshopsLost {
        amount: u64,
        shortfall: u64,
    },
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
    EvidenceDiscovered {
        evidence: Id,
    },
    /// Something learned since revealed another fact of known evidence: its
    /// fact `fact` (0-based) is now known, for good.
    EvidenceFactLearned {
        evidence: Id,
        fact: usize,
    },
    /// Something learned since changed how known evidence reads: its
    /// current interpretation is now `reading` (0-based). The facts are as
    /// they were.
    EvidenceReinterpreted {
        evidence: Id,
        reading: usize,
    },
    /// The route reached this authored outcome; play may go on.
    OutcomeReached {
        outcome: Id,
    },
    /// The story moved on to this phase.
    PhaseEntered {
        phase: Id,
    },
    InventoryViewed,
    StatusViewed,
    QuestsViewed,
    MapViewed,
}

/// The player's order for one round of a battle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BattleOrder {
    Charge,
    /// Melee troops brace: every melee hit either side deals is weakened.
    Hold,
    /// Mounted troops leave the line to ride down the enemy's archers.
    Flank,
    /// Leave the field, giving the enemy a pursuit and the battle.
    Retreat,
}

/// How a battle ended for the player's side.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BattleOutcome {
    Victory,
    Defeat,
    Draw,
}

/// How an encounter ended; the player's death leaves it open instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    /// Every opponent died or yielded.
    Victory,
    /// The player yielded.
    Yielded,
    Fled,
}

/// A command a client may offer in the current scene. Unavailable actions are
/// shown for explanation; the engine still rechecks legality on execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Action {
    pub command: Command,
    pub available: bool,
}

/// What the player may know of the map: the places they know of (where they
/// are, and each place whose `known_when` holds), the roads and exits between
/// those, and where the player is. Characters who move are left out, since a
/// map would show where they are now, not where they were last seen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MapView {
    pub here: Id,
    /// In authored order.
    pub places: Vec<MapPlace>,
    /// In authored order.
    pub roads: Vec<MapRoad>,
    /// One-way links, each drawn towards its destination.
    pub exits: Vec<MapExit>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MapPlace {
    pub location: Id,
    pub point: MapPoint,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MapRoad {
    pub road: Id,
    pub between: [Id; 2],
    /// The player's travel time along it.
    pub minutes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MapExit {
    pub from: Id,
    pub to: Id,
    pub direction: Direction,
}

/// What the player's journal shows: the story's phase, the quests the
/// player knows of, the evidence discovered and the outcome reached.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Journal {
    /// The current phase; `None` in a world without phases.
    pub phase: Option<Id>,
    /// Main quests, then side quests, each in authored order. An available
    /// quest whose prerequisites do not hold yet is left out.
    pub quests: Vec<JournalQuest>,
    /// Known evidence, in authored order.
    pub evidence: Vec<Id>,
    pub outcome: Option<Id>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JournalQuest {
    pub quest: Id,
    pub main: bool,
    pub status: QuestStatus,
}
