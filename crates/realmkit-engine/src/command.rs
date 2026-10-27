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
    /// Host-reported world time: this many minutes pass while the player
    /// goes on being where they are, as [`Command::Wait`] passes them but
    /// whether or not the world authors waiting, and leaving any
    /// conversation open. For a continuous host (a graphical client whose
    /// player is in the world as time goes by), which decides how much real
    /// time is worth a minute; never offered as an action.
    PassTime(u64),
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
    /// Moves carried currency into the bank; only at a branch.
    Deposit(u64),
    /// Moves currency from the bank into the player's purse; only at a branch.
    Withdraw(u64),
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
    /// Starts a real-time fight with a fighter here and whoever engaging it
    /// would bring in: one opponent, or its whole group.
    Duel(Id),
    /// In a real-time fight: one input on its fixed-step clock.
    Intent(Intent),
    /// Gives a named companion in the party a standing tactical order,
    /// while exploring or in a real-time fight. An order changes what the
    /// companion aims its next attacks at; it never starts or lands one.
    Tactic {
        companion: Id,
        order: TacticalOrder,
    },
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

/// Input to a real-time fight, applied at the fight's current step: the
/// player's own actions, and what only the host can see (contact and
/// arrivals). A host collects input between steps and submits it in order;
/// replaying the same intents from the same state gives the same fight.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Intent {
    /// Runs the clock on by 1 to [`realmkit_spec::ADVANCE_BOUND`] steps,
    /// stopping early if the fight ends. Splitting an advance anywhere gives
    /// the same fight.
    Advance(u32),
    /// Begins a basic attack at a hostile fighter; `None` aims at the only
    /// one still fighting, and is refused when there are several.
    Attack {
        #[serde(default)]
        target: Option<Id>,
    },
    /// Begins a known skill's attack, paying its costs at once; `target` as
    /// for [`Intent::Attack`].
    Skill {
        skill: Id,
        #[serde(default)]
        target: Option<Id>,
    },
    /// Raises a guard, which stops hits once it is up.
    Guard,
    LowerGuard,
    /// Turns to retreat; the escape happens after the retreat's steps.
    Retreat,
    /// Begins the world's timed parry: contact during its window is
    /// deflected and staggers the attacker; after it, a committed recovery.
    Parry,
    /// Begins the world's timed dodge: contact during its window finds
    /// nothing; after it, a committed recovery. Moving is the host's.
    Dodge,
    /// The host reports that `attacker`'s attack touched `target` now
    /// (`None`: the fighter it was aimed at). The engine accepts it only
    /// while that attack is active and has not landed, on a fighter still
    /// fighting and, unless the world allows friendly fire, on another
    /// team; it decides the damage itself and cannot check the geometry.
    Contact {
        attacker: Id,
        #[serde(default)]
        target: Option<Id>,
    },
    /// The host reports that `character`, here, arrives on `team`: 0 for
    /// an ally, an opponent's team, or the next unused number for a new one.
    Join {
        character: Id,
        team: u32,
    },
}

/// A named companion's standing order: a high-level objective, never a
/// single attack. RealmKit has no geometry, so Follow and Hold name the
/// objective a host moves the companion by; the engine decides only whom
/// its attacks aim at. Without a valid preferred foe, every order falls
/// back to the companion's authored `targeting` over every foe it may aim
/// at, so an order never stops a fight.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TacticalOrder {
    /// Go with the player; aim by the authored `targeting`.
    #[default]
    Follow,
    /// Hold position: stay at the objective the host keeps, and strike only
    /// at foes whose attack is aimed at the companion.
    Hold,
    /// Protect `ally`, the player or another companion in the party: aim at
    /// a foe whose attack is aimed at them, else as Follow.
    Defend { ally: Id },
    /// Aim at `target`, a hostile fighter in the current real-time fight,
    /// while it still fights, else as Follow. Lapses to Follow when the
    /// fight ends.
    Attack { target: Id },
}

