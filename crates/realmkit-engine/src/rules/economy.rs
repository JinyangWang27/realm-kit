//! Currency, trade at markets, and the price tick that moves prices with
//! what each place makes and needs.

use super::*;
use realmkit_spec::{
    scaled, Economy, Good, Market, MarketKind, PriceTick, Proficiency, BASE_INDEX, CURRENCY_BOUND,
    STOCK_BOUND, TRADE_BOUND,
};

/// The market at the player's location, open while its merchant is here.
pub(crate) fn market_here<'w>(
    world: &'w WorldSpec,
    state: &GameState,
) -> Result<(&'w Economy, &'w Market), EngineError> {
    let economy = world.economy().ok_or(EngineError::NoMarket)?;
    let market = economy
        .market(&state.player.location)
        .ok_or(EngineError::NoMarket)?;
    match &market.merchant {
        // A defeated merchant is gone, like its listing.
        Some(merchant)
            if character_here(world, state, merchant).is_none() || defeated(state, merchant) =>
        {
            Err(EngineError::NotHere(merchant.clone()))
        }
        _ => Ok((economy, market)),
    }
}

/// The index after one unit is bought: up by the trade step, within bounds.
fn raised(economy: &Economy, index: u32) -> u32 {
    index
        .saturating_add(economy.trade_step)
        .min(economy.index_bounds[1])
}

/// The index after one unit is sold: down by the trade step, within bounds.
fn lowered(economy: &Economy, index: u32) -> u32 {
    index
        .saturating_sub(economy.trade_step)
        .max(economy.index_bounds[0])
}

/// A good's prices at the open market here, as trading would charge them.
pub(crate) fn quote(world: &WorldSpec, state: &GameState, good: &str) -> Option<Quote> {
    let (economy, market) = market_here(world, state).ok()?;
    let price = economy.good(good)?.price;
    let index = *state
        .economy
        .as_ref()?
        .prices
        .get(&market.location)?
        .get(good)?;
    let spread = economy.spread(market, proficiency::rank(state, Proficiency::Trading));
    let stock = state.economy.as_ref()?.stock.get(&market.location);
    Some(Quote {
        buy: buy_price(price, index, spread),
        sell: sell_price(price, index, spread),
        next_buy: buy_price(price, raised(economy, index), spread),
        next_sell: sell_price(price, lowered(economy, index), spread),
        stock: stock.map(|s| s.goods[good]),
        purse: stock.map(|s| s.currency),
    })
}

/// Buys or sells units one at a time: each trades at the price for the
/// current index, then moves it one trade step, up for a purchase and down
/// for a sale. The step is the same both ways, which validation relies on
/// to keep trading back and forth from paying.
pub(crate) fn trade(
    world: &WorldSpec,
    state: &mut GameState,
    good: &str,
    quantity: u64,
    buying: bool,
    events: &mut Vec<Event>,
) -> Result<(), EngineError> {
    let (economy, market) = market_here(world, state)?;
    if quantity == 0 || quantity > TRADE_BOUND {
        return Err(EngineError::InvalidQuantity);
    }
    if let (Some(ware), true) = (economy.ware(market, good), buying) {
        return buy_ware(world, state, ware, quantity, events);
    }
    let good = economy
        .good(good)
        .ok_or_else(|| EngineError::NotTraded(good.into()))?;
    let stack = [realmkit_spec::ItemStack {
        item: good.item.clone(),
        quantity,
    }];
    if !buying {
        story::take_items(state, &stack, &mut Vec::new())?;
    }
    let spread = economy.spread(market, proficiency::rank(state, Proficiency::Trading));
    let wallet = state.economy.as_mut().unwrap();
    let index = wallet
        .prices
        .get_mut(&market.location)
        .and_then(|p| p.get_mut(&good.item))
        .unwrap();
    let mut total = 0_u64;
    for _ in 0..quantity {
        let unit = if buying {
            buy_price(good.price, *index, spread)
        } else {
            sell_price(good.price, *index, spread)
        };
        total = total.checked_add(unit).ok_or(EngineError::NumericLimit)?;
        *index = if buying {
            raised(economy, *index)
        } else {
            lowered(economy, *index)
        };
    }
    let good = good.item.clone();
    // Where merchants keep stock, a purchase comes out of it and pays into
    // their purse, and a sale is what the purse can cover.
    if let Some(stock) = wallet.stock.get_mut(&market.location) {
        let units = stock.goods.get_mut(&good).unwrap();
        if buying {
            *units = units
                .checked_sub(quantity)
                .ok_or_else(|| EngineError::OutOfStock(good.clone()))?;
            stock.currency = fill(stock.currency, total);
        } else {
            *units = units
                .checked_add(quantity)
                .filter(|u| *u <= STOCK_BOUND)
                .ok_or(EngineError::NumericLimit)?;
            stock.currency = stock
                .currency
                .checked_sub(total)
                .ok_or(EngineError::MerchantCannotPay)?;
        }
    }
    if buying {
        wallet.currency = wallet
            .currency
            .checked_sub(total)
            .ok_or(EngineError::NotEnoughCurrency)?;
        story::grant_items(world, state, &stack, &mut Vec::new())?;
        events.push(Event::Bought {
            good,
            quantity,
            cost: total,
        });
    } else {
        receive(wallet, total)?;
        events.push(Event::Sold {
            good,
            quantity,
            earned: total,
        });
    }
    Ok(())
}

