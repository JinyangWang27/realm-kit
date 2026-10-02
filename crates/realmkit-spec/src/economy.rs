//! Currency, trade goods, the producers that make them and the markets whose
//! prices follow what each place makes and needs.

use crate::*;

/// The most currency anyone holds, and the most an effect moves at once.
pub const CURRENCY_BOUND: u64 = 1_000_000_000_000;
/// The dearest base price of one unit.
pub const PRICE_BOUND: u64 = 1_000_000;
/// The highest price index allowed; 1,000 is the base price.
pub const INDEX_BOUND: u32 = 100_000;
/// The most units, producers or demand anything authors.
pub const QUANTITY_BOUND: u64 = 10_000;
/// The most units one command buys or sells.
pub const TRADE_BOUND: u64 = 1_000;
/// The price index of a good at its base price.
pub const BASE_INDEX: u32 = 1_000;
/// The most a market makes or uses up of one good on a tick, so a tick's
/// draws stay far inside 64 bits.
pub const SUPPLY_BOUND: u64 = 100_000_000;

/// Optional economy: currency and markets. Every price comes from production.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Economy {
    pub currency: Currency,
    /// Trade goods, in the order price ticks visit them.
    pub goods: Vec<Good>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub producers: Vec<Producer>,
    /// Markets, in the order price ticks visit them.
    pub markets: Vec<Market>,
    /// Markets whose prices pull together on each price tick.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub links: Vec<TradeLink>,
    /// Price indices stay within these, in thousandths of the base price.
    pub index_bounds: [u32; 2],
    /// Buying costs this much over the price, selling pays this much under it.
    pub spread_percent: u32,
    /// Each unit bought raises the index by this much, and each unit sold
    /// lowers it by the same, so trading back and forth never pays.
    pub trade_step: u32,
    /// Prices move on this schedule; without one, only trade moves them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tick: Option<PriceTick>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Currency {
    /// How an amount is shown; `{amount}` is the number.
    pub format: TextTemplate,
    /// What a new playthrough starts with.
    #[serde(default)]
    pub start: u64,
}

/// A good traded in every market, as one counted item.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Good {
    pub item: Id,
    /// The price of one unit at index 1,000.
    pub price: u64,
    /// Units consumed on each price tick by each kind of market.
    #[serde(default)]
    pub demand: BTreeMap<MarketKind, u64>,
    /// The good it is made from; while that is dearer, this one's price is
    /// pulled up towards it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input: Option<Id>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum MarketKind {
    Town,
    Village,
}

/// A kind of producer, such as grain fields or looms: what one unit makes
/// and uses up on each price tick.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Producer {
    pub id: Id,
    pub name: String,
    #[serde(default)]
    pub yields: BTreeMap<Id, u64>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub consumes: BTreeMap<Id, u64>,
}

/// Trade at a location.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Market {
    pub location: Id,
    pub kind: MarketKind,
    /// Trading needs this character present, if set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub merchant: Option<Id>,
    /// How many of each producer the market has.
    #[serde(default)]
    pub producers: BTreeMap<Id, u64>,
    /// Starting price indices; a good left out starts at 1,000.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub prices: BTreeMap<Id, u32>,
    /// Replaces the economy's spread here.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spread_percent: Option<u32>,
    /// Items sold here at a fixed price, buy-only and never running out.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub wares: Vec<Ware>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TradeLink {
    pub between: [Id; 2],
    /// Each side moves this share of the gap towards the other on a tick.
    pub percent: u32,
}

/// An item a market sells at a fixed price per unit.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Ware {
    pub item: Id,
    pub price: u64,
}

/// The recurring price update, in four phases over every market.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PriceTick {
    pub schedule: Schedule,
    /// A surplus or shortage moves an index by a draw below this many
    /// thousandths per unit of it.
    pub supply_step: u32,
    /// Below this index a surplus lowers prices proportionally less.
    pub damp_below: u32,
    /// Each tick an index moves this share of the way back to 1,000.
    pub revert_percent: u32,
    /// A good dearer to make than to buy is pulled this share towards its input.
    #[serde(default)]
    pub input_pull_percent: u32,
}

impl Economy {
    pub fn good(&self, item: &str) -> Option<&Good> {
        self.goods.iter().find(|g| g.item == item)
    }
    pub fn market(&self, location: &str) -> Option<&Market> {
        self.markets.iter().find(|m| m.location == location)
    }
    /// The ware a market sells as `item`, if any.
    pub fn ware<'m>(&self, market: &'m Market, item: &str) -> Option<&'m Ware> {
        market.wares.iter().find(|w| w.item == item)
    }
    pub fn producer(&self, id: &str) -> Option<&Producer> {
        self.producers.iter().find(|p| p.id == id)
    }
    /// The spread at a market.
    pub fn spread(&self, market: &Market) -> u32 {
        market.spread_percent.unwrap_or(self.spread_percent)
    }
    /// What a market makes and uses up of a good on one tick, at price
    /// `index`: its producers' yields, and its kind's demand plus what its
    /// producers consume. Producers make do with less of a dear good: while
    /// the index is above 1,000, their consumption is scaled by 1,000 ÷
    /// index, rounded down once. Saturating; validation keeps both within
    /// [`SUPPLY_BOUND`] at the base price, where consumption is greatest.
    pub fn supply(&self, market: &Market, good: &Good, index: u32) -> (u64, u64) {
        let (mut made, mut industry) = (0_u64, 0_u64);
        for (id, count) in &market.producers {
            let Some(producer) = self.producer(id) else {
                continue;
            };
            let per = |units: &BTreeMap<Id, u64>| {
                count.saturating_mul(units.get(&good.item).copied().unwrap_or(0))
            };
            made = made.saturating_add(per(&producer.yields));
            industry = industry.saturating_add(per(&producer.consumes));
        }
        if index > BASE_INDEX {
            industry = industry.saturating_mul(u64::from(BASE_INDEX)) / u64::from(index);
        }
        let demand = good.demand.get(&market.kind).copied().unwrap_or(0);
        (made, demand.saturating_add(industry))
    }
}
