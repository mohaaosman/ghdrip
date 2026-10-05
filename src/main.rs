use crate::config_stuff::{Colors, Contributions, Image, Position, load_config};
use crate::contributions::{
    HEIGHT, Painter, Period, Placement, choose_layout, get_contributions, print_static,
    select_years, visible_width, weeks_needed,
};
use crate::display_info::{indent_width, info_lines, print_user_info};
use crate::get_avatar_image::get_image;
use crate::total_issues::get_issues;
use crate::totalstars::get_total_stars;
use crate::user_info::{UserInfo, get_user_info};
use anyhow::Result;
use chrono::Datelike;
use clap::Parser;
use std::env;
use std::io::IsTerminal;

mod config_stuff;
mod contributions;
mod display_info;
mod errors;
mod get_avatar_image;
mod total_issues;
mod totalstars;
mod user_info;

#[derive(Parser, Debug)]
#[command(
    name = "ghdrip",
    about = "A way to beautifully display your github stats",
    author = "mohaaosman"
)]
#[command(version)]
pub struct Cli {
    /// Github username or an organisation's name to fetch
    pub username: String,

    /// Do not display the profile avatar
    #[arg(long)]
    pub no_avatar: bool,

    /// Disable the colored output
    #[arg(long)]
    pub no_color: bool,

    /// Show the contribution calendar of this year instead of the last year
    #[arg(long, value_name = "YEAR")]
    pub year: Option<i32>,

    /// Exit after drawing instead of waiting to pick a year with the arrow keys
    #[arg(long)]
    pub no_interactive: bool,

    /// Do not display the contribution calendar
    #[arg(long)]
    pub no_contributions: bool,

    /// Draw the contribution calendar without animating it
    #[arg(long)]
    pub no_animation: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    if cli.no_color {
        owo_colors::set_override(false);
    }

    let username = &cli.username;

    // Take GHDRIP_TOKEN from env, if it returns "" then take it as not being there
    let token = env::var("GHDRIP_TOKEN")
        .ok()
        .filter(|token| !token.is_empty());

    let has_token = token.is_some();

    // ID of the image from the process id given by the os
    let image_id = std::process::id();

    // Load the config and use it
    let config = load_config()?;
    let fields = config.fields();

    // If --no-color is passed do no colors and no bold
    let colors = if cli.no_color {
        Colors::from_config(&config).no_color()
    } else {
        Colors::from_config(&config)
    };

    let image = Image::from_config(&config);
    let contributions = Contributions::from_config(&config);

    let today = chrono::Local::now().date_naive();
    let period = cli.year.map(Period::Year).unwrap_or(Period::LastYear);
    let show_calendar = contributions.enabled && !cli.no_contributions;

    // If pat token is avalible use that or else use unauthorized
    // requests and build the client instance
    let octocrab = match &token {
        Some(token) => octocrab::Octocrab::builder()
            .personal_token(token.clone())
            .build()?,

        None => octocrab::Octocrab::builder().build()?,
    };

    let (user_result, stars_result, issues_result, calendar_result) = tokio::join!(
        get_user_info(username, &octocrab, has_token),
        get_total_stars(&octocrab, username),
        get_issues(&octocrab, username),
        async {
            match show_calendar {
                true => {
                    Some(get_contributions(&octocrab, username, has_token, period, today).await)
                }
                false => None,
            }
        }
    );

    let user = user_result?;

    // Years to pick from, newest first, like the list next to github's calendar
    let years: Vec<i32> = (user.created_at.year()..=today.year()).rev().collect();
    if let Some(year) = cli.year
        && !years.contains(&year)
    {
        eprintln!(
            "No contributions for {year}, pick a year from {} to {}",
            user.created_at.year(),
            today.year()
        );
        std::process::exit(1);
    }

    // Organisations do not have a contribution calendar
    let is_user = user.r#type == "User";
    let total_stars = stars_result;
    let total_issues = issues_result;

