//! Currency, goods, producers, markets and the price tick.

use super::*;

/// Currency content needs the world's `economy` block.
pub(super) fn needed(out: &mut Vec<Diagnostic>, w: &WorldSpec, owner: &str) {
    if w.economy().is_none() {
        issue(
            out,
            owner,
            "economy_disabled",
            "this world has no economy block, so there is no currency",
        );
    }
}

/// An amount of currency an effect or condition names.
pub(super) fn amount(out: &mut Vec<Diagnostic>, w: &WorldSpec, owner: &str, amount: u64) {
    needed(out, w, owner);
    if amount == 0 || amount > CURRENCY_BOUND {
        issue(
            out,
            owner,
            "invalid_amount",
            format!("an amount of currency is 1 to {CURRENCY_BOUND}"),
        );
    }
}

fn bounded(out: &mut Vec<Diagnostic>, owner: &str, value: u64, bound: u64, what: &str) {
    if value > bound {
        issue(
            out,
            owner,
            "invalid_amount",
            format!("{what} is at most {bound}"),
        );
    }
}

pub(super) fn rules(out: &mut Vec<Diagnostic>, w: &WorldSpec, economy: &Economy) {
    let owner = &w.world.id;
    template(out, owner, &economy.currency.format.0, &["amount"]);
    if !economy.currency.format.0.contains("{amount}") {
        issue(
            out,
            owner,
            "invalid_template",
            "the currency format shows the amount with {amount}",
        );
    }
    bounded(
        out,
        owner,
        economy.currency.start,
        CURRENCY_BOUND,
        "the starting currency",
    );
    let [low, high] = economy.index_bounds;
    if !(1..=BASE_INDEX).contains(&low) || !(BASE_INDEX..=INDEX_BOUND).contains(&high) {
        issue(
            out,
            owner,
            "invalid_bounds",
            format!(
                "index bounds hold the base index {BASE_INDEX}, from 1 to at most {INDEX_BOUND}"
            ),
        );
    }
    bounded(out, owner, economy.spread_percent.into(), 1_000, "a spread");
    bounded(
        out,
        owner,
        economy.trade_step.into(),
        INDEX_BOUND.into(),
        "a trade step",
    );
    goods(out, w, economy);
    producers(out, economy);
    markets(out, w, economy);
    links(out, w, economy);
    no_round_trip(out, w, economy);
    prosperity(out, w, economy);
    stock(out, w, economy);
    workshops(out, w, economy);
    if let Some(trading) = &economy.trading {
        if trading.name.trim().is_empty() {
            issue(
                out,
                owner,
                "empty_name",
                "the trading proficiency needs a name",
            );
        }
        if !(1..=RANK_BOUND).contains(&trading.max)
            || u64::from(trading.narrow_percent) * u64::from(trading.max) > 100
        {
            issue(
                out,
                owner,
                "invalid_amount",
                format!("trading ranks 1 to {RANK_BOUND} narrow the spread by at most 100% in all"),
            );
        }
    }
    if let Some(tick) = &economy.tick {
        time::needed(out, w, owner);
        time::schedule(out, w, owner, &tick.schedule);
        bounded(out, owner, tick.supply_step.into(), 1_000, "a supply step");
        bounded(
            out,
            owner,
            tick.damp_below.into(),
            INDEX_BOUND.into(),
            "the damping level",
        );
        bounded(
            out,
            owner,
            tick.revert_percent.into(),
            100,
            "a revert share",
        );
        bounded(
            out,
            owner,
            tick.input_pull_percent.into(),
            100,
            "an input pull",
        );
    }
}

/// Trading back and forth never pays. One step moves the index both ways,
/// so any trades that end holding what the player started with return the
/// index to where it was, and the worst case is one unit traded across one
/// step at the lowest index: (100 + spread)² × low ≥ 10,000 × (low + step).
/// The price's own rounding only lowers a sale and raises a purchase. Steps
/// that differ either way let a large stack bought (or sold) at one end come
/// back at a profit, so the package has one step, not two. Without this, a
/// package could hand the player unlimited currency.
fn no_round_trip(out: &mut Vec<Diagnostic>, w: &WorldSpec, economy: &Economy) {
    let owner = &w.world.id;
    let low = u64::from(economy.index_bounds[0]);
    // Bounds without a lowest price are reported on their own.
    if low == 0 {
        return;
    }
    // The best trader pays the narrowest spread.
    let best = economy.trading.as_ref().map_or(0, |t| t.max);
    let spreads = std::iter::once(economy.spread_percent)
        .chain(economy.markets.iter().filter_map(|m| m.spread_percent));
    for authored in spreads {
        let spread = narrowed(economy, authored, best);
        let margin = (100 + u64::from(spread)).pow(2) * low;
        if margin < 10_000 * (low + u64::from(economy.trade_step)) {
            issue(
                out,
                owner,
                "invalid_spread",
                format!("a spread of {authored}% (narrowed to {spread}%) lets trading a unit back and forth turn a profit"),
            );
        }
    }
}

