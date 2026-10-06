//! The bank: deposits and withdrawals at its branches, each day's closing
//! balance, and interest on a month's average close. It knows nothing of
//! what its money is later spent on.

use super::*;
use realmkit_spec::{BASIS_POINTS, CURRENCY_BOUND};

/// The account, open at a branch where the player stands.
fn account_here<'s>(
    world: &WorldSpec,
    state: &'s mut GameState,
    amount: u64,
) -> Result<&'s mut EconomyState, EngineError> {
    let banking = world.economy().and_then(|e| e.banking.as_ref());
    if banking.is_none_or(|b| !b.branches.contains(&state.player.location)) {
        return Err(EngineError::NoBank);
    }
    if amount == 0 || amount > CURRENCY_BOUND {
        return Err(EngineError::InvalidAmount);
    }
    Ok(state.economy.as_mut().unwrap())
}

/// Moves `amount` between the purse and the bank, all or nothing; either
/// side stays within the currency bound.
pub(crate) fn transfer(
    world: &WorldSpec,
    state: &mut GameState,
    amount: u64,
    depositing: bool,
    events: &mut Vec<Event>,
) -> Result<(), EngineError> {
    let wallet = account_here(world, state, amount)?;
    let bank = wallet.bank.as_mut().unwrap();
    let (from, to, short) = if depositing {
        (
            &mut wallet.currency,
            &mut bank.balance,
            EngineError::NotEnoughCurrency,
        )
    } else {
        (
            &mut bank.balance,
            &mut wallet.currency,
            EngineError::NotEnoughDeposit,
        )
    };
    let left = from.checked_sub(amount).ok_or(short)?;
    *to = to
        .checked_add(amount)
        .filter(|total| *total <= CURRENCY_BOUND)
        .ok_or(EngineError::NumericLimit)?;
    *from = left;
    events.push(if depositing {
        Event::Deposited { amount }
    } else {
        Event::Withdrawn { amount }
    });
    Ok(())
}

/// Midnight: the day just ended closes at the bank's balance.
pub(crate) fn close_day(state: &mut GameState) {
    // At most 31 closes of at most the currency bound: far inside 64 bits.
    if let Some(bank) = state.economy.as_mut().and_then(|e| e.bank.as_mut()) {
        bank.month_to_date += bank.balance;
    }
}

/// Interest on the ended month's average daily close over all its `days`,
/// rounded down once, paid into the bank up to the currency bound. The
/// month's sum starts again from nothing.
pub(crate) fn interest(
    world: &WorldSpec,
    state: &mut GameState,
    days: u8,
    events: &mut Vec<Event>,
) {
    let Some(bank) = state.economy.as_mut().and_then(|e| e.bank.as_mut()) else {
        return;
    };
    let sum = std::mem::take(&mut bank.month_to_date);
    if sum == 0 {
        return;
    }
    let rate = world.economy().unwrap().banking.as_ref().unwrap();
    let days = u64::from(days);
    let earned = u128::from(sum) * u128::from(rate.monthly_interest_basis_points)
        / u128::from(days * u64::from(BASIS_POINTS));
    // At most the bound: the average is, and the rate at most 100 percent.
    let earned = u64::try_from(earned).unwrap();
    let amount = earned.min(CURRENCY_BOUND - bank.balance);
    bank.balance += amount;
    events.push(Event::InterestCredited {
        average: sum / days,
        amount,
        forgone: earned - amount,
    });
}
