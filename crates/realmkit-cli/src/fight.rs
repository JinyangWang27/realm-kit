//! The fight screen. In terminal play a fight has the alternate screen to
//! itself, redrawn each turn: everyone's health, the next turns, the last few
//! lines of the fight and the menu. When it ends, only the opening line and
//! the final turn return to the normal scrollback.

use crate::{menu::Menu, render::Paint};
use crossterm::{
    cursor::MoveTo,
    queue,
    style::Stylize,
    terminal::{Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};
use realmkit_engine::Engine;
use std::io::{self, Write};

/// Lines of fight history shown; the latest turn is always shown whole.
const TAIL: usize = 6;
const TURNS_SHOWN: usize = 5;
const BAR: usize = 10;
const NEXT: &str = "Next";
const DOWN: &str = "down";
const YIELDED: &str = "yielded";

/// Whether the engine is in a fight the player can still act in; a dead
/// player's fight is over for the screen.
pub fn active(engine: &Engine<'_>) -> bool {
    engine.encounter().is_some() && !engine.is_dead()
}

pub struct FightScreen {
    /// Every line of the fight so far, with the turn it came from.
    lines: Vec<(usize, String)>,
    turn: usize,
    /// Help, typos and other messages that are no turn: shown once, below.
    pub notice: String,
}

impl FightScreen {
    pub fn enter(output: &mut impl Write) -> io::Result<Self> {
        queue!(output, EnterAlternateScreen)?;
        Ok(Self {
            lines: Vec::new(),
            turn: 0,
            notice: String::new(),
        })
    }

    /// Adds one command's output as the latest turn.
    pub fn record(&mut self, text: &[u8]) {
        let text = String::from_utf8_lossy(text);
        let lines: Vec<_> = text.lines().filter(|l| !l.trim().is_empty()).collect();
        if lines.is_empty() {
            return;
        }
        self.turn += 1;
        self.lines
            .extend(lines.into_iter().map(|l| (self.turn, l.to_string())));
    }

    /// Returns to the normal screen, which keeps the fight's opening line and
    /// its final turn: `last`, or the latest recorded turn when play stopped
    /// without one.
    pub fn leave(self, output: &mut impl Write, last: &[u8]) -> io::Result<()> {
        queue!(output, LeaveAlternateScreen)?;
        if let Some((_, opening)) = self.lines.first() {
            writeln!(output, "{opening}")?;
        }
        if !last.is_empty() {
            return output.write_all(last);
        }
        let latest = self.lines.iter().skip(1).filter(|(t, _)| *t == self.turn);
        for (_, line) in latest {
            writeln!(output, "{line}")?;
        }
        Ok(())
    }

    pub fn draw(
        &self,
        output: &mut impl Write,
        engine: &Engine<'_>,
        menu: &Menu,
        paint: Paint,
    ) -> io::Result<()> {
        queue!(output, MoveTo(0, 0), Clear(ClearType::All))?;
        status(output, engine, paint)?;
        writeln!(output)?;
        // The latest turn whole, then older lines up to the tail length.
        let latest = self
            .lines
            .iter()
            .position(|(turn, _)| *turn == self.turn)
            .unwrap_or(self.lines.len());
        let start = latest.min(self.lines.len().saturating_sub(TAIL));
        for (turn, line) in &self.lines[start..] {
            if *turn == self.turn {
                writeln!(output, "› {}", paint.title(line))?;
            } else {
                writeln!(output, "  {}", paint.dim(line))?;
            }
        }
        if !self.notice.is_empty() {
            writeln!(output, "\n{}", self.notice.trim_end())?;
        }
        menu.write(output, true, paint)?;
        Ok(())
    }
}

/// Opponents, then the player, each with a health bar, then the next turns.
fn status(output: &mut impl Write, engine: &Engine<'_>, paint: Paint) -> io::Result<()> {
    let Some(encounter) = engine.encounter() else {
        return Ok(());
    };
    let world = engine.world();
    let name = |id: &str| world.character(id).unwrap().name.as_str();
    let resources = world.combat().unwrap().resources;
    let rage = resources.rage_per_action > 0 || resources.rage_per_max_hp > 0;
    let (player, opponents) = encounter.participants.split_first().unwrap();
    // ponytail: pads by character count; wide (CJK) names misalign until a width table is needed.
    let width = encounter
        .participants
        .iter()
        .map(|p| name(&p.character).chars().count())
        .max()
        .unwrap_or(0);
    for p in opponents.iter().chain([player]) {
        let max = match &world.character(&p.character).unwrap().combat {
            Some(profile) => profile.stats,
            None => engine.player_stats().unwrap(),
        };
        let label = name(&p.character);
        let pad = " ".repeat(width - label.chars().count());
        let digits = max.hp.to_string().len();
        write!(
            output,
            "{label}{pad}  {} {:>digits$}/{}",
            bar(p.hp, max.hp, paint),
            p.hp,
            max.hp
        )?;
        let mut notes = Vec::new();
        if p.hp == 0 {
            writeln!(output, "  {DOWN}")?;
            continue;
        }
        if max.mp > 0 {
            notes.push(format!("MP {}/{}", p.mp, max.mp));
        }
        if rage {
            notes.push(format!("rage {}", p.rage));
        }
        if p.yielded {
            notes.push(YIELDED.into());
        }
        match notes.is_empty() {
            true => writeln!(output)?,
            false => writeln!(output, "  {}", notes.join(" · "))?,
        }
    }
    let order: Vec<_> = engine
        .turn_order(TURNS_SHOWN)
        .iter()
        .map(|id| name(id))
        .collect();
    writeln!(output, "{NEXT}: {}", order.join(", "))
}

/// Ten cells of health, in halves: green above half, yellow above a quarter,
/// red below. A living fighter always shows at least half a cell.
fn bar(hp: u32, max: u32, paint: Paint) -> String {
    let halves = (u64::from(hp.min(max)) * (2 * BAR as u64) / u64::from(max.max(1))) as usize;
    let halves = if hp > 0 { halves.max(1) } else { 0 };
    let (full, half) = (halves / 2, halves % 2);
    let filled = "█".repeat(full) + if half == 1 { "▌" } else { "" };
    let empty = "░".repeat(BAR - full - half);
    if !paint.styled || hp == 0 {
        return filled + &empty;
    }
    let share = u64::from(hp) * 4 / u64::from(max.max(1));
    let filled = match share {
        0 => filled.red(),
        1 => filled.yellow(),
        _ => filled.green(),
    };
    format!("{filled}{}", empty.dim())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bars_fill_in_halves_and_keep_a_sliver_for_the_living() {
        let plain = Paint::default();
        assert_eq!(bar(24, 24, plain), "██████████");
        assert_eq!(bar(39, 60, plain), "██████▌░░░");
        assert_eq!(bar(1, 100, plain), "▌░░░░░░░░░");
        assert_eq!(bar(0, 24, plain), "░░░░░░░░░░");
        // More HP than the maximum fills the bar instead of panicking.
        assert_eq!(bar(30, 24, plain), "██████████");
    }

    #[test]
    fn leaving_without_a_final_command_keeps_the_last_turn() {
        let mut screen = FightScreen::enter(&mut Vec::new()).unwrap();
        screen.record(b"You face The Rat.\nThe Rat bites You: 1 damage.\n");
        screen.record(b"You hit The Rat: 2 damage.\n");
        let mut output = Vec::new();
        screen.leave(&mut output, b"").unwrap();
        let text = String::from_utf8(output).unwrap();
        assert!(
            text.ends_with("You face The Rat.\nYou hit The Rat: 2 damage.\n"),
            "{text:?}"
        );
    }

    #[test]
    fn blank_output_is_not_a_turn() {
        let mut screen = FightScreen::enter(&mut Vec::new()).unwrap();
        for turn in 0..3 {
            screen.record(format!("a{turn}\nb{turn}\nc{turn}\n").as_bytes());
        }
        screen.record(b"\n");
        let latest = screen.lines.iter().filter(|(t, _)| *t == 3).count();
        assert_eq!((screen.turn, latest), (3, 3), "blank output is no turn");
    }
}
