mod fetch;
mod render;

pub use fetch::{Calendar, Period, get_contributions};
pub use render::{HEIGHT, visible_width};

use crate::config_stuff::Position;
use anyhow::Result;
use chrono::NaiveDate;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use render::{Grid, MAX_WEEKS, Style, Years, render};
use std::collections::HashMap;
use std::io::{self, Write};
use std::time::Duration;

// Time between the frames of the animation
const FRAME_TIME: Duration = Duration::from_millis(14);

// Fewest weeks worth drawing when the terminal is narrow
const MIN_WEEKS: usize = 13;

/// Where the calendar goes and how big it is.
pub enum Placement {
    Right,
    Below,
}

pub struct Layout {
    pub placement: Placement,
    pub cell_width: usize,
    pub max_weeks: usize,
}

/// Pick the widest calendar that fits, on the right of the info (starting at
/// `right_col`) or below it (starting at `below_col`).
pub fn choose_layout(
    position: Position,
    term_width: usize,
    right_col: usize,
    below_col: usize,
    weeks: usize,
) -> Option<Layout> {
    // Leave the last column empty so the terminal never wraps the line
    let right = term_width.saturating_sub(right_col + 1);
    let below = term_width.saturating_sub(below_col + 1);

    let full = |available: usize| {
        [2, 1]
            .into_iter()
            .find(|&cell_width| Style::width_for(cell_width, weeks) <= available)
            .map(|cell_width| (cell_width, weeks))
    };

    let trimmed = |available: usize| {
        let fit = available.saturating_sub(Style::width_for(1, 0));
        (fit >= MIN_WEEKS).then_some((1, fit.min(weeks)))
    };

    let place = |placement: fn() -> Placement, found: Option<(usize, usize)>| {
        found.map(|(cell_width, max_weeks)| Layout {
            placement: placement(),
            cell_width,
            max_weeks,
        })
    };

    let right_full = || place(|| Placement::Right, full(right));
    let right_trimmed = || place(|| Placement::Right, trimmed(right));
    let below_full = || place(|| Placement::Below, full(below));
    let below_trimmed = || place(|| Placement::Below, trimmed(below));

    match position {
        Position::Right => right_full()
            .or_else(right_trimmed)
            .or_else(below_full)
            .or_else(below_trimmed),
        Position::Auto => right_full().or_else(below_full).or_else(below_trimmed),
        Position::Below => below_full().or_else(below_trimmed),
    }
}

/// The weeks a calendar needs, so it can be sized before it is fetched.
pub fn weeks_needed(period: Period, today: NaiveDate, interactive: bool) -> usize {
    if interactive {
        return MAX_WEEKS;
    }
    let (from, to) = period.range(today);
    let calendar = Calendar {
        period,
        from,
        to,
        total: 0,
        days: Vec::new(),
    };
    Grid::new(&calendar, MAX_WEEKS).weeks.len()
}

/// Draws the calendar into a block of lines that has already been printed.
pub struct Painter {
    // How many lines above the cursor the block starts
    pub rows_up: usize,
    // Column the block starts at, from 0
    pub col: usize,
    pub style: Style,
    pub years: Vec<i32>,
    pub animate: bool,
}

impl Painter {
    pub fn new(
        rows_up: usize,
        col: usize,
        layout: &Layout,
        color: bool,
        levels: [(u8, u8, u8); 5],
        years: Vec<i32>,
        animate: bool,
    ) -> Self {
        Painter {
            rows_up,
            col,
            style: Style {
                color,
                levels,
                cell_width: layout.cell_width,
                max_weeks: layout.max_weeks,
            },
            years,
            animate,
        }
    }

    /// Draw the calendar, sweeping the squares in when animations are enabled.
    pub fn show(&self, calendar: &Calendar, today: NaiveDate, hint: bool) -> Result<()> {
        let grid = Grid::new(calendar, self.style.max_weeks);
        let years = Years {
            all: &self.years,
            selected: calendar.period.year(today),
            hint,
        };

        if self.animate {
            print!("\x1b[?25l");
            for frame in 0..=grid.frames() {
                self.paint(&render(
                    calendar,
                    &grid,
                    &self.style,
                    &years,
                    Some(frame),
                    None,
                ))?;
                std::thread::sleep(FRAME_TIME);
            }
            print!("\x1b[?25h");
        }

        self.paint(&render(calendar, &grid, &self.style, &years, None, None))
    }

    /// Draw the calendar with a message in place of the title.
    fn message(&self, calendar: &Calendar, selected: i32, message: &str) -> Result<()> {
        let grid = Grid::new(calendar, self.style.max_weeks);
        let years = Years {
            all: &self.years,
            selected,
            hint: true,
        };
        self.paint(&render(
            calendar,
            &grid,
            &self.style,
            &years,
            None,
            Some(message),
        ))
    }