    let user_info = UserInfo {
        name: user.login,
        id: user.id.0.try_into()?,
        total_stars: total_stars,
        followers: user.followers.try_into()?,
        public_repos: user.public_repos,
        issues: total_issues,
        created_at: user.created_at.format("%d-%m-%Y at %I:%M %p").to_string(),

        company: user
            .company
            .map(|b| b.split_whitespace().collect::<Vec<_>>().join(" ")),

        location: user
            .location
            .map(|b| b.split_whitespace().collect::<Vec<_>>().join(" ")),

        blog: user
            .blog
            .map(|b| b.split_whitespace().collect::<Vec<_>>().join(" ")),

        twitter_user: user
            .twitter_username
            .map(|b| b.split_whitespace().collect::<Vec<_>>().join(" ")),

        bio: user
            .bio
            .map(|b| b.split_whitespace().collect::<Vec<_>>().join(" ")),

        avatar_url: format!("{}&s=200", user.avatar_url),
    };

    let show_avatar =
        !cli.no_avatar && get_image(&user_info.avatar_url, image_id, &image).await.is_ok();

    let lines = info_lines(&user_info, &fields, &colors);

    let calendar = match calendar_result {
        Some(Ok(calendar)) if is_user => Some(calendar),
        Some(Err(e)) if is_user => {
            print_user_info(lines, &image, show_avatar, 0);
            eprintln!("Could not load the contribution calendar: {e:#}");
            return Ok(());
        }
        _ => None,
    };

    let Some(calendar) = calendar else {
        print_user_info(lines, &image, show_avatar, 0);
        return Ok(());
    };

    // Moving the cursor around to draw and animate only works in a terminal
    let is_tty = std::io::stdout().is_terminal();
    let (term_width, term_height) = crossterm::terminal::size()
        .map(|(w, h)| (w as usize, h as usize))
        .unwrap_or((120, 40));

    let interactive = !cli.no_interactive && is_tty && std::io::stdin().is_terminal();
    let animate = contributions.animate && !cli.no_animation && is_tty;

    let indent = indent_width(&image, show_avatar);
    let info_width = lines
        .iter()
        .take(HEIGHT)
        .map(|line| visible_width(line))
        .max()
        .unwrap_or(0);
    let right_col = indent + info_width + contributions.gap;
    let below_col = if show_avatar { image.left_gap } else { 0 };
    let weeks = weeks_needed(period, today, interactive);

    // Drawing on the right needs the whole info block on screen at once
    let block_height = lines.len().max(HEIGHT).max(if show_avatar {
        image.image_rows.saturating_sub(1)
    } else {
        0
    }) + 1;
    let position = match contributions.position {
        _ if !is_tty || block_height >= term_height => Position::Below,
        position => position,
    };

    let Some(layout) = choose_layout(position, term_width, right_col, below_col, weeks) else {
        print_user_info(lines, &image, show_avatar, 0);
        return Ok(());
    };

    let painter = match layout.placement {
        Placement::Right => {
            let rows_up = print_user_info(lines, &image, show_avatar, HEIGHT);
            Painter::new(
                rows_up,
                right_col,
                &layout,
                colors.enabled,
                contributions.levels,
                years,
                animate,
            )
        }

        Placement::Below if !is_tty || HEIGHT + 1 >= term_height => {
            print_user_info(lines, &image, show_avatar, 0);
            print_static(
                &calendar,
                &layout,
                colors.enabled,
                contributions.levels,
                &years,
                today,
                below_col,
            );
            return Ok(());
        }

        Placement::Below => {
            print_user_info(lines, &image, show_avatar, 0);
            // Make room for the calendar and keep the blank line at the bottom
            print!("{}", "\n".repeat(HEIGHT + 1));
            Painter::new(
                HEIGHT + 1,
                below_col,
                &layout,
                colors.enabled,
                contributions.levels,
                years,
                animate,
            )
        }
    };

    painter.show(&calendar, today, interactive)?;

    if interactive {
        select_years(&painter, &octocrab, username, has_token, calendar, today).await?;
    }

    Ok(())
}
