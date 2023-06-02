<div align="center">
  <h1>🦊 koji</h1>

  An interactive CLI for creating [conventional commits](https://www.conventionalcommits.org/en/v1.0.0/),
  built on [cocogitto](https://github.com/oknozor/cocogitto) and inspired by
  [cz-cli](https://github.com/commitizen/cz-cli).
  
  [![Current Release](https://img.shields.io/github/v/release/its-danny/koji)](https://github.com/its-danny/koji/releases)
  [![GitHub Workflow Status](https://img.shields.io/github/actions/workflow/status/its-danny/koji/ci.yml?brainch=main)](https://github.com/its-danny/koji/actions)
  [![Codecov](https://img.shields.io/codecov/c/gh/its-danny/koji)](https://codecov.io/gh/its-danny/koji)
  [![Dependency Status](https://deps.rs/repo/github/its-danny/koji/status.svg)](https://deps.rs/repo/github/its-danny/koji)
  [![Conventional Commits](https://img.shields.io/badge/Conventional%20Commits-1.0.0-pink.svg)](https://conventionalcommits.org)
  [![License](https://img.shields.io/github/license/its-danny/koji)](LICENSE)

  [![Commit with koji](https://github.com/its-danny/koji/raw/main/meta/demo.gif)](https://github.com/its-danny/koji/raw/main/meta/demo.gif)
</div>

## Features

- Create conventional commits with ease
- Use alongside [cocogitto](https://github.com/oknozor/cocogitto)
for automatic versioning, changelog generation, and more
- Use emoji 👋 (or, [shortcodes](https://github.com/ikatyang/emoji-cheat-sheet))
- Autocomplete for commit scope
- Run as a git hook
- Custom commit types

## Installation

### webi

```bash
curl -sS https://webinstall.dev/koji | bash
```

### cargo

```bash
cargo install --locked koji
```

Be sure to have [git](https://git-scm.com/) installed first.

## Usage

The basic way to use koji is as a replacement for `git commit`,
enforcing the [conventional commit](https://www.conventionalcommits.org/en/v1.0.0/)
standard by writing your commit through an interactive prompt.

```bash
# Do some work
cd dev/koji
git add README.md

# Commit your work
koji
```

See `koji --help` for more options.

## Using as a git hook

An alternative way to use koji is as a [git hook](https://git-scm.com/book/en/v2/Customizing-Git-Git-Hooks),
running it any time you run `git commit`.

### Manually

Update `.git/hooks/prepare-commit-msg` with the following code:

```bash
#!/bin/bash
exec < /dev/tty && koji --hook || true
```

### [husky](https://github.com/typicode/husky)

```bash
npx husky add .husky/prepare-commit-msg "exec < /dev/tty && koji --hook || true
```

### [rusty-hook](https://github.com/swellaby/rusty-hook)

Add this to your `.rusty-hook.toml`:

```toml
prepare-commit-msg = "exec < /dev/tty && koji --hook || true"
```

Similar should work for any hook runner, just make sure you're using
it with the `prepare-commit-msg` hook.

When using it as a hook, any message passed to `git commit -m` will be used
for the commit summary. Writing your commit as a conventional commit,
e.g. `git commit -m "feat(space): delete some stars"`, will bypass
koji altogether.

## Configuration

Config files are prioritized in the following order:

- Passed in via `--config`
- `.koji.toml` in the working directory
- `~/.config/koji/config.toml`
- The [default](https://github.com/its-danny/koji/blob/main/meta/config/default.toml) config

### Options

#### `emoji`

- Type: `bool`
- Optional: `true`
- Description: Prepend the commit summary with relevant emoji based on commit type.
```toml
emoji = true
```

#### `autocomplete`

- Type: `bool`
- Optional: `true`
- Description: Enables auto-complete for scope prompt via scanning commit history.
```toml
autocomplete = true
```

#### `commit_types`

- Type: `Vec<CommitType>`
- Optional: `true`
- Description: A list of commit types to use instead of the [default](https://github.com/its-danny/koji/blob/main/meta/config/default.toml).
```toml
[[commit_types]]
name = "feat"
emoji = "✨"
description = "A new feature"
```
