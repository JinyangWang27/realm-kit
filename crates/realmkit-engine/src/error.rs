//! Why a command or a save was refused.

use super::*;

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error(transparent)]
    World(#[from] SpecError),
    #[error("there is no exit in that direction")]
    NoExit,
    #[error("that exit is locked")]
    ExitLocked { location: Id, direction: Direction },
    #[error("there is no road from here to {0}")]
    NoRoad(Id),
    #[error("that road is closed")]
    RoadBlocked { road: Id },
    #[error("time cannot be passed by waiting in this world")]
    NoWaiting,
    #[error("a wait lasts 1 to {} minutes", realmkit_spec::DURATION_BOUND)]
    InvalidWait,
    #[error("there is no market here")]
    NoMarket,
    #[error("{0} is not traded here")]
    NotTraded(Id),
    #[error("trade 1 to {} units at a time", realmkit_spec::TRADE_BOUND)]
    InvalidQuantity,
    #[error("you cannot afford that")]
    NotEnoughCurrency,
    #[error("there is no {0} left to buy here")]
    OutOfStock(Id),
    #[error("the merchants cannot pay that much")]
    MerchantCannotPay,
    #[error("workshops are bought in a town with a market")]
    NoWorkshopHere,
    #[error("you own as many workshops here as a town allows")]
    WorkshopLimit,
    #[error("you have no such workshop here")]
    NoWorkshop,
    #[error("this world has no such proficiency")]
    NoSuchProficiency,
    #[error("that proficiency cannot rise further")]
    ProficiencyCap,
    #[error("more than one outcome would be reached at once: {0:?}")]
    AmbiguousOutcome(Vec<Id>),
    #[error("an outcome holds before the first turn")]
    OutcomeAtStart,
    #[error("you cannot take up {0} yet")]
    QuestLocked(Id),
    #[error("{0} is not available here")]
    NotHere(Id),
    #[error("{0} has already been defeated")]
    AlreadyDefeated(Id),
    #[error("you do not know that skill: {0}")]
    UnknownSkill(Id),
    #[error("you have not reached the level for {0}")]
    SkillLocked(Id),
    #[error("not enough MP for {0}")]
    NotEnoughMp(Id),
    #[error("not enough rage for {0}")]
    NotEnoughRage(Id),
    #[error("finish the fight first")]
    InEncounter,
    #[error("there is no running from this fight")]
    NoFlee,
    #[error("you cannot spend points on that stat")]
    NoSuchStat,
    #[error("not enough unspent stat points")]
    NotEnoughPoints,
    #[error("that stat cannot take more points")]
    PointCap,
    #[error("stat points cannot be refunded here")]
    NoRespec,
    #[error("you have no equipment #{0}")]
    NoSuchGear(u64),
    #[error("#{0} is already equipped")]
    AlreadyEquipped(u64),
    #[error("#{0} is not equipped")]
    NotEquipped(u64),
    #[error("you do not know that recipe")]
    UnknownRecipe(Id),
    #[error("there is no {0} here")]
    NoStation(Id),
    #[error("you are not yet able to do that")]
    RequirementsUnmet,
    #[error("you do not have enough {0}")]
    NotEnoughMaterials(Id),
    #[error("you do not know that enchantment")]
    UnknownEnchantment(Id),
    #[error("#{0} is already enchanted")]
    AlreadyEnchanted(u64),
    #[error("{enchantment} does not fit #{gear}")]
    DoesNotFit { gear: u64, enchantment: Id },
    #[error("#{0} cannot be improved further")]
    NoHigherTier(u64),
    #[error("you are not fighting anyone; engage first")]
    NotFighting,
    #[error("this is not a safe place to rest")]
    NotSafe,
    #[error("{0} are not recruited here")]
    NotRecruitedHere(Id),
    #[error("no more {0} can be recruited here for now")]
    PoolEmpty(Id),
    #[error("only {left} {line} can be recruited here for now")]
    TooFewRecruits { line: Id, left: u64 },
    #[error("this world has no soldiers")]
    NoRetinue,
    #[error("this world has no map")]
    NoMap,
    #[error("{0} is on your side")]
    NotHostile(Id),
    #[error("you have no riders to send round the flank")]
    NoFlank,
    #[error("you are not leading a battle")]
    NotInBattle,
    #[error("your retinue is full")]
    RosterFull,
    #[error("you do not have that many {0} to upgrade")]
    NotEnoughTroops(Id),
    #[error("{0} cannot be upgraded into {1}")]
    NoSuchUpgrade(Id, Id),
    #[error("{0} cannot be used")]
    NotConsumable(Id),
    #[error("that would restore nothing")]
    NothingToRestore,
    #[error("there is no active conversation")]
    NoDialogue,
    #[error("choose one of the displayed options")]
    InvalidChoice,
    #[error("quest cannot be accepted or completed in its current state: {0}")]
    QuestState(Id),
    #[error("unknown quest: {0}")]
    UnknownQuest(Id),
    #[error("the player is dead; start a new game")]
    PlayerDead,
    #[error("numeric limit exceeded; command was not applied")]
    NumericLimit,
    #[error("answer each start question, in order, with one of its options")]
    StartChoices,
    #[error("save cannot be loaded: {0}")]
    InvalidSave(String),
}