/// Buys `quantity` of a ware at its fixed price, all or nothing.
fn buy_ware(
    world: &WorldSpec,
    state: &mut GameState,
    ware: &realmkit_spec::Ware,
    quantity: u64,
    events: &mut Vec<Event>,
) -> Result<(), EngineError> {
    // A purchase is a grant: equipment comes at most a grant's worth at once.
    let wearable = world
        .item(&ware.item)
        .is_some_and(|i| i.equipment.is_some());
    if wearable && quantity > realmkit_spec::GEAR_STACK_BOUND {
        return Err(EngineError::InvalidQuantity);
    }
    let cost = ware
        .price
        .checked_mul(quantity)
        .ok_or(EngineError::NumericLimit)?;
    let here = &state.player.location;
    let wallet = state.economy.as_mut().unwrap();
    wallet.currency = wallet
        .currency
        .checked_sub(cost)
        .ok_or(EngineError::NotEnoughCurrency)?;
    // A ware comes from elsewhere, but its price still fills the purse.
    if let Some(stock) = wallet.stock.get_mut(here) {
        stock.currency = fill(stock.currency, cost);
    }
    let stack = [realmkit_spec::ItemStack {
        item: ware.item.clone(),
        quantity,
    }];
    story::grant_items(world, state, &stack, &mut Vec::new())?;
    events.push(Event::Bought {
        good: ware.item.clone(),
        quantity,
        cost,
    });
    Ok(())
}

/// One unit's price of a ware at the open market here.
pub(crate) fn ware_price(world: &WorldSpec, state: &GameState, item: &str) -> Option<u64> {
    let (economy, market) = market_here(world, state).ok()?;
    Some(economy.ware(market, item)?.price)
}

/// A purse after taking `amount`: it holds at most the currency bound and
/// lets the rest go, since its size only limits what merchants can buy.
fn fill(purse: u64, amount: u64) -> u64 {
    purse.saturating_add(amount).min(CURRENCY_BOUND)
}

fn receive(wallet: &mut EconomyState, amount: u64) -> Result<(), EngineError> {
    wallet.currency = wallet
        .currency
        .checked_add(amount)
        .filter(|total| *total <= CURRENCY_BOUND)
        .ok_or(EngineError::NumericLimit)?;
    Ok(())
}

pub(crate) fn grant(
    state: &mut GameState,
    amount: u64,
    events: &mut Vec<Event>,
) -> Result<(), EngineError> {
    receive(state.economy.as_mut().unwrap(), amount)?;
    events.push(Event::CurrencyReceived { amount });
    Ok(())
}

pub(crate) fn pay(
    state: &mut GameState,
    amount: u64,
    events: &mut Vec<Event>,
) -> Result<(), EngineError> {
    let wallet = state.economy.as_mut().unwrap();
    wallet.currency = wallet
        .currency
        .checked_sub(amount)
        .ok_or(EngineError::NotEnoughCurrency)?;
    events.push(Event::CurrencyPaid { amount });
    Ok(())
}

