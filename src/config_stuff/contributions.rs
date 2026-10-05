use crate::config_stuff::colors::Color;
use crate::config_stuff::config::Config;
use serde::Deserialize;

/// Where the contribution calendar is drawn.
#[derive(Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
#[serde(rename_all = "snake_case")]
pub enum Position {
    /// Right of the info if the terminal is wide enough, otherwise below it
    #[default]
    Auto,
    Right,
    Below,
}

#[derive(Deserialize, Default)]
pub struct ContributionsConfig {
    pub enabled: Option<bool>,
    pub position: Option<Position>,
    pub animate: Option<bool>,
    // Gap between the info text and the calendar
    pub gap: Option<usize>,
    // Colors from "no contributions" to "most contributions", exactly 5
    pub colors: Option<Vec<Color>>,
}

pub struct Contributions {
    pub enabled: bool,
    pub position: Position,
    pub animate: bool,
    pub gap: usize,
    pub levels: [(u8, u8, u8); 5],
}

// GitHub's dark theme greens
const DEFAULT_LEVELS: [(u8, u8, u8); 5] = [
    (33, 38, 45),
    (14, 68, 41),
    (0, 109, 50),
    (38, 166, 65),
    (57, 211, 83),
];

impl Contributions {
    pub fn from_config(config: &Config) -> Self {
        let c = &config.contributions;

        let levels = match &c.colors {
            Some(colors) if colors.len() == 5 => {
                let mut levels = DEFAULT_LEVELS;
                for (level, color) in levels.iter_mut().zip(colors) {
                    *level = color.to_rgb();
                }
                levels
            }
            _ => DEFAULT_LEVELS,
        };

        Contributions {
            enabled: c.enabled.unwrap_or(true),
            position: c.position.unwrap_or_default(),
            animate: c.animate.unwrap_or(true),
            gap: c.gap.unwrap_or(4),
            levels,
        }
    }
}
