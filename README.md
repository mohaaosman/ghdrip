# ghdrip

**GH Drip**, short for GitHub Drip. A neofetch/fastfetch-style tool for GitHub profiles. Give it a username and it
prints the avatar, profile stats and an animated contribution calendar right in
your terminal.

![Preview Image](assets/preview.png)

> [!NOTE]
> ghdrip is a fork of [**ghfetch**](https://github.com/Yahddyyp/ghfetch) by
> [**Yahddyyp**](https://github.com/Yahddyyp). See [Credits](#credits).

## Features

- Avatar rendered with the Kitty graphics protocol, skipped if it cannot be loaded
- Profile stats: stars, followers, repos, issues, join date, company, location, Twitter, blog and bio
- Clickable links in the bio, blog and Twitter fields
- Animated contribution calendar with a year picker
- Fields, order and colors all set from one TOML file
- Works without a token; a GitHub token raises the rate limits

## Installation

### Requirements

- A terminal that supports Kitty's image protocol (e.g. Kitty or Ghostty) to
  show the avatar. Use `--no-avatar` on other terminals.
- [Rust](https://www.rust-lang.org/tools/install) (edition 2024) when building from source.

> [!NOTE]
> Terminals implement the Kitty image protocol differently (Ghostty uses Unicode
> placeholders, for example), so the image may be placed a line or two off.

### Prebuilt binaries

Archives for Linux and macOS (x86_64 and aarch64) are attached to each
[release](https://github.com/mohaaosman/ghdrip/releases).

### From source

```bash
git clone https://github.com/mohaaosman/ghdrip.git
cd ghdrip
cargo build --release
```

The binary is built at `target/release/ghdrip`. To put it on your `PATH`:

```bash
cargo install --path .
```

## Usage

```bash
ghdrip <username>
```

`<username>` can be a user or an organisation.

In a terminal, ghdrip stays open after drawing so you can flip through years
with `←`/`→` (or `h`/`l`). Press `q`, `Esc` or `Enter` to quit.

| Flag                 | Description                                        |
| -------------------- | -------------------------------------------------- |
| `--year <YEAR>`      | Show the contribution calendar for a calendar year |
| `--no-interactive`   | Exit after drawing instead of waiting for keys     |
| `--no-animation`     | Draw the calendar straight away                    |
| `--no-contributions` | Hide the contribution calendar                     |
| `--no-avatar`        | Hide the avatar image                              |
| `--no-color`         | Disable colored output and links                   |
| `-h`, `--help`       | Show help                                          |
| `-V`, `--version`    | Show the version                                   |

### Contribution calendar

The calendar is drawn to the right of the info when the terminal is wide enough,
and below it when it is not. Without a token it is read from GitHub's public
contributions page; with `GHDRIP_TOKEN` set it comes from the GraphQL API.

### GitHub token

Unauthenticated requests to the GitHub API have strict rate limits. Set a
[personal access token](https://github.com/settings/tokens) to raise them. It
needs no scopes.

```bash
export GHDRIP_TOKEN="your_token"
```

## Configuration

The config file lives at:

```
~/.config/ghdrip/config.toml
```

### Fields

Choose which fields are shown and in what order:

```toml
fields = [
    "user",
    { underline = "user" }, # a field name or a fixed width
    "id",
    "total_stars",
    "followers",
    "repos",
    "issues",
    "joined",
    "company",
    "location",
    "twitter",
    "blog",
    "break",
    "bio",
]
```

### Colors

```toml
name = { r = 203, g = 166, b = 247 }
# Uses the terminal's default foreground color if omitted
# underline = { r = 255, g = 255, b = 255 }
id = { r = 137, g = 220, b = 235 }
total_stars = { r = 166, g = 227, b = 161 }
followers = { r = 250, g = 179, b = 135 }
repos = { r = 116, g = 199, b = 236 }
issues = { r = 249, g = 226, b = 175 }
joined = { r = 137, g = 220, b = 235 }
company = { r = 250, g = 179, b = 135 }
location = { r = 137, g = 220, b = 235 }
twitter = { r = 203, g = 166, b = 247 }
blog = { r = 203, g = 166, b = 247 }
```

### Contribution calendar

```toml
[contributions]
enabled = true
# "auto" puts it on the right when the terminal is wide enough, else below
# "right" keeps it on the right by showing fewer weeks
position = "auto"
animate = true
# Gap between the info and the calendar
gap = 4
# Five colors from "no contributions" to "most contributions"
colors = [
    { r = 33, g = 38, b = 45 },
    { r = 14, g = 68, b = 41 },
    { r = 0, g = 109, b = 50 },
    { r = 38, g = 166, b = 65 },
    { r = 57, g = 211, b = 83 },
]
```

### Image

```toml
[image]
image_columns = 24
image_rows = 12
left_gap = 1
right_gap = 3
```

## Credits

ghdrip is built on [**ghfetch**](https://github.com/Yahddyyp/ghfetch), created
by [**Yahddyyp**](https://github.com/Yahddyyp). The original design, the
avatar rendering, the stats display and the configuration system all come from
their work. If you like this project, please go star the original.

The upstream version is also available through Homebrew:

```bash
brew install Yahddyyp/tap/ghfetch
```

## License

Released under the [MIT License](LICENSE), copyright © 2026 Yahddyyp.