/// One price tick, in four phases; each finishes for every market before the
/// next begins. `scripts/combat_sim/economy.py` mirrors it exactly.
///
/// 1. Supply: production against consumption at the pre-tick indices (see
///    [`Economy::supply`]), a town's villages counted with it. A surplus lowers the index by a draw below `supply_step` ×
///    surplus, scaled by index ÷ `damp_below` while the index is below it; a
///    shortage raises it the same way, undamped. Draws come from the market
///    stream in market, then goods, order, only where supply is unbalanced.
/// 2. Revert: the gap to 1,000 shrinks by `revert_percent`, rounded towards zero.
/// 3. Inputs: a good whose input is dearer moves `input_pull_percent` of the
///    way up to it, measured on the phase-2 prices.
/// 4. Links: each linked pair moves `percent` of the gap towards each other,
///    measured on the phase-3 prices and applied together.
pub(crate) fn tick(world: &WorldSpec, state: &mut GameState) {
    let economy = world.economy().unwrap();
    let rules = economy.tick.unwrap();
    let [low, high] = economy.index_bounds.map(i64::from);
    let stream = state.rng.as_mut().unwrap().market.as_mut().unwrap();
    let wallet = state.economy.as_mut().unwrap();
    let prosperity = &wallet.prosperity;
    let prices = &mut wallet.prices;
    let mut index: BTreeMap<(&str, &str), i64> = BTreeMap::new();
    for market in &economy.markets {
        for good in &economy.goods {
            let at = prices[&market.location][&good.item];
            index.insert((&market.location, &good.item), i64::from(at));
        }
    }
    supply(economy, prosperity, &rules, stream, &mut index, low, high);
    for value in index.values_mut() {
        let gap = *value - i64::from(BASE_INDEX);
        *value = i64::from(BASE_INDEX) + gap * (100 - i64::from(rules.revert_percent)) / 100;
    }
    // Phases 3 and 4 measure every move on the previous phase's prices, so
    // each gathers its moves first and applies them together.
    let mut pulls = Vec::new();
    for market in &economy.markets {
        for good in &economy.goods {
            let Some(input) = &good.input else {
                continue;
            };
            let made_from = index[&(market.location.as_str(), input.as_str())];
            let own = index[&(market.location.as_str(), good.item.as_str())];
            if made_from > own {
                let pull = (made_from - own) * i64::from(rules.input_pull_percent) / 100;
                pulls.push(((market.location.as_str(), good.item.as_str()), pull));
            }
        }
    }
    for (key, pull) in pulls {
        *index.get_mut(&key).unwrap() += pull;
    }
    let mut moves = Vec::new();
    for link in &economy.links {
        let [a, b] = &link.between;
        for good in &economy.goods {
            let at = |m: &str| index[&(m, good.item.as_str())];
            let step = (at(b) - at(a)) * i64::from(link.percent) / 100;
            moves.push(((a.as_str(), good.item.as_str()), step));
            moves.push(((b.as_str(), good.item.as_str()), -step));
        }
    }
    for (key, step) in moves {
        *index.get_mut(&key).unwrap() += step;
    }
    for ((market, good), value) in index {
        let value = value.clamp(low, high);
        *prices.get_mut(market).unwrap().get_mut(good).unwrap() = value as u32;
    }
}

/// Phase 1: production against consumption moves each index, all of it
/// measured on the pre-tick indices.
fn supply<'w>(
    economy: &'w Economy,
    prosperity: &BTreeMap<Id, u32>,
    rules: &PriceTick,
    stream: &mut u64,
    index: &mut BTreeMap<(&'w str, &'w str), i64>,
    low: i64,
    high: i64,
) {
    let step = u64::from(rules.supply_step);
    let before = index.clone();
    for market in &economy.markets {
        for good in &economy.goods {
            let at = |m: &str| before[&(m, good.item.as_str())] as u32;
            let (made, used) = market_supply(economy, prosperity, market, good, &at);
            let value = index
                .get_mut(&(market.location.as_str(), good.item.as_str()))
                .unwrap();
            if made > used && step > 0 {
                let mut fall = rng::below(stream, (made - used) * step) as i64;
                if *value < i64::from(rules.damp_below) {
                    fall = fall * *value / i64::from(rules.damp_below);
                }
                *value = (*value - fall).max(low);
            } else if used > made && step > 0 {
                let rise = rng::below(stream, (used - made) * step) as i64;
                *value = (*value + rise).min(high);
            }
        }
    }
}

