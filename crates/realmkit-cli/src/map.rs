//! The overland map as text: a window onto the world's places and roads.
//! The engine decides what the player may know; this only draws it.

use crate::{
    menu::Key,
    render::{duration, Paint},
};
use crossterm::{
    cursor::MoveTo,
    queue,
    terminal::{Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};
use realmkit_engine::{Engine, MapPlace, MapView};
use realmkit_spec::{PlaceKind, WorldSpec};
use std::io::{self, Write};

/// Line mode's fixed frame: no terminal size is known there.
pub const LINE_COLS: usize = 80;
/// Rows of map in line mode's frame; the legend takes one more.
pub const LINE_ROWS: usize = 23;
/// Each zoom level halves the scale of the one before.
pub const MAX_ZOOM: u32 = 6;
// Room kept around the places at zoom 0, so the outermost labels fit.
const PAD_COLS: usize = 12;
const PAD_ROWS: usize = 1;

// Below the map: where the player is or what Tab picked, the legend, keys.
const SCREEN_LINES: u16 = 3;
const KEYS_HINT: &str = "+/- zoom · arrows pan · 0 fit · c centre · Tab next place · Esc back";
const YOU: char = '@';
const ROAD: char = '·';
const LEGEND_YOU: &str = "you";

fn glyph(kind: PlaceKind) -> char {
    match kind {
        PlaceKind::Town => 'O',
        PlaceKind::Castle => '#',
        PlaceKind::Village => 'o',
        PlaceKind::Waypoint => '.',
    }
}

fn kind_name(kind: PlaceKind) -> &'static str {
    match kind {
        PlaceKind::Town => "town",
        PlaceKind::Castle => "castle",
        PlaceKind::Village => "village",
        PlaceKind::Waypoint => "waypoint",
    }
}

/// Which part of the world is drawn, and at what size. The scale depends
/// only on the world and the zoom level, never on the terminal, so resizing
/// changes how much is visible but not the zoom.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Viewport {
    /// The world point at the middle of the window.
    pub centre: (f64, f64),
    pub zoom: u32,
    pub cols: usize,
    pub rows: usize,
}

impl Viewport {
    /// Zoom 0, centred on every place, which then fits line mode's frame.
    pub fn fit(view: &MapView, cols: usize, rows: usize) -> Self {
        let ((left, top), (right, bottom)) = extent(view);
        Self {
            centre: ((left + right) / 2.0, (top + bottom) / 2.0),
            zoom: 0,
            cols,
            rows,
        }
    }

    /// Centred on a place; `None` if it is not on the map.
    pub fn on(view: &MapView, location: &str, zoom: u32, cols: usize, rows: usize) -> Option<Self> {
        let place = view.places.iter().find(|p| p.location == location)?;
        Some(Self {
            centre: (f64::from(place.point.x), f64::from(place.point.y)),
            zoom: zoom.min(MAX_ZOOM),
            cols,
            rows,
        })
    }

    /// World units per column; a row spans twice as many, since terminal
    /// cells are about twice as tall as they are wide.
    pub fn scale(&self, view: &MapView) -> f64 {
        let ((left, top), (right, bottom)) = extent(view);
        let across = (right - left) / (LINE_COLS - 1 - 2 * PAD_COLS) as f64;
        let down = (bottom - top) / (2 * (LINE_ROWS - 1 - 2 * PAD_ROWS)) as f64;
        let fit = across.max(down);
        let fit = if fit > 0.0 { fit } else { 1.0 };
        fit / f64::from(1u32 << self.zoom.min(MAX_ZOOM))
    }

    /// Pans by a quarter of the window: `dx` and `dy` are -1, 0 or 1.
    pub fn pan(&mut self, view: &MapView, dx: i32, dy: i32) {
        let scale = self.scale(view);
        self.centre.0 += f64::from(dx) * scale * (self.cols / 4) as f64;
        self.centre.1 += f64::from(dy) * scale * 2.0 * (self.rows / 4) as f64;
    }

