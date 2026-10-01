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
    for step in [economy.trade_step.buy, economy.trade_step.sell] {
        bounded(out, owner, step.into(), INDEX_BOUND.into(), "a trade step");
    }
    goods(out, w, economy);
    producers(out, economy);
    markets(out, w, economy);
    links(out, w, economy);
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
        for good in &economy.goods {
            let (made, used) = economy.supply(market, good, BASE_INDEX);
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
