# ghdrip: rename, live year picker, avatar fallback, bio links

## Goal

- Binary, config, token env, release files → `ghdrip`. README keeps ghfetch credit.
- Run `ghdrip <user>` in a terminal → stays open, arrows flip years, `q`/Enter/Esc/Ctrl-C quit.
- Avatar download/decode fails → print without avatar, never exit.
- Links in bio clickable (OSC 8). Blog + Twitter too.

## Scope

In:
- `Cargo.toml` name, clap name, `GHDRIP_TOKEN`, `~/.config/ghdrip/config.toml`, user agent, `release.yml`, README, CHANGELOG.
- Interactive by default when stdin + stdout are terminals. `-i` replaced by `--no-interactive`.
- Kitty commands use `q=2` → terminal sends no replies that the picker would read as Esc.
- Drain pending input before the picker loop.
- `get_image` failure → `show_avatar = false`.
- Bio: `http(s)://…` → link to itself, `@name` → `https://github.com/name`.
- `visible_width` skips OSC sequences.

Out:
- Old `GHFETCH_TOKEN` / `~/.config/ghfetch` fallback.
- Picker in the static (non-tty / too-short terminal) path.

## Steps

1. Rename everywhere listed.
2. `get_avatar_image.rs`: `q=2`. `main.rs`: avatar failure → no avatar.
3. `main.rs`: `--no-interactive`, interactive = tty in + out.
4. `contributions/mod.rs`: drain input before loop.
5. `display_info.rs`: `hyperlink` + `link_bio`. Blog, Twitter, Bio use them when colors on.
6. `render.rs`: `visible_width` skips `ESC ] … BEL`.
7. README + CHANGELOG.

## Done when

- `cargo build` + `cargo test` pass.
- `target/debug/ghdrip <user>` waits for keys; arrows change year.
- Bad avatar URL → info prints, no error exit.
