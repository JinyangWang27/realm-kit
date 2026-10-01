//! Currency, trade at markets, and the price tick that moves prices with
//! what each place makes and needs.

use super::*;
use realmkit_spec::{Economy, Market, PriceTick, BASE_INDEX, CURRENCY_BOUND, TRADE_BOUND};

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

/// The index after one unit is bought: up by the buy step, within bounds.
fn raised(economy: &Economy, index: u32) -> u32 {
    index
        .saturating_add(economy.trade_step.buy)
        .min(economy.index_bounds[1])
}

/// The index after one unit is sold: down by the sell step, within bounds.
fn lowered(economy: &Economy, index: u32) -> u32 {
    index
        .saturating_sub(economy.trade_step.sell)
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
    let spread = economy.spread(market);
    Some(Quote {
        buy: buy_price(price, index, spread),
        sell: sell_price(price, index, spread),
        next_buy: buy_price(price, raised(economy, index), spread),
        next_sell: sell_price(price, lowered(economy, index), spread),
    })
}

/// Buys units one at a time: each costs the price at the current index, then
/// raises it by the buy step.
pub(crate) fn buy(
    world: &WorldSpec,
    state: &mut GameState,
    good: &str,
    quantity: u64,
    events: &mut Vec<Event>,
) -> Result<(), EngineError> {
    let (economy, market) = market_here(world, state)?;
    let good = economy
        .good(good)
        .ok_or_else(|| EngineError::NotTraded(good.into()))?;
    if quantity == 0 || quantity > TRADE_BOUND {
        return Err(EngineError::InvalidQuantity);
    }
    let spread = economy.spread(market);
    let wallet = state.economy.as_mut().unwrap();
    let index = wallet
        .prices
        .get_mut(&market.location)
        .and_then(|p| p.get_mut(&good.item))
        .unwrap();
    let mut cost = 0_u64;
    for _ in 0..quantity {
        cost = cost
            .checked_add(buy_price(good.price, *index, spread))
            .ok_or(EngineError::NumericLimit)?;
        *index = raised(economy, *index);
    }
    wallet.currency = wallet
        .currency
        .checked_sub(cost)
        .ok_or(EngineError::NotEnoughCurrency)?;
    let count = state.player.inventory.entry(good.item.clone()).or_default();
    *count = count
        .checked_add(quantity)
        .ok_or(EngineError::NumericLimit)?;
    events.push(Event::Bought {
        good: good.item.clone(),
        quantity,
        cost,
    });
    Ok(())
}

/// Sells units one at a time: each fetches the price at the current index,
/// then lowers it by the sell step.
pub(crate) fn sell(
    world: &WorldSpec,
    state: &mut GameState,
    good: &str,
    quantity: u64,
    events: &mut Vec<Event>,
) -> Result<(), EngineError> {
    let (economy, market) = market_here(world, state)?;
    let good = economy
        .good(good)
        .ok_or_else(|| EngineError::NotTraded(good.into()))?;
    if quantity == 0 || quantity > TRADE_BOUND {
        return Err(EngineError::InvalidQuantity);
    }
    story::take_items(
        state,
        &[realmkit_spec::ItemStack {
            item: good.item.clone(),
            quantity,
        }],
        &mut Vec::new(),
    )?;
    let spread = economy.spread(market);
    let wallet = state.economy.as_mut().unwrap();
    let index = wallet
        .prices
        .get_mut(&market.location)
        .and_then(|p| p.get_mut(&good.item))
        .unwrap();
    let mut earned = 0_u64;
    for _ in 0..quantity {
        earned = earned
            .checked_add(sell_price(good.price, *index, spread))
            .ok_or(EngineError::NumericLimit)?;
        *index = lowered(economy, *index);
    }
    receive(wallet, earned)?;
    events.push(Event::Sold {
        good: good.item.clone(),
        quantity,
        earned,
    });
    Ok(())
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
/// 1. Supply: production against consumption at the current index (see
///    [`Economy::supply`]). A surplus lowers the index by a draw below `supply_step` ×
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
    let prices = &mut state.economy.as_mut().unwrap().prices;
    let mut index: BTreeMap<(&str, &str), i64> = BTreeMap::new();
    for market in &economy.markets {
        for good in &economy.goods {
            let at = prices[&market.location][&good.item];
            index.insert((&market.location, &good.item), i64::from(at));
        }
    }
    supply(economy, &rules, stream, &mut index, low, high);
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

/// Phase 1: production against consumption moves each index.
fn supply<'w>(
    economy: &'w Economy,
    rules: &PriceTick,
    stream: &mut u64,
    index: &mut BTreeMap<(&'w str, &'w str), i64>,
    low: i64,
    high: i64,
) {
    let step = u64::from(rules.supply_step);
    for market in &economy.markets {
        for good in &economy.goods {
            let value = index
                .get_mut(&(market.location.as_str(), good.item.as_str()))
                .unwrap();
            let (made, used) = economy.supply(market, good, *value as u32);
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