    /// The cell a world point falls in, which may lie outside the window.
    fn cell(&self, scale: f64, place: &MapPlace) -> (i64, i64) {
        let col = (f64::from(place.point.x) - self.centre.0) / scale + (self.cols / 2) as f64;
        let row =
            (f64::from(place.point.y) - self.centre.1) / (2.0 * scale) + (self.rows / 2) as f64;
        (col.round() as i64, row.round() as i64)
    }
}

fn extent(view: &MapView) -> ((f64, f64), (f64, f64)) {
    let xs = view.places.iter().map(|p| f64::from(p.point.x));
    let ys = view.places.iter().map(|p| f64::from(p.point.y));
    let (left, right) = xs.fold((f64::MAX, f64::MIN), |(lo, hi), v| (lo.min(v), hi.max(v)));
    let (top, bottom) = ys.fold((f64::MAX, f64::MIN), |(lo, hi), v| (lo.min(v), hi.max(v)));
    ((left, top), (right, bottom))
}

/// What has been drawn: characters, and which cells a glyph or label holds
/// (roads may be drawn over; glyphs and labels may not).
struct Canvas {
    cells: Vec<Vec<char>>,
    taken: Vec<Vec<bool>>,
}

impl Canvas {
    fn new(viewport: &Viewport) -> Self {
        Self {
            cells: vec![vec![' '; viewport.cols]; viewport.rows],
            taken: vec![vec![false; viewport.cols]; viewport.rows],
        }
    }

    fn inside(&self, (col, row): (i64, i64)) -> Option<(usize, usize)> {
        let (col, row) = (usize::try_from(col).ok()?, usize::try_from(row).ok()?);
        (row < self.cells.len() && col < self.cells[0].len()).then_some((col, row))
    }

    /// The cells of a line between two cells that fall in the window, ends
    /// excluded, from `from` towards `to`.
    fn points(&self, from: (i64, i64), to: (i64, i64)) -> Vec<(usize, usize)> {
        let (dc, dr) = (to.0 - from.0, to.1 - from.1);
        let steps = dc.abs().max(dr.abs());
        // d × step / steps, rounded.
        let at = |d: i64, step: i64| (2 * d * step + steps).div_euclid(2 * steps);
        (1..steps)
            .filter_map(|step| self.inside((from.0 + at(dc, step), from.1 + at(dr, step))))
            .collect()
    }

    /// A dotted line between two cells, ends excluded; with `head`, an arrow
    /// next to `to` points at it.
    fn line(&mut self, from: (i64, i64), to: (i64, i64), head: bool) {
        let (dc, dr) = (to.0 - from.0, to.1 - from.1);
        // Rows look twice as tall as columns are wide.
        let arrow = if dc.abs() >= 2 * dr.abs() {
            if dc > 0 {
                '>'
            } else {
                '<'
            }
        } else if dr > 0 {
            'v'
        } else {
            '^'
        };
        // The arrow takes the last step before `to`.
        let steps = dc.abs().max(dr.abs());
        let at = |d: i64| (2 * d * (steps - 1) + steps).div_euclid(2 * steps);
        let head_at = (head && steps > 1).then(|| (from.0 + at(dc), from.1 + at(dr)));
        for (col, row) in self.points(from, to) {
            if !self.taken[row][col] {
                self.cells[row][col] = if head_at == Some((col as i64, row as i64)) {
                    arrow
                } else {
                    ROAD
                };
            }
        }
    }

    /// Writes `text` from `start` on one row if every cell is in the window
    /// and free, keeping a free cell either side when `gap` is set.
    fn try_text(&mut self, text: &str, start: (i64, i64), gap: bool) -> bool {
        let width = text.chars().count() as i64;
        let pad = i64::from(gap);
        let span = (start.0 - pad)..(start.0 + width + pad);
        let free = span.clone().all(|col| {
            self.inside((col, start.1))
                .is_some_and(|(c, r)| !self.taken[r][c])
        });
        if !free {
            return false;
        }
        for (i, ch) in text.chars().enumerate() {
            let (col, row) = self.inside((start.0 + i as i64, start.1)).unwrap();
            self.cells[row][col] = ch;
        }
        for col in span {
            if let Some((c, r)) = self.inside((col, start.1)) {
                self.taken[r][c] = true;
            }
        }
        true
    }

    fn lines(self) -> Vec<String> {
        self.cells
            .into_iter()
            .map(|row| row.into_iter().collect::<String>().trim_end().to_string())
            .collect()
    }
}

