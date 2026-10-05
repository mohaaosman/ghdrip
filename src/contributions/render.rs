use crate::contributions::fetch::{Calendar, Day, Period};
use chrono::{Datelike, Days, NaiveDate};

/// Height of the calendar: title, months, seven days, legend and years.
pub const HEIGHT: usize = 11;

// Space taken by the "Mon " day labels
const LABEL_WIDTH: usize = 4;

// The most columns a calendar can have, a leap year starting on a Saturday
pub const MAX_WEEKS: usize = 54;

const MUTED: (u8, u8, u8) = (139, 148, 158);
const SELECTED_BG: (u8, u8, u8) = (31, 111, 235);
const SELECTED_FG: (u8, u8, u8) = (255, 255, 255);

// Used instead of colors when colors are disabled
const PLAIN_CELLS: [&str; 5] = ["·", "░", "▒", "▓", "█"];

pub struct Style {
    pub color: bool,
    pub levels: [(u8, u8, u8); 5],
    // 2 leaves a gap between the squares, 1 packs them together
    pub cell_width: usize,
    // Only the most recent weeks are drawn when the terminal is too narrow
    pub max_weeks: usize,
}

impl Style {
    /// Visible width of a calendar with the given number of weeks.
    pub fn width_for(cell_width: usize, weeks: usize) -> usize {
        LABEL_WIDTH + weeks * cell_width - (cell_width - 1)
    }

    pub fn width(&self) -> usize {
        Self::width_for(self.cell_width, self.max_weeks)
    }
}

/// What the year selector shows.
pub struct Years<'a> {
    // Newest first
    pub all: &'a [i32],
    pub selected: i32,
    // Show the key hints for the interactive selector
    pub hint: bool,
}

/// One line of the calendar, keeping track of how wide it looks on screen.
#[derive(Default)]
struct Line {
    text: String,
    width: usize,
    color: bool,
}

impl Line {
    fn new(color: bool) -> Self {
        Line {
            color,
            ..Default::default()
        }
    }

    fn plain(&mut self, text: &str) {
        self.text.push_str(text);
        self.width += text.chars().count();
    }

    fn fg(&mut self, text: &str, (r, g, b): (u8, u8, u8)) {
        if self.color {
            self.text
                .push_str(&format!("\x1b[38;2;{r};{g};{b}m{text}\x1b[0m"));
            self.width += text.chars().count();
        } else {
            self.plain(text);
        }
    }

    fn pad_to(&mut self, width: usize) {
        if self.width < width {
            self.plain(&" ".repeat(width - self.width));
        }
    }

    fn finish(mut self, width: usize) -> String {
        self.pad_to(width);
        self.text
    }
}

/// The calendar laid out in columns of weeks, starting on a Sunday.
pub struct Grid {
    pub weeks: Vec<[Option<Day>; 7]>,
}

impl Grid {
    pub fn new(calendar: &Calendar, max_weeks: usize) -> Self {
        let start =
            calendar.from - Days::new(calendar.from.weekday().num_days_from_sunday() as u64);
        let count = (calendar.to - start).num_days() as usize / 7 + 1;

        let mut weeks = vec![[None; 7]; count];

        // Fill in every day of the period, even ones github did not send
        let mut date = calendar.from;
        while date <= calendar.to {
            let offset = (date - start).num_days() as usize;
            weeks[offset / 7][offset % 7] = Some(Day {
                date,
                count: 0,
                level: 0,
            });
            date = date + Days::new(1);
        }

        for day in &calendar.days {
            let offset = (day.date - start).num_days();
            if offset >= 0 && (offset as usize) < count * 7 {
                weeks[offset as usize / 7][offset as usize % 7] = Some(*day);
            }
        }

        // Keep the most recent weeks if they do not all fit
        let skip = count.saturating_sub(max_weeks);
        weeks.drain(..skip);

        Grid { weeks }
    }

    /// The number of frames it takes for the animation to finish.
    pub fn frames(&self) -> usize {
        // The last square starts appearing at weeks + 6, and takes up to 4
        // more frames to reach its full color
        self.weeks.len() + 6 + 5
    }
}

/// Render the calendar into lines of the same visible width. `frame` is how far
/// into the animation it is, `None` draws it finished.
pub fn render(
    calendar: &Calendar,
    grid: &Grid,
    style: &Style,
    years: &Years,
    frame: Option<usize>,
    title_override: Option<&str>,
) -> Vec<String> {
    let width = style.width();
    let mut lines = Vec::with_capacity(HEIGHT);

    lines.push(title(calendar, grid, style, frame, title_override).finish(width));
    lines.push(months(grid, style).finish(width));

    for weekday in 0..7 {
        let mut line = Line::new(style.color);
        let label = match weekday {
            1 => "Mon ",
            3 => "Wed ",
            5 => "Fri ",
            _ => "    ",
        };
        line.fg(label, MUTED);

        for (column, week) in grid.weeks.iter().enumerate() {
            if column > 0 && style.cell_width == 2 {
                line.plain(" ");
            }
            match week[weekday].and_then(|day| shown_level(day, column + weekday, frame)) {
                Some(level) => cell(&mut line, style, level),
                None => line.plain(" "),
            }
        }

        lines.push(line.finish(width));
    }

    lines.push(legend(style, years.hint, width).finish(width));
    lines.push(year_selector(style, years, width).finish(width));

    lines
}

