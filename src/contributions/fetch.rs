use anyhow::{Context, Result, bail};
use chrono::{Datelike, Days, Months, NaiveDate};
use serde_json::{Value, json};

/// A single square of the calendar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Day {
    pub date: NaiveDate,
    pub count: u32,
    // 0 for no contributions up to 4 for the most
    pub level: u8,
}

/// Which stretch of time the calendar shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Period {
    LastYear,
    Year(i32),
}

impl Period {
    /// The first and last day of the period, the last day never being in the future.
    pub fn range(self, today: NaiveDate) -> (NaiveDate, NaiveDate) {
        match self {
            Period::LastYear => {
                let from = today
                    .checked_sub_months(Months::new(12))
                    .and_then(|d| d.checked_add_days(Days::new(1)))
                    .unwrap_or(today);
                (from, today)
            }
            Period::Year(year) => {
                let from = NaiveDate::from_ymd_opt(year, 1, 1).unwrap_or(today);
                let to = NaiveDate::from_ymd_opt(year, 12, 31)
                    .unwrap_or(today)
                    .min(today);
                (from, to)
            }
        }
    }

    /// The year that is highlighted in the year selector.
    pub fn year(self, today: NaiveDate) -> i32 {
        match self {
            Period::LastYear => today.year(),
            Period::Year(year) => year,
        }
    }
}

pub struct Calendar {
    pub period: Period,
    pub from: NaiveDate,
    pub to: NaiveDate,
    pub total: u32,
    pub days: Vec<Day>,
}

/// Get the contribution calendar, through GraphQL when a token is available as it
/// needs authentication, or else from the public contributions page.
pub async fn get_contributions(
    octocrab: &octocrab::Octocrab,
    username: &str,
    has_token: bool,
    period: Period,
    today: NaiveDate,
) -> Result<Calendar> {
    let (from, to) = period.range(today);

    let mut calendar = match has_token {
        true => match from_graphql(octocrab, username, from, to).await {
            Ok(calendar) => calendar,
            Err(_) => from_html(username, from, to).await?,
        },
        false => from_html(username, from, to).await?,
    };

    calendar.period = period;
    Ok(calendar)
}

async fn from_graphql(
    octocrab: &octocrab::Octocrab,
    username: &str,
    from: NaiveDate,
    to: NaiveDate,
) -> Result<Calendar> {
    let query = r#"query($login: String!, $from: DateTime!, $to: DateTime!) {
        user(login: $login) {
            contributionsCollection(from: $from, to: $to) {
                contributionCalendar {
                    totalContributions
                    weeks { contributionDays { date contributionCount contributionLevel } }
                }
            }
        }
    }"#;

    let body = json!({
        "query": query,
        "variables": {
            "login": username,
            "from": format!("{from}T00:00:00Z"),
            "to": format!("{to}T23:59:59Z"),
        },
    });

    let response: Value = octocrab.graphql(&body).await?;
    parse_graphql(&response, from, to)
}

fn parse_graphql(response: &Value, from: NaiveDate, to: NaiveDate) -> Result<Calendar> {
    let calendar = &response["data"]["user"]["contributionsCollection"]["contributionCalendar"];
    let Some(weeks) = calendar["weeks"].as_array() else {
        bail!("no contribution calendar in the GraphQL response");
    };

    let mut days = Vec::new();
    for day in weeks
        .iter()
        .filter_map(|w| w["contributionDays"].as_array())
        .flatten()
    {
        let Some(date) = day["date"]
            .as_str()
            .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
        else {
            continue;
        };

        let level = match day["contributionLevel"].as_str() {
            Some("FIRST_QUARTILE") => 1,
            Some("SECOND_QUARTILE") => 2,
            Some("THIRD_QUARTILE") => 3,
            Some("FOURTH_QUARTILE") => 4,
            _ => 0,
        };

        days.push(Day {
            date,
            count: day["contributionCount"].as_u64().unwrap_or(0) as u32,
            level,
        });
    }

    let total = calendar["totalContributions"]
        .as_u64()
        .map(|t| t as u32)
        .unwrap_or_else(|| days.iter().map(|d| d.count).sum());

    Ok(finish(days, total, from, to))
}

async fn from_html(username: &str, from: NaiveDate, to: NaiveDate) -> Result<Calendar> {
    let url = format!("https://github.com/users/{username}/contributions?from={from}&to={to}");

    let html = reqwest::Client::new()
        .get(&url)
        .header(reqwest::header::USER_AGENT, "ghfetch")
        .send()
        .await
        .with_context(|| "failed to download the contribution calendar")?
        .error_for_status()
        .with_context(|| "contribution calendar request returned an error")?
        .text()
        .await?;

    parse_html(&html, from, to)
}