/// Draws the map through a viewport: `viewport.rows` lines, without the
/// legend. Roads go first, then the places' glyphs, then labels in priority
/// order (the player, towns, castles, villages, waypoints), each tried to
/// the right, the left, above and below, and otherwise dropped.
pub fn render(world: &WorldSpec, view: &MapView, viewport: &Viewport) -> Vec<String> {
    let scale = viewport.scale(view);
    let mut canvas = Canvas::new(viewport);
    let cell_of = |location: &str| {
        let place = view.places.iter().find(|p| p.location == location)?;
        Some(viewport.cell(scale, place))
    };
    for road in &view.roads {
        if let (Some(a), Some(b)) = (cell_of(&road.between[0]), cell_of(&road.between[1])) {
            canvas.line(a, b, false);
        }
    }
    for exit in &view.exits {
        if let (Some(a), Some(b)) = (cell_of(&exit.from), cell_of(&exit.to)) {
            canvas.line(a, b, true);
        }
    }
    // The player's place first, then by kind, then in authored order.
    let mut order: Vec<&MapPlace> = view.places.iter().collect();
    order.sort_by_key(|p| (p.location != view.here, p.point.kind));
    let mut drawn: Vec<(&MapPlace, (i64, i64), usize)> = Vec::new();
    for place in order {
        let cell = viewport.cell(scale, place);
        let Some((col, row)) = canvas.inside(cell) else {
            continue;
        };
        // Another, more important place already holds this cell.
        if let Some(shared) = drawn.iter_mut().find(|(_, c, _)| *c == cell) {
            shared.2 += 1;
            continue;
        }
        canvas.cells[row][col] = if place.location == view.here {
            YOU
        } else {
            glyph(place.point.kind)
        };
        canvas.taken[row][col] = true;
        drawn.push((place, cell, 0));
    }
    let named = |kind: PlaceKind| match viewport.zoom {
        0 => kind <= PlaceKind::Castle,
        1 => kind <= PlaceKind::Village,
        _ => true,
    };
    for (place, (col, row), hidden) in &drawn {
        if place.location != view.here && !named(place.point.kind) {
            continue;
        }
        let mut label = world.location(&place.location).unwrap().name.clone();
        if *hidden > 0 {
            label += &format!(" +{hidden}");
        }
        let width = label.chars().count() as i64;
        // Beside the glyph, a space apart; the space belongs to the label.
        let _ = canvas.try_text(&format!(" {label}"), (col + 1, *row), false)
            || canvas.try_text(&format!("{label} "), (col - 1 - width, *row), false)
            || canvas.try_text(&label, (col - width / 2, row - 1), true)
            || canvas.try_text(&label, (col - width / 2, row + 1), true);
    }
    // Each road's time at the middle of the part that is in view.
    if viewport.zoom >= 2 {
        for road in &view.roads {
            if let (Some(a), Some(b)) = (cell_of(&road.between[0]), cell_of(&road.between[1])) {
                let points = canvas.points(a, b);
                let Some(&(col, row)) = points.get(points.len() / 2) else {
                    continue;
                };
                let text = duration(road.minutes);
                let width = text.chars().count() as i64;
                canvas.try_text(&text, (col as i64 - width / 2, row as i64), true);
            }
        }
    }
    canvas.lines()
}

/// The glyphs in use: the player, then each kind of place on the map.
pub fn legend(view: &MapView) -> String {
    let mut kinds: Vec<PlaceKind> = view.places.iter().map(|p| p.point.kind).collect();
    kinds.sort();
    kinds.dedup();
    let mut parts = vec![format!("{YOU} {LEGEND_YOU}")];
    parts.extend(
        kinds
            .into_iter()
            .map(|k| format!("{} {}", glyph(k), kind_name(k))),
    );
    parts.join("   ")
}

/// Line mode's map: the fit view, or a closer one centred on a place, in a
/// fixed 80 × 24 frame whose last line is the legend.
pub fn frame(world: &WorldSpec, view: &MapView, zoom: Option<(u32, &str)>) -> Option<Vec<String>> {
    let viewport = match zoom {
        None => Viewport::fit(view, LINE_COLS, LINE_ROWS),
        Some((zoom, place)) => Viewport::on(view, place, zoom, LINE_COLS, LINE_ROWS)?,
    };
    let mut lines = render(world, view, &viewport);
    lines.push(legend(view));
    Some(lines)
}