/// The level a square shows at this point of the animation, `None` if it
/// has not appeared yet. Squares sweep in diagonally and then grow brighter.
fn shown_level(day: Day, start: usize, frame: Option<usize>) -> Option<u8> {
    match frame {
        None => Some(day.level),
        Some(frame) if frame < start => None,
        Some(frame) => Some(day.level.min((frame - start).min(4) as u8)),
    }
}

fn cell(line: &mut Line, style: &Style, level: u8) {
    let level = level.min(4) as usize;
    if style.color {
        line.fg("■", style.levels[level]);
    } else {
        line.plain(PLAIN_CELLS[level]);
    }
}

fn title(
    calendar: &Calendar,
    grid: &Grid,
    style: &Style,
    frame: Option<usize>,
    title_override: Option<&str>,
) -> Line {
    let mut line = Line::new(style.color);

    if let Some(text) = title_override {
        line.fg(text, MUTED);
        return line;
    }

    // Count up alongside the animation
    let total = match frame {
        Some(frame) => {
            let frames = grid.frames().max(1);
            (calendar.total as u64 * frame.min(frames) as u64 / frames as u64) as u32
        }
        None => calendar.total,
    };

    let noun = if calendar.total == 1 {
        "contribution"
    } else {
        "contributions"
    };

    let when = match calendar.period {
        Period::Year(year) => format!("in {year}"),
        Period::LastYear => "in the last year".to_string(),
    };

    let text = format!("{} {noun} {when}", thousands(total));
    if style.color {
        line.text.push_str(&format!("\x1b[1m{text}\x1b[0m"));
        line.width += text.chars().count();
    } else {
        line.plain(&text);
    }
    line
}

/// Month names above the first week of each month.
fn months(grid: &Grid, style: &Style) -> Line {
    let mut line = Line::new(style.color);
    line.plain(&" ".repeat(LABEL_WIDTH));

    // Where each month starts, and whether the first column is mid month
    let mut labels: Vec<(usize, NaiveDate, bool)> = Vec::new();
    for (column, week) in grid.weeks.iter().enumerate() {
        let Some(first) = week.iter().flatten().next() else {
            continue;
        };
        let starts_month = week.iter().flatten().any(|d| d.date.day() == 1);
        if starts_month {
            let day = week.iter().flatten().find(|d| d.date.day() == 1).unwrap();
            labels.push((column * style.cell_width, day.date, true));
        } else if labels.is_empty() {
            labels.push((column * style.cell_width, first.date, false));
        }
    }

    let mut used = 0;
    for (i, (position, date, starts_month)) in labels.iter().enumerate() {
        let name = date.format("%b").to_string();

        // Drop the partial first month if it would crowd the next one
        if !starts_month
            && let Some((next, _, _)) = labels.get(i + 1)
            && *next < position + name.len() + 1
        {
            continue;
        }

        if *position < used {
            continue;
        }

        line.plain(&" ".repeat(position - used));
        line.fg(&name, MUTED);
        used = position + name.len() + 1;
        line.plain(" ");
    }

    line
}

fn legend(style: &Style, hint: bool, width: usize) -> Line {
    let mut line = Line::new(style.color);

    let squares = if style.cell_width == 2 { 9 } else { 5 };
    let legend_width = "Less ".len() + squares + " More".len();

    let hint_text = "←/→ year · q quit";
    if hint && LABEL_WIDTH + hint_text.chars().count() + 2 + legend_width <= width {
        line.plain(&" ".repeat(LABEL_WIDTH));
        line.fg(hint_text, MUTED);
    }

    line.pad_to(width.saturating_sub(legend_width));
    line.fg("Less ", MUTED);
    for level in 0..5 {
        if level > 0 && style.cell_width == 2 {
            line.plain(" ");
        }
        cell(&mut line, style, level);
    }
    line.fg(" More", MUTED);

    line
}