/// A spread narrowed by `rank` in trading, as the engine computes it.
fn narrowed(economy: &Economy, spread: u32, rank: u32) -> u32 {
    let narrow = economy.trading.as_ref().map_or(0, |t| t.narrow_percent);
    let kept = 100_u64.saturating_sub(u64::from(narrow) * u64::from(rank));
    (u64::from(spread) * kept / 100) as u32
}

/// Goods are counted items, each traded once, with a positive price.
fn goods(out: &mut Vec<Diagnostic>, w: &WorldSpec, economy: &Economy) {
    ids(out, "good", economy.goods.iter().map(|g| g.item.as_str()));
    for good in &economy.goods {
        counted(out, w, &good.item, &good.item, 1);
        if good.price == 0 || good.price > PRICE_BOUND {
            issue(
                out,
                &good.item,
                "invalid_amount",
                format!("a base price is 1 to {PRICE_BOUND}"),
            );
        }
        for demand in good.demand.values() {
            bounded(out, &good.item, *demand, QUANTITY_BOUND, "demand");
        }
        if let Some(input) = &good.input {
            reference(
                out,
                &good.item,
                "input good",
                input,
                economy.good(input).is_some(),
            );
            if input == &good.item {
                issue(
                    out,
                    &good.item,
                    "invalid_input",
                    "a good is not made from itself",
                );
            }
        }
    }
}

fn producers(out: &mut Vec<Diagnostic>, economy: &Economy) {
    ids(
        out,
        "producer",
        economy.producers.iter().map(|p| p.id.as_str()),
    );
    for producer in &economy.producers {
        if producer.name.trim().is_empty() {
            issue(out, &producer.id, "empty_name", "a producer needs a name");
        }
        for (good, units) in producer.yields.iter().chain(&producer.consumes) {
            reference(
                out,
                &producer.id,
                "good",
                good,
                economy.good(good).is_some(),
            );
            bounded(
                out,
                &producer.id,
                *units,
                QUANTITY_BOUND,
                "a producer's units",
            );
        }
    }
}

/// One market per location, its producers known, its prices in bounds, and
/// no more of a good made or used up on a tick than arithmetic allows.
fn markets(out: &mut Vec<Diagnostic>, w: &WorldSpec, economy: &Economy) {
    ids(
        out,
        "market",
        economy.markets.iter().map(|m| m.location.as_str()),
    );
    let [low, high] = economy.index_bounds;
    for market in &economy.markets {
        let owner = &market.location;
        reference(out, owner, "location", owner, w.location(owner).is_some());
        if let Some(merchant) = &market.merchant {
            reference(
                out,
                owner,
                "merchant",
                merchant,
                w.character(merchant).is_some(),
            );
            // A merchant who is never at the market would keep it shut.
            let placed = w
                .location(owner)
                .is_some_and(|l| l.characters.contains(merchant));
            let visits = w
                .character(merchant)
                .and_then(|c| c.moves.as_ref())
                .is_some_and(|m| m.among.contains(owner));
            let known = w.character(merchant).is_some() && w.location(owner).is_some();
            if known && !placed && !visits {
                issue(
                    out,
                    owner,
                    "invalid_merchant",
                    format!("merchant {merchant} is never at this market"),
                );
            }
        }
        if let Some(spread) = market.spread_percent {
            bounded(out, owner, spread.into(), 1_000, "a spread");
        }
        for (id, count) in &market.producers {
            reference(out, owner, "producer", id, economy.producer(id).is_some());
            bounded(out, owner, *count, QUANTITY_BOUND, "a producer count");
        }
        for (good, index) in &market.prices {
            reference(out, owner, "good", good, economy.good(good).is_some());
            if !(low..=high).contains(index) {
                issue(
                    out,
                    owner,
                    "invalid_price",
                    format!("{good}'s starting index is outside the index bounds"),
                );
            }
        }
        let mut sold = BTreeSet::new();
        for ware in &market.wares {
            reference(out, owner, "item", &ware.item, w.item(&ware.item).is_some());
            if !(1..=PRICE_BOUND).contains(&ware.price) {
                issue(
                    out,
                    owner,
                    "invalid_price",
                    format!("{} costs 1 to {PRICE_BOUND}", ware.item),
                );
            }
            if economy.good(&ware.item).is_some() || !sold.insert(&ware.item) {
                issue(
                    out,
                    owner,
                    "invalid_ware",
                    format!(
                        "{} is sold here once, and is not also a trade good",
                        ware.item
                    ),
                );
            }
        }
        if let Some(start) = market.prosperity {
            if economy.prosperity.is_none() {
                issue(
                    out,
                    owner,
                    "invalid_amount",
                    "a starting prosperity needs the economy's prosperity block",
                );
            }
            bounded(
                out,
                owner,
                start.into(),
                PROSPERITY_BOUND.into(),
                "prosperity",
            );
        }
        if let Some(town) = &market.town {
            let valid = market.kind == MarketKind::Village
                && economy
                    .market(town)
                    .is_some_and(|t| t.kind == MarketKind::Town);
            if !valid {
                issue(
                    out,
                    owner,
                    "invalid_town",
                    format!(
                        "only a village names its market town, and {town} must be a town market"
                    ),
                );
            }
        }
        // Demand peaks at whichever end of the prosperity range scales it most.
        let peak = economy.prosperity.map_or(0, |p| {
            if p.demand_percent[0] >= p.demand_percent[1] {
                0
            } else {
                PROSPERITY_BOUND
            }
        });
        for good in &economy.goods {
            let (mut made, mut used) = economy.supply(market, good, BASE_INDEX, peak);
            if market.kind == MarketKind::Town {
                for village in economy.villages(&market.location) {
                    let (m, u) = economy.supply(village, good, BASE_INDEX, peak);
                    made = made.saturating_add(m);
                    used = used.saturating_add(u);
                }
            }
            if made > SUPPLY_BOUND || used > SUPPLY_BOUND {
                issue(
                    out,
                    owner,
                    "invalid_amount",
                    format!(
                        "at most {SUPPLY_BOUND} of {} is made or used here per tick",
                        good.item
                    ),
                );
            }
        }
    }
}