/// The places one road or exit from here, in authored order, with the
/// road's time where there is one.
fn neighbours(view: &MapView) -> Vec<(&str, Option<u64>)> {
    let here = view.here.as_str();
    let roads = view.roads.iter().filter_map(|r| {
        let [a, b] = &r.between;
        let other = if a == here {
            b
        } else if b == here {
            a
        } else {
            return None;
        };
        Some((other.as_str(), Some(r.minutes)))
    });
    let exits = view
        .exits
        .iter()
        .filter(|e| e.from == here)
        .map(|e| (e.to.as_str(), None));
    roads.chain(exits).collect()
}

/// The map on the alternate screen until Esc: zoom, pan, fit, centre and
/// step through the places one road from here. It opens centred on the
/// player at `zoom`, the level last used, or on a place `at` a level; on
/// leaving, `zoom` keeps the level. `size` is the terminal's columns and
/// rows, as last reported. Returns whether play goes on (false on quit).
pub fn explore(
    engine: &Engine<'_>,
    keys: &mut impl Iterator<Item = io::Result<Key>>,
    output: &mut impl Write,
    paint: Paint,
    size: &std::cell::Cell<(u16, u16)>,
    zoom: &mut u32,
    at: Option<(u32, &str)>,
) -> io::Result<bool> {
    let world = engine.world();
    let Some(view) = engine.map_view() else {
        return Ok(true);
    };
    let window = |(cols, rows): (u16, u16)| {
        (
            usize::from(cols.max(1)),
            usize::from(rows.saturating_sub(SCREEN_LINES).max(1)),
        )
    };
    let (cols, rows) = window(size.get());
    let (start, place) = at.unwrap_or((*zoom, &view.here));
    let Some(mut viewport) = Viewport::on(&view, place, start, cols, rows) else {
        return Ok(true);
    };
    let neighbours = neighbours(&view);
    let mut picked: Option<usize> = None;
    queue!(output, EnterAlternateScreen)?;
    let going_on = loop {
        queue!(output, MoveTo(0, 0), Clear(ClearType::All))?;
        for line in render(world, &view, &viewport) {
            writeln!(output, "{line}")?;
        }
        let name = |id: &str| world.location(id).unwrap().name.as_str();
        let status = match picked.map(|i| neighbours[i]) {
            Some((id, Some(minutes))) => format!("{} — {}", name(id), duration(minutes)),
            Some((id, None)) => name(id).to_string(),
            None => name(&view.here).to_string(),
        };
        writeln!(output, "{}", paint.title(&status))?;
        writeln!(output, "{}", legend(&view))?;
        write!(output, "{}", paint.dim(KEYS_HINT))?;
        output.flush()?;
        let key = match keys.next().transpose()? {
            None | Some(Key::Quit) => break false,
            Some(key) => key,
        };
        match key {
            Key::Esc => break true,
            Key::Char('+' | '=') => viewport.zoom = (viewport.zoom + 1).min(MAX_ZOOM),
            Key::Char('-') => viewport.zoom = viewport.zoom.saturating_sub(1),
            Key::Left => viewport.pan(&view, -1, 0),
            Key::Right => viewport.pan(&view, 1, 0),
            Key::Up => viewport.pan(&view, 0, -1),
            Key::Down => viewport.pan(&view, 0, 1),
            Key::Char('0') => {
                viewport = Viewport::fit(&view, viewport.cols, viewport.rows);
            }
            Key::Char('c') => {
                viewport = Viewport::on(
                    &view,
                    &view.here,
                    viewport.zoom,
                    viewport.cols,
                    viewport.rows,
                )
                .unwrap();
                picked = None;
            }
            Key::Tab if !neighbours.is_empty() => {
                let next = picked.map_or(0, |i| (i + 1) % neighbours.len());
                picked = Some(next);
                viewport = Viewport::on(
                    &view,
                    neighbours[next].0,
                    viewport.zoom,
                    viewport.cols,
                    viewport.rows,
                )
                .unwrap();
            }
            Key::Resize(cols, rows) => (viewport.cols, viewport.rows) = window((cols, rows)),
            _ => {}
        }
    };
    *zoom = viewport.zoom;
    queue!(output, LeaveAlternateScreen)?;
    Ok(going_on)
}