/// A row of years, like the list next to github's calendar.
fn year_selector(style: &Style, years: &Years, width: usize) -> Line {
    let mut line = Line::new(style.color);
    line.plain(&" ".repeat(LABEL_WIDTH));

    const ITEM: usize = 6;
    let available = width.saturating_sub(LABEL_WIDTH);

    let selected = years
        .all
        .iter()
        .position(|y| *y == years.selected)
        .unwrap_or(0);

    // Show a window of years around the selected one if they do not all fit
    let (start, end) = if years.all.len() * ITEM <= available {
        (0, years.all.len())
    } else {
        let fit = (available.saturating_sub(4) / ITEM).max(1);
        let start = selected
            .saturating_sub(fit / 2)
            .min(years.all.len().saturating_sub(fit));
        (start, (start + fit).min(years.all.len()))
    };

    let windowed = end - start < years.all.len();
    if windowed {
        line.fg(if start > 0 { "‹ " } else { "  " }, MUTED);
    }

    for (i, year) in years.all[start..end].iter().enumerate() {
        let text = if start + i == selected && !style.color {
            format!("[{year}]")
        } else {
            format!(" {year} ")
        };

        if start + i == selected && style.color {
            let (br, bg, bb) = SELECTED_BG;
            let (fr, fg, fb) = SELECTED_FG;
            line.text.push_str(&format!(
                "\x1b[1;38;2;{fr};{fg};{fb};48;2;{br};{bg};{bb}m{text}\x1b[0m"
            ));
            line.width += text.chars().count();
        } else {
            line.fg(&text, MUTED);
        }
    }

    if windowed && end < years.all.len() {
        line.fg(" ›", MUTED);
    }

    line
}

/// 7658 -> "7,658"
fn thousands(n: u32) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// Visible width of text that may contain color escape codes.
pub fn visible_width(text: &str) -> usize {
    let mut width = 0;
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' && chars.clone().next() == Some(']') {
            for c in chars.by_ref() {
                if c == '\x07' {
                    break;
                }
            }
        } else if c == '\x1b' {
            // Skip the escape sequence up to its final letter
            for c in chars.by_ref() {
                if c.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            width += 1;
        }
    }
    width
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    fn sample() -> Calendar {
        let from = date(2025, 1, 1);
        let to = date(2025, 12, 31);
        let mut days = Vec::new();
        let mut d = from;
        let mut i = 0u32;
        while d <= to {
            days.push(Day {
                date: d,
                count: i % 9,
                level: (i % 5) as u8,
            });
            d = d + Days::new(1);
            i += 1;
        }
        let total = days.iter().map(|d| d.count).sum();
        Calendar {
            period: Period::Year(2025),
            from,
            to,
            total,
            days,
        }
    }

    #[test]
    fn grid_starts_on_sunday() {
        let grid = Grid::new(&sample(), MAX_WEEKS);
        // 2025-01-01 is a Wednesday
        assert!(grid.weeks[0][..3].iter().all(Option::is_none));
        assert_eq!(grid.weeks[0][3].unwrap().date, date(2025, 1, 1));
        assert_eq!(grid.weeks.len(), 53);
    }

    #[test]
    fn grid_keeps_recent_weeks() {
        let grid = Grid::new(&sample(), 10);
        assert_eq!(grid.weeks.len(), 10);
        assert_eq!(grid.weeks[9][3].unwrap().date, date(2025, 12, 31));
    }

    #[test]
    fn lines_have_the_same_width() {
        let calendar = sample();
        let years = [2026, 2025, 2024];
        let years = Years {
            all: &years,
            selected: 2025,
            hint: true,
        };

        for (color, cell_width) in [(true, 2), (true, 1), (false, 2), (false, 1)] {
            let style = Style {
                color,
                levels: [(0, 0, 0); 5],
                cell_width,
                max_weeks: 53,
            };
            let grid = Grid::new(&calendar, style.max_weeks);
            for frame in [Some(0), Some(20), None] {
                let lines = render(&calendar, &grid, &style, &years, frame, None);
                assert_eq!(lines.len(), HEIGHT);
                for line in &lines {
                    assert_eq!(visible_width(line), style.width(), "{line:?}");
                }
            }
        }
    }

    #[test]
    fn animation_counts_up() {
        let calendar = sample();
        let style = Style {
            color: false,
            levels: [(0, 0, 0); 5],
            cell_width: 2,
            max_weeks: 53,
        };
        let grid = Grid::new(&calendar, 53);
        let years = Years {
            all: &[2025],
            selected: 2025,
            hint: false,
        };

        let first = render(&calendar, &grid, &style, &years, Some(0), None);
        assert!(first[0].starts_with("0 contributions in 2025"));
        assert!(first[2..9].iter().all(|l| !l.contains('█')));

        let last = render(&calendar, &grid, &style, &years, Some(grid.frames()), None);
        assert_eq!(last, render(&calendar, &grid, &style, &years, None, None));
    }

    #[test]
    fn formats_thousands() {
        assert_eq!(thousands(7), "7");
        assert_eq!(thousands(7658), "7,658");
        assert_eq!(thousands(1234567), "1,234,567");
    }
}