/// Pull the days out of github's contribution calendar html. Each day is a
/// `<td data-date=".." data-level="..">` and its count lives in a `<tool-tip>`.
fn parse_html(html: &str, from: NaiveDate, to: NaiveDate) -> Result<Calendar> {
    let mut tooltips = std::collections::HashMap::new();
    for (tag, rest) in tags(html, "<tool-tip") {
        if let Some(id) = attr(tag, "for") {
            let text = rest.split("</tool-tip>").next().unwrap_or("");
            tooltips.insert(id, parse_count(text));
        }
    }

    let mut days = Vec::new();
    for (tag, _) in tags(html, "<td") {
        let Some(date) =
            attr(tag, "data-date").and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
        else {
            continue;
        };

        let level = attr(tag, "data-level")
            .and_then(|l| l.parse::<u8>().ok())
            .unwrap_or(0)
            .min(4);

        let count = attr(tag, "id")
            .and_then(|id| tooltips.get(id).copied())
            .unwrap_or(level as u32);

        days.push(Day { date, count, level });
    }

    if days.is_empty() {
        bail!("no contribution calendar found");
    }

    // The heading reads like "7,658 contributions in the last year"
    let total = html
        .find("js-contribution-activity-description")
        .map(|start| &html[start..])
        .and_then(|rest| rest.split_once('>'))
        .and_then(|(_, rest)| rest.split("</h2>").next())
        .and_then(first_number)
        .unwrap_or_else(|| days.iter().map(|d| d.count).sum());

    Ok(finish(days, total, from, to))
}

/// Keep only the days asked for, ordered by date.
fn finish(mut days: Vec<Day>, total: u32, from: NaiveDate, to: NaiveDate) -> Calendar {
    days.retain(|d| d.date >= from && d.date <= to);
    days.sort_by_key(|d| d.date);
    days.dedup_by_key(|d| d.date);
    Calendar {
        period: Period::LastYear,
        from,
        to,
        total,
        days,
    }
}

/// Every opening tag with the given name, alongside the text after it.
fn tags<'a>(html: &'a str, name: &'a str) -> impl Iterator<Item = (&'a str, &'a str)> {
    html.match_indices(name).filter_map(move |(start, _)| {
        let after = &html[start + name.len()..];
        // Make sure "<td" did not match "<tdx"
        if !after.starts_with(|c: char| c.is_whitespace()) {
            return None;
        }
        let end = after.find('>')?;
        Some((&after[..end], &after[end + 1..]))
    })
}

/// The value of an attribute inside a tag.
fn attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let needle = format!(" {name}=\"");
    let start = tag.find(&needle)? + needle.len();
    let len = tag[start..].find('"')?;
    Some(&tag[start..start + len])
}

/// "3 contributions on October 5th." or "No contributions on October 5th."
fn parse_count(text: &str) -> u32 {
    // Only look before " on " so the "5" in "October 5th" is never the count
    let count = text.split(" on ").next().unwrap_or("");
    first_number(count).unwrap_or(0)
}

/// The first number in the text, ignoring thousands separators.
fn first_number(text: &str) -> Option<u32> {
    let start = text.find(|c: char| c.is_ascii_digit())?;
    let digits: String = text[start..]
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == ',')
        .filter(char::is_ascii_digit)
        .collect();
    digits.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    #[test]
    fn parses_html_calendar() {
        let html = r#"
            <h2 id="js-contribution-activity-description" class="f4 text-normal mb-2">
              1,234 contributions
                in 2025
            </h2>
            <table><tbody><tr>
            <td tabindex="0" data-ix="0" style="width: 10px" data-date="2025-01-01" id="contribution-day-component-3-0" data-level="2" role="gridcell" class="ContributionCalendar-day"></td>
            <tool-tip id="tooltip-1" for="contribution-day-component-3-0" popover="manual" class="sr-only">12 contributions on January 1st.</tool-tip>
            <td tabindex="0" data-ix="0" data-date="2025-01-02" id="contribution-day-component-4-0" data-level="0" role="gridcell" class="ContributionCalendar-day"></td>
            <tool-tip id="tooltip-2" for="contribution-day-component-4-0" popover="manual" class="sr-only">No contributions on January 2nd.</tool-tip>
            <td class="ContributionCalendar-label">Mon</td>
            </tr></tbody></table>
        "#;

        let calendar = parse_html(html, date(2025, 1, 1), date(2025, 12, 31)).unwrap();
        assert_eq!(calendar.total, 1234);
        assert_eq!(
            calendar.days,
            vec![
                Day {
                    date: date(2025, 1, 1),
                    count: 12,
                    level: 2
                },
                Day {
                    date: date(2025, 1, 2),
                    count: 0,
                    level: 0
                },
            ]
        );
    }

    #[test]
    fn parses_graphql_calendar() {
        let response = json!({"data": {"user": {"contributionsCollection": {"contributionCalendar": {
            "totalContributions": 7,
            "weeks": [{"contributionDays": [
                {"date": "2024-12-31", "contributionCount": 1, "contributionLevel": "FIRST_QUARTILE"},
                {"date": "2025-01-01", "contributionCount": 7, "contributionLevel": "FOURTH_QUARTILE"},
            ]}]
        }}}}});

        let calendar = parse_graphql(&response, date(2025, 1, 1), date(2025, 12, 31)).unwrap();
        assert_eq!(calendar.total, 7);
        assert_eq!(
            calendar.days,
            vec![Day {
                date: date(2025, 1, 1),
                count: 7,
                level: 4
            }]
        );
    }

    #[test]
    fn year_range_stops_at_today() {
        let today = date(2026, 10, 5);
        assert_eq!(Period::Year(2026).range(today), (date(2026, 1, 1), today));
        assert_eq!(
            Period::Year(2024).range(today),
            (date(2024, 1, 1), date(2024, 12, 31))
        );
        assert_eq!(Period::LastYear.range(today), (date(2025, 10, 6), today));
    }
}