/// What a market makes and uses up of a good on a tick, with each market's
/// index for it given by `at`: its own supply at its own prosperity, and a
/// town adds each of its villages' at theirs.
fn market_supply(
    economy: &Economy,
    prosperity: &BTreeMap<Id, u32>,
    market: &Market,
    good: &Good,
    at: &dyn Fn(&str) -> u32,
) -> (u64, u64) {
    let own = |m: &Market| {
        let level = prosperity.get(&m.location).copied().unwrap_or(0);
        economy.supply(m, good, at(&m.location), level)
    };
    let (mut made, mut used) = own(market);
    if market.kind == MarketKind::Town {
        for village in economy.villages(&market.location) {
            let (m, u) = own(village);
            made = made.saturating_add(m);
            used = used.saturating_add(u);
        }
    }
    (made, used)
}

/// Each market's prosperity moves one point towards its ideal: the base,
/// lowered for each good it needs whose index is at or above the scarcity
/// level. Draws nothing. `scripts/combat_sim/economy.py` mirrors it.
pub(crate) fn prosper(world: &WorldSpec, state: &mut GameState) {
    let economy = world.economy().unwrap();
    let rules = economy.prosperity.unwrap();
    let wallet = state.economy.as_mut().unwrap();
    for market in &economy.markets {
        let prices = &wallet.prices[&market.location];
        let scarce = economy
            .goods
            .iter()
            .filter(|g| economy.needs(market, g) && prices[&g.item] >= rules.scarce_above)
            .count() as u32;
        let ideal = rules
            .base
            .saturating_sub(rules.scarcity.saturating_mul(scarce));
        let now = wallet.prosperity.get_mut(&market.location).unwrap();
        *now = match (*now).cmp(&ideal) {
            std::cmp::Ordering::Less => *now + 1,
            std::cmp::Ordering::Greater => *now - 1,
            std::cmp::Ordering::Equal => *now,
        };
    }
}

/// A market's full purse and each good's target stock, in goods order: the
/// units (scaled by prosperity, where it exists) split by what the market
/// and its villages make, weighed by `made × 1,000 ÷ index` so cheap goods
/// count for more.
fn targets(economy: &Economy, wallet: &EconomyState, market: &Market) -> (u64, Vec<u64>) {
    let rules = economy.stock.unwrap();
    let (mut units, mut currency) = (rules.units, rules.currency);
    if economy.prosperity.is_some() {
        let level = wallet.prosperity[&market.location];
        units = scaled(units, rules.prosperity_percent, level);
        currency = scaled(currency, rules.prosperity_percent, level);
    }
    let weights: Vec<u128> = economy
        .goods
        .iter()
        .map(|good| {
            let at = |m: &str| wallet.prices[m][&good.item];
            let (made, _) = market_supply(economy, &wallet.prosperity, market, good, &at);
            u128::from(made) * u128::from(BASE_INDEX) / u128::from(at(&market.location))
        })
        .collect();
    let total: u128 = weights.iter().sum();
    let targets = weights
        .iter()
        .map(|w| match total {
            0 => 0,
            // At most `units`, which validation keeps within half the stock bound.
            _ => (u128::from(units) * w / total) as u64,
        })
        .collect();
    (currency, targets)
}

/// New Game: every market's stock at its targets and its purse full,
/// without drawing.
pub(crate) fn stock_up(world: &WorldSpec, state: &mut GameState) {
    let Some(economy) = world.economy().filter(|e| e.stock.is_some()) else {
        return;
    };
    let wallet = state.economy.as_mut().unwrap();
    for market in &economy.markets {
        let (currency, targets) = targets(economy, wallet, market);
        let goods = economy.goods.iter().map(|g| g.item.clone()).zip(targets);
        let stock = MarketStock {
            currency,
            goods: goods.collect(),
        };
        wallet.stock.insert(market.location.clone(), stock);
    }
}

/// Every market's purse fills and its stock is redrawn around its targets,
/// in market then goods order, from the `stock` stream: a good with target
/// `t` gets a draw below `2t + 1`, and one with none gets none and draws
/// nothing. `scripts/combat_sim/economy.py` mirrors it.
pub(crate) fn restock(world: &WorldSpec, state: &mut GameState) {
    let economy = world.economy().unwrap();
    let stream = state.rng.as_mut().unwrap().stock.as_mut().unwrap();
    let wallet = state.economy.as_mut().unwrap();
    for market in &economy.markets {
        let (currency, targets) = targets(economy, wallet, market);
        let stock = wallet.stock.get_mut(&market.location).unwrap();
        stock.currency = currency;
        for (good, target) in economy.goods.iter().zip(targets) {
            let units = match target {
                0 => 0,
                _ => rng::below(stream, 2 * target + 1),
            };
            stock.goods.insert(good.item.clone(), units);
        }
    }
}