/// Links join two different markets once, and one market's shares add up to
/// at most 100%, so a price never passes all of its neighbours.
fn links(out: &mut Vec<Diagnostic>, w: &WorldSpec, economy: &Economy) {
    let owner = &w.world.id;
    let mut pairs = BTreeSet::new();
    let mut shares: BTreeMap<&str, u64> = BTreeMap::new();
    for link in &economy.links {
        let [a, b] = &link.between;
        for end in [a, b] {
            reference(
                out,
                owner,
                "linked market",
                end,
                economy.market(end).is_some(),
            );
            *shares.entry(end).or_default() += u64::from(link.percent);
        }
        if a == b || !pairs.insert((a.min(b), a.max(b))) {
            issue(
                out,
                owner,
                "invalid_link",
                format!("a trade link joins two different markets once: {a}, {b}"),
            );
        }
        if link.percent == 0 || link.percent > 100 {
            issue(
                out,
                owner,
                "invalid_link",
                "a link's share is 1 to 100 percent",
            );
        }
    }
    for (market, total) in shares {
        if total > 100 {
            issue(
                out,
                market,
                "invalid_link",
                "a market's links share at most 100 percent in total",
            );
        }
    }
}

fn prosperity(out: &mut Vec<Diagnostic>, w: &WorldSpec, economy: &Economy) {
    let Some(rules) = &economy.prosperity else {
        return;
    };
    let owner = &w.world.id;
    time::needed(out, w, owner);
    time::schedule(out, w, owner, &rules.schedule);
    let bound = u64::from(PROSPERITY_BOUND);
    bounded(out, owner, rules.base.into(), bound, "prosperity");
    bounded(out, owner, rules.scarcity.into(), bound, "a scarcity");
    let [low, high] = economy.index_bounds;
    if !(low..=high).contains(&rules.scarce_above) {
        issue(
            out,
            owner,
            "invalid_amount",
            "the scarcity level is within the index bounds",
        );
    }
    for percent in rules.demand_percent {
        bounded(out, owner, percent.into(), 1_000, "a demand percentage");
    }
}