/// A dialogue choice as the conversation offers it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DialogueOption {
    pub text: String,
    /// Why it cannot be taken now; `None` when it can.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocked: Option<String>,
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
    /// In a real-time fight: `character`'s attack entered `phase`; a wind-up is its start.
    Swing {
        character: Id,
        skill: Option<Id>,
        phase: AttackPhase,
    },
    /// In a real-time fight: an attack's active window closed without contact.
    Missed {
        character: Id,
    },
    /// In a real-time fight: `character` finished recovering and can act again.
    Ready {
        character: Id,
    },
    /// In a real-time fight: `character`'s guard began rising, or is now up.
    GuardRaising {
        character: Id,
    },
    GuardUp {
        character: Id,
    },
    GuardLowered {
        character: Id,
    },
    /// In a real-time fight: a raised guard stopped `prevented` of a hit's damage.
    Blocked {
        character: Id,
        prevented: u32,
    },
    /// In a real-time fight: `character` arrived on `team` at this step.
    Joined {
        character: Id,
        team: u32,
    },
    /// In a real-time fight: a hit between two fighters neither of whom is
    /// the player, who sees [`Event::DamageDealt`] and
    /// [`Event::DamageReceived`] for their own.
    Struck {
        attacker: Id,
        target: Id,
        amount: u32,
        variant: usize,
        skill: Option<Id>,
    },
    /// In a real-time fight: a fighter on the player's team fell. One on
    /// another team is [`Event::EnemyDefeated`]; the player, [`Event::PlayerDied`].
    AllyDown {
        character: Id,
    },
    /// In a real-time fight: the player turned to retreat.
    RetreatStarted,
    /// In a real-time fight: `character` began a parry or a dodge.
    Parrying {
        character: Id,
    },
    Dodging {
        character: Id,
    },
    /// In a real-time fight: `character`'s parry deflected `attacker`'s blow.
    Parried {
        character: Id,
        attacker: Id,
    },
    /// In a real-time fight: `attacker`'s blow found nothing; `character`
    /// was dodging. The attack can still land while it is active.
    Evaded {
        character: Id,
        attacker: Id,
    },
    /// In a real-time fight: `character` was interrupted and is staggered.
    Staggered {
        character: Id,
    },
    /// A named companion joined the player's party.
    JoinedParty {
        character: Id,
    },
    /// A named companion left the party, staying where the player is.
    LeftParty {
        character: Id,
    },
    /// A companion took a standing order.
    OrderGiven {
        companion: Id,
        order: TacticalOrder,
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
    /// The roster's wages, `from_bank` of them out of the bank and the
    /// rest from carried currency.
    WagesPaid {
        amount: u64,
        from_bank: u64,
    },
    /// Wages fell due that the bank and carried currency together,
    /// `available`, could not cover, so none were paid; desertions follow.
    WagesUnpaid {
        amount: u64,
        available: u64,
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
    Deposited {
        amount: u64,
    },
    Withdrawn {
        amount: u64,
    },
    /// A Gregorian month ended at midnight; what settles for it follows:
    /// interest, then month-end wages.
    MonthEnded {
        year: i32,
        month: u8,
    },
    /// Interest on the month's `average` daily closing balance, rounded
    /// down, was paid into the bank; `forgone` more would have passed the
    /// currency bound.
    InterestCredited {
        average: u64,
        amount: u64,
        forgone: u64,
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
    /// The choices are numbered as `ChooseDialogue` takes them, unavailable
    /// ones included.
    Dialogue {
        npc: Id,
        node: Id,
        choices: Vec<DialogueOption>,
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
    /// The player's standing on `track`, with `faction` on a faction track,
    /// moved by `by` to `value`, after clamping to the track's bounds.
    StandingChanged {
        track: Id,
        faction: Option<Id>,
        by: i32,
        value: i32,
    },
    /// The two factions, in canonical order, went to war.
    WarDeclared {
        factions: [Id; 2],
    },
    /// The two factions, in canonical order, made peace.
    PeaceMade {
        factions: [Id; 2],
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
    /// The player's travel time along it; the shortest, for a road whose
    /// time varies.
    pub minutes: u64,
    /// The longest travel time of a road whose time varies, drawn only when
    /// the player sets out; `None` for a fixed road.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub longest: Option<u64>,
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
