# Aufseher

A regex-based Telegram anti-spam bot.

## Features

- **Pattern matching**: bans users whose names or messages match a regular expression.
- **Deobfuscation**: retries each check with spacing, invisible characters, and emoji stripped.
- **Channel bans**: bans channels posting in the group, sparing anonymous admins and the linked channel.
- **Hot reload**: reloads the pattern file when it changes.

## Installation

Download a binary from [GitHub Releases](https://github.com/k4yt3x/aufseher/releases/latest), use the container image `ghcr.io/k4yt3x/aufseher`, or install with Cargo:

```bash
cargo install --locked --git https://github.com/k4yt3x/aufseher.git
```

## Usage

Make the bot an administrator of your group with the **Delete messages** and **Ban users** rights, then start it with a pattern file:

```bash
TELEGRAM_BOT_TOKEN=<token> aufseher -c aufseher.yaml
```

```yaml
name_regexes:
  - "^[0-9]{10}a?$"
message_regexes:
  - "(?i)buynow"
```

Name patterns match display names and chat titles; message patterns match text, captions, and link URLs.

For systemd, see [`configs/aufseher.service`](configs/aufseher.service). It reads the token from `/etc/aufseher.env`.

## AI use declaration

AI tools were used to assist the design and implementation of this project. All design decisions were made by humans, and every change was reviewed and approved by a human maintainer.

## License

This project is licensed under the [ISC License](LICENSE).\
Copyright 2023-2026 K4YT3X.