    /// Move up into the block, write each line at the right column, then go back
    /// to where the cursor was.
    fn paint(&self, lines: &[String]) -> Result<()> {
        let mut out = String::new();

        if self.rows_up > 0 {
            out.push_str(&format!("\x1b[{}A", self.rows_up));
        }

        for (i, line) in lines.iter().enumerate() {
            out.push_str(&format!("\x1b[{}G{line}", self.col + 1));
            if i + 1 < lines.len() {
                out.push_str("\x1b[1B");
            }
        }

        let down = self.rows_up.saturating_sub(lines.len().saturating_sub(1));
        if down > 0 {
            out.push_str(&format!("\x1b[{down}B"));
        }
        out.push('\r');

        let mut stdout = io::stdout().lock();
        stdout.write_all(out.as_bytes())?;
        stdout.flush()?;
        Ok(())
    }
}

/// Let the user flip through the years with the arrow keys until they quit.
pub async fn select_years(
    painter: &Painter,
    octocrab: &octocrab::Octocrab,
    username: &str,
    has_token: bool,
    first: Calendar,
    today: NaiveDate,
) -> Result<()> {
    crossterm::terminal::enable_raw_mode()?;
    print!("\x1b[?25l");

    let result = selector_loop(painter, octocrab, username, has_token, first, today).await;

    print!("\x1b[?25h");
    crossterm::terminal::disable_raw_mode()?;
    result
}

async fn selector_loop(
    painter: &Painter,
    octocrab: &octocrab::Octocrab,
    username: &str,
    has_token: bool,
    first: Calendar,
    today: NaiveDate,
) -> Result<()> {
    let mut period = first.period;
    let mut calendars: HashMap<Period, Calendar> = HashMap::new();
    calendars.insert(period, first);

    while event::poll(Duration::ZERO)? {
        event::read()?;
    }

    loop {
        let Event::Key(key) = event::read()? else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }

        let index = painter
            .years
            .iter()
            .position(|y| *y == period.year(today))
            .unwrap_or(0);

        // The years are listed newest first, so right goes back in time
        let index = match key.code {
            KeyCode::Right | KeyCode::Down | KeyCode::Char('l') | KeyCode::Char('j') => {
                (index + 1).min(painter.years.len().saturating_sub(1))
            }
            KeyCode::Left | KeyCode::Up | KeyCode::Char('h') | KeyCode::Char('k') => {
                index.saturating_sub(1)
            }
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => break,
            KeyCode::Char('q') | KeyCode::Esc | KeyCode::Enter => break,
            _ => continue,
        };

        let Some(&year) = painter.years.get(index) else {
            continue;
        };
        let next = Period::Year(year);
        if next == period {
            continue;
        }

        if !calendars.contains_key(&next) {
            let current = &calendars[&period];
            painter.message(current, year, &format!("Loading {year}…"))?;

            match get_contributions(octocrab, username, has_token, next, today).await {
                Ok(calendar) => {
                    calendars.insert(next, calendar);
                }
                Err(_) => {
                    painter.message(
                        current,
                        period.year(today),
                        &format!("Could not load {year}"),
                    )?;
                    continue;
                }
            }
        }

        period = next;
        painter.show(&calendars[&period], today, true)?;
    }

    // Redraw without the key hints now that the selector is done
    let grid = Grid::new(&calendars[&period], painter.style.max_weeks);
    let years = Years {
        all: &painter.years,
        selected: period.year(today),
        hint: false,
    };
    painter.paint(&render(
        &calendars[&period],
        &grid,
        &painter.style,
        &years,
        None,
        None,
    ))
}

/// Print the calendar as plain lines, for when the cursor cannot be moved around.
pub fn print_static(
    calendar: &Calendar,
    layout: &Layout,
    color: bool,
    levels: [(u8, u8, u8); 5],
    years: &[i32],
    today: NaiveDate,
    indent: usize,
) {
    let style = Style {
        color,
        levels,
        cell_width: layout.cell_width,
        max_weeks: layout.max_weeks,
    };
    let grid = Grid::new(calendar, style.max_weeks);
    let years = Years {
        all: years,
        selected: calendar.period.year(today),
        hint: false,
    };
    for line in render(calendar, &grid, &style, &years, None, None) {
        println!("{}{}", " ".repeat(indent), line.trim_end());
    }
    println!();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefers_the_right_side() {
        let layout = choose_layout(Position::Auto, 200, 80, 1, 53).unwrap();
        assert!(matches!(layout.placement, Placement::Right));
        assert_eq!(layout.cell_width, 2);
    }

    #[test]
    fn packs_squares_before_moving_below() {
        let layout = choose_layout(Position::Auto, 145, 80, 1, 53).unwrap();
        assert!(matches!(layout.placement, Placement::Right));
        assert_eq!(layout.cell_width, 1);
    }

    #[test]
    fn falls_back_below_when_narrow() {
        let layout = choose_layout(Position::Auto, 106, 80, 1, 53).unwrap();
        assert!(matches!(layout.placement, Placement::Below));
        assert_eq!(layout.cell_width, 1);
        assert_eq!(layout.max_weeks, 53);

        let layout = choose_layout(Position::Right, 106, 80, 1, 53).unwrap();
        assert!(matches!(layout.placement, Placement::Right));
        assert_eq!(layout.max_weeks, 21);

        let layout = choose_layout(Position::Auto, 40, 80, 1, 53).unwrap();
        assert!(matches!(layout.placement, Placement::Below));
        assert_eq!(layout.max_weeks, 34);

        assert!(choose_layout(Position::Auto, 10, 80, 1, 53).is_none());
    }
}