#[cfg(test)]
mod tests {
    use super::*;
    use realmkit_engine::{Engine, MapExit};
    use realmkit_spec::{Direction, MapPoint};

    fn marches() -> WorldSpec {
        WorldSpec::load(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../examples/marches"
        ))
        .unwrap()
    }

    fn view(world: &WorldSpec) -> MapView {
        Engine::new_with_seed(world, 7).unwrap().map_view().unwrap()
    }

    fn at(world: &mut WorldSpec, id: &str, x: u32, y: u32, kind: PlaceKind) {
        let location = world.locations.iter_mut().find(|l| l.id == id).unwrap();
        location.map = Some(MapPoint { x, y, kind });
    }

    #[test]
    fn the_fit_view_shows_every_place_with_its_label() {
        let world = marches();
        let lines = frame(&world, &view(&world), None).unwrap();
        assert_eq!(lines.len(), 24);
        let text = lines.join("\n");
        // Villages are unnamed when zoomed out, but always drawn.
        for shown in ["@ Greyford", "# Hollin Keep", "O Vellmarket", "o"] {
            assert!(text.contains(shown), "{shown:?} in\n{text}");
        }
        assert!(!text.contains("Ashmere"));
        assert_eq!(lines[23], "@ you   O town   # castle   o village");
        assert!(lines.iter().all(|l| l.chars().count() <= LINE_COLS));
    }

    #[test]
    fn closer_views_name_villages_then_road_times() {
        let world = marches();
        let view = view(&world);
        let one = frame(&world, &view, Some((1, "ashmere")))
            .unwrap()
            .join("\n");
        assert!(one.contains("Ashmere") && !one.contains("2 h"));
        let two = frame(&world, &view, Some((2, "ashmere")))
            .unwrap()
            .join("\n");
        assert!(two.contains("Ashmere") && two.contains("2 h"), "{two}");
        assert_eq!(frame(&world, &view, Some((1, "nowhere"))), None);
    }

    #[test]
    fn a_label_without_room_on_the_right_tries_the_left_then_drops() {
        let mut world = marches();
        // Two towns side by side: the second's label cannot go right of the
        // first's glyph, so the first takes the left.
        at(&mut world, "greyford", 1000, 1000, PlaceKind::Town);
        at(&mut world, "vellmarket", 1001, 1000, PlaceKind::Town);
        let mut view = view(&world);
        view.here = "ashmere".into();
        view.roads.clear();
        let viewport = Viewport {
            centre: (1000.0, 1000.0),
            zoom: MAX_ZOOM,
            cols: 60,
            rows: 5,
        };
        let lines = render(&world, &view, &viewport);
        assert_eq!(
            lines[2].trim(),
            "Greyford O        O Vellmarket",
            "{lines:?}"
        );
        // With no room anywhere, only the glyph remains.
        let narrow = Viewport {
            cols: 3,
            rows: 1,
            ..viewport
        };
        assert_eq!(render(&world, &view, &narrow), [" O"]);
    }

    #[test]
    fn places_sharing_a_cell_show_the_most_important_and_a_count() {
        let mut world = marches();
        at(&mut world, "ashmere", 1001, 1000, PlaceKind::Village);
        let view = view(&world);
        let text = frame(&world, &view, None).unwrap().join("\n");
        assert!(text.contains("@ Greyford +1"), "{text}");
    }

    #[test]
    fn a_one_way_exit_points_at_its_destination() {
        let world = marches();
        let mut view = view(&world);
        view.roads.clear();
        view.exits = vec![MapExit {
            from: "greyford".into(),
            to: "vellmarket".into(),
            direction: Direction::East,
        }];
        let text = frame(&world, &view, None).unwrap().join("\n");
        assert!(
            text.contains(">O Vellmarket") || text.contains("> O Vellmarket"),
            "{text}"
        );
    }
}