/// Restocks stay within the stock and currency bounds at any prosperity.
fn stock(out: &mut Vec<Diagnostic>, w: &WorldSpec, economy: &Economy) {
    let Some(rules) = &economy.stock else {
        return;
    };
    let owner = &w.world.id;
    time::needed(out, w, owner);
    time::schedule(out, w, owner, &rules.schedule);
    for percent in rules.prosperity_percent {
        bounded(out, owner, percent.into(), 1_000, "a stock percentage");
    }
    let top = match economy.prosperity {
        Some(_) => *rules.prosperity_percent.iter().max().unwrap(),
        None => 100,
    };
    if economy.prosperity.is_none() && rules.prosperity_percent != [100, 100] {
        warn(
            out,
            owner,
            "unused_percent",
            "stock scales with prosperity, but the economy has no prosperity block",
        );
    }
    let most = |value: u64| u128::from(value) * u128::from(top) / 100;
    if most(rules.units) > u128::from(STOCK_BOUND / 2) {
        issue(
            out,
            owner,
            "invalid_amount",
            format!(
                "a restock spreads at most {} units at any prosperity",
                STOCK_BOUND / 2
            ),
        );
    }
    if most(rules.currency) > u128::from(CURRENCY_BOUND) {
        issue(
            out,
            owner,
            "invalid_amount",
            format!("a purse holds at most {CURRENCY_BOUND} at any prosperity"),
        );
    }
}

/// Workshops make a trade good from trade goods, and never pay for
/// themselves by being bought and sold back.
fn workshops(out: &mut Vec<Diagnostic>, w: &WorldSpec, economy: &Economy) {
    let Some(rules) = &economy.workshops else {
        return;
    };
    let owner = &w.world.id;
    time::needed(out, w, owner);
    time::schedule(out, w, owner, &rules.schedule);
    if !(1..=WORKSHOP_BOUND).contains(&rules.limit) {
        issue(
            out,
            owner,
            "invalid_limit",
            format!("a town holds 1 to {WORKSHOP_BOUND} workshops"),
        );
    }
    ids(out, "workshop", rules.kinds.iter().map(|k| k.id.as_str()));
    for kind in &rules.kinds {
        let owner = &kind.id;
        if kind.name.trim().is_empty() {
            issue(out, owner, "empty_name", "a workshop needs a name");
        }
        for (good, units) in std::iter::once((&kind.good, &kind.output)).chain(&kind.inputs) {
            reference(out, owner, "good", good, economy.good(good).is_some());
            if *units == 0 || *units > QUANTITY_BOUND {
                issue(
                    out,
                    owner,
                    "invalid_amount",
                    format!("a workshop makes or uses 1 to {QUANTITY_BOUND} units of a good"),
                );
            }
        }
        for (value, what) in [
            (kind.price, "a workshop's price"),
            (kind.resale, "a workshop's resale"),
            (kind.overhead, "a workshop's overhead"),
        ] {
            bounded(out, owner, value, CURRENCY_BOUND, what);
        }
        if kind.resale > kind.price {
            issue(
                out,
                owner,
                "invalid_resale",
                "selling a workshop back pays at most its price",
            );
        }
    }
}

/// A workshop effect or condition names a kind the world authors.
pub(super) fn workshop(out: &mut Vec<Diagnostic>, w: &WorldSpec, owner: &str, kind: &str) {
    let Some(rules) = w.economy().and_then(|e| e.workshops.as_ref()) else {
        issue(
            out,
            owner,
            "workshops_disabled",
            "this world's economy has no workshops",
        );
        return;
    };
    reference(
        out,
        owner,
        "workshop",
        kind,
        rules.kinds.iter().any(|k| k.id == kind),
    );
}

/// A proficiency an effect or condition names is authored, and a rank is
/// within its bound.
pub(super) fn proficiency(
    out: &mut Vec<Diagnostic>,
    w: &WorldSpec,
    owner: &str,
    proficiency: Proficiency,
    rank: u32,
) {
    if w.proficiency_max(proficiency).is_none() {
        issue(
            out,
            owner,
            "proficiencies_disabled",
            format!("this world does not define the {proficiency:?} proficiency"),
        );
    }
    if !(1..=RANK_BOUND).contains(&rank) {
        issue(
            out,
            owner,
            "invalid_amount",
            format!("a proficiency rank is 1 to {RANK_BOUND}"),
        );
    }
}

/// Levels grant proficiency points only where a proficiency can take them.
pub(super) fn proficiency_points(out: &mut Vec<Diagnostic>, w: &WorldSpec) {
    let granted = w
        .combat()
        .is_some_and(|c| c.levels.iter().any(|l| l.proficiency_points > 0));
    let any = Proficiency::ALL
        .iter()
        .any(|p| w.proficiency_max(*p).is_some());
    if granted && !any {
        issue(
            out,
            &w.world.id,
            "proficiencies_disabled",
            "levels grant proficiency points, but the world defines no proficiency",
        );
    }
    let total: u64 = w.combat().map_or(0, |c| {
        c.levels
            .iter()
            .map(|l| u64::from(l.proficiency_points))
            .sum()
    });
    if total > u64::from(u32::MAX) {
        issue(
            out,
            &w.world.id,
            "invalid_points",
            "levels grant too many proficiency points in all",
        );
    }
}