/// The weekly settlement: each workshop's output, less its inputs and
/// overhead, valued at its town's index without a spread. The total is paid
/// at once; a loss takes at most what the player holds and reports the
/// rest as a shortfall. Workshops never move prices.
pub(crate) fn settle(world: &WorldSpec, state: &mut GameState, events: &mut Vec<Event>) {
    let economy = world.economy().unwrap();
    let wallet = state.economy.as_mut().unwrap();
    if wallet.workshops.is_empty() {
        return;
    }
    let mut net = 0_i128;
    for (town, kinds) in &wallet.workshops {
        let value = |good: &str, units: u64| {
            let price = economy.good(good).unwrap().price;
            let index = wallet.prices[town][good];
            i128::from(price) * i128::from(index) * i128::from(units) / 1_000
        };
        for (kind, count) in kinds {
            let kind = economy.workshop(kind).unwrap();
            let inputs: i128 = kind.inputs.iter().map(|(g, n)| value(g, *n)).sum();
            let one = value(&kind.good, kind.output) - inputs - i128::from(kind.overhead);
            net += one * i128::from(*count);
        }
    }
    let clamp = |v: i128| u64::try_from(v).unwrap_or(u64::MAX);
    if net >= 0 {
        let earned = clamp(net);
        let amount = earned.min(CURRENCY_BOUND - wallet.currency);
        wallet.currency += amount;
        events.push(Event::WorkshopsEarned {
            amount,
            forgone: earned - amount,
        });
    } else {
        let owed = clamp(-net);
        let amount = owed.min(wallet.currency);
        wallet.currency -= amount;
        events.push(Event::WorkshopsLost {
            amount,
            shortfall: owed - amount,
        });
    }
}

/// Buys a workshop of `kind` in the town market where the player stands,
/// within the per-town limit.
pub(crate) fn buy_workshop(
    world: &WorldSpec,
    state: &mut GameState,
    kind: &str,
    events: &mut Vec<Event>,
) -> Result<(), EngineError> {
    let economy = world.economy().unwrap();
    let rules = economy.workshops.as_ref().unwrap();
    let kind = economy.workshop(kind).unwrap();
    let here = state.player.location.clone();
    if economy
        .market(&here)
        .is_none_or(|m| m.kind != MarketKind::Town)
    {
        return Err(EngineError::NoWorkshopHere);
    }
    let wallet = state.economy.as_mut().unwrap();
    let owned: u32 = wallet.workshops.get(&here).map_or(0, |k| k.values().sum());
    if owned >= rules.limit {
        return Err(EngineError::WorkshopLimit);
    }
    wallet.currency = wallet
        .currency
        .checked_sub(kind.price)
        .ok_or(EngineError::NotEnoughCurrency)?;
    *wallet
        .workshops
        .entry(here.clone())
        .or_default()
        .entry(kind.id.clone())
        .or_default() += 1;
    events.push(Event::WorkshopBought {
        workshop: kind.id.clone(),
        location: here,
        cost: kind.price,
    });
    Ok(())
}

/// Sells back one workshop of `kind` where the player stands.
pub(crate) fn sell_workshop(
    world: &WorldSpec,
    state: &mut GameState,
    kind: &str,
    events: &mut Vec<Event>,
) -> Result<(), EngineError> {
    let kind = world.economy().unwrap().workshop(kind).unwrap();
    let here = state.player.location.clone();
    let wallet = state.economy.as_mut().unwrap();
    let kinds = wallet
        .workshops
        .get_mut(&here)
        .ok_or(EngineError::NoWorkshop)?;
    let count = kinds.get_mut(&kind.id).ok_or(EngineError::NoWorkshop)?;
    *count -= 1;
    if *count == 0 {
        kinds.remove(&kind.id);
    }
    if kinds.is_empty() {
        wallet.workshops.remove(&here);
    }
    receive(wallet, kind.resale)?;
    events.push(Event::WorkshopSold {
        workshop: kind.id.clone(),
        location: here,
        earned: kind.resale,
    });
    Ok(())
}
