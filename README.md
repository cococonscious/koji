<div align="center">
  <h1>🦊 koji</h1>

  An interactive CLI for creating [conventional commits](https://www.conventionalcommits.org/en/v1.0.0/),
  built on [cocogitto](https://github.com/oknozor/cocogitto) and inspired by
  [cz-cli](https://github.com/commitizen/cz-cli).
  
  [![GitHub Actions Workflow Status](https://img.shields.io/github/actions/workflow/status/cococonscious/koji/push-pr-lint-test.yml)](https://github.com/cococonscious/koji/actions/workflows/push-pr-lint-test.yml?query=branch%3Amain)
  [![Codecov](https://img.shields.io/codecov/c/gh/cococonscious/koji)](https://codecov.io/gh/cococonscious/koji)
  [![Crate Version](https://img.shields.io/crates/v/koji)](https://crates.io/crates/koji)
  [![Current Release](https://img.shields.io/github/v/release/cococonscious/koji)](https://github.com/cococonscious/koji/releases)
  [![Dependency Status](https://deps.rs/repo/github/cococonscious/koji/status.svg)](https://deps.rs/repo/github/cococonscious/koji)
  [![License](https://img.shields.io/github/license/cococonscious/koji)](LICENSE)

  [![Commit with koji](https://github.com/cococonscious/koji/raw/main/meta/demo.gif)](https://github.com/cococonscious/koji/raw/main/meta/demo.gif)
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

### Homebrew

> Community-maintained

```bash
brew install koji
```

### Arch Linux (AUR)

> Community-maintained

```bash
yay -S git-koji-bin
```

### Nix

> Community-maintained

```bash
nix-shell -p koji
```

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

Use `koji completions <SHELL>` to generate completion scripts for your shell.

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

### [prek](https://github.com/j178/prek) / [pre-commit](https://pre-commit.com/)

Add the `prepare-commit-msg` hook type and the koji hook to `.pre-commit-config.yaml`:

```yaml
default_install_hook_types: [prepare-commit-msg, ...]
repos:
  - repo: local
    hooks:
      - id: prepare-msg
        name: prepare commit message
        entry: bash -c "exec < /dev/tty && koji --hook || true"
        language: system
        stages: [prepare-commit-msg]
```

Similar should work for any hook runner, just make sure you're using
it with the `prepare-commit-msg` hook.

When using it as a hook, any message passed to `git commit -m` will be used
for the commit summary. Writing your commit as a conventional commit,
e.g. `git commit -m "feat(space): delete some stars"`, will bypass
koji altogether.

## Configuration

Config values are prioritized in the following order:

- Passed in as arguments (see: `koji --help`)
- Read from file passed in via `--config`
- `.koji.toml` in the working directory
- Unix/Redox:
  - `$XDG_CONFIG_HOME/koji/config.toml`
  - `~/.config/koji/config.toml`
- Windows:
  - `%USERPROFILE%\AppData\Roaming\koji\config.toml`
- The [default](https://github.com/cococonscious/koji/blob/main/meta/config/default.toml) config

### Options

#### `autocomplete`

- Type: `bool`
- Optional: `true`
- Description: Enables auto-complete for scope prompt via scanning commit history.
```toml
autocomplete = true
```

#### `breaking-changes`
- Type: `bool`
- Optional: `true`
- Description: Enables breaking change prompt.
```toml
breaking_changes = true
```

#### `commit-types`

- Type: `Vec<CommitType>`
- Optional: `true`
- Description: A list of commit types to use instead of the [default](https://github.com/cococonscious/koji/blob/main/meta/config/default.toml).
```toml
[[commit_types]]
name = "feat"
emoji = "✨"
description = "A new feature"
```

#### `emoji`

- Type: `bool`
- Optional: `true`
- Description: Prepend the commit summary with relevant emoji based on commit type.
```toml
emoji = true
```

#### `issues`

- Type: `bool`
- Optional: `true`
- Description: Enables issue prompt, which will append a reference to an issue in the commit body.
```toml
issues = true
```

#### `force_config_scopes`

- Type: `bool`
- Optional: `true`
- Description: When `true`, the scope prompt becomes a selection list restricted to the configured `commit_scopes`. If a single scope is auto-detected from staged changes it is pre-selected automatically.
```toml
force_config_scopes = true
```

#### `allow_empty_scope`

- Type: `bool`
- Optional: `true`
- Description: When `false`, a scope is required and cannot be left blank. Pressing `<esc>` without a detected scope will abort the commit. Defaults to `true`.
```toml
allow_empty_scope = false
```

#### `commit_scopes`

- Type: `Vec<CommitScope>`
- Optional: `true`
- Description: A list of named commit scopes. Each scope can carry a human-readable `description` (For CLI use), path `patterns` for automatic detection, and an `ast_grep` rule for content-based detection.

*For automatically assigning scopes based on the context of the change*

```toml
[[commit_scopes]]
name = "core"
description = "Changes to the core library"
patterns = "/crates/core/**/*.rs"

[[commit_scopes]]
name = "build"
patterns = ["^/build\\.rs$", "/justfile"]

[[commit_scopes]]
name = "test"
description = "Test-only changes"
patterns = "/tests/**"

# In reference the above scope \/ (TOML is weird)
[commit_scopes.ast_grep]
language = "Rust"
files = ["**/*.rs"]
rule = { kind = "function_item", has = { stopBy = "end", pattern = "#[test]" } }
```

**`patterns`** -- one or more regex strings matched against staged file paths (prefixed with `/`).

**`ast_grep`** -- an [ast-grep](https://ast-grep.github.io/) rule. When any staged file matches both the `files` filter and the structural rule, this scope is pre-assigned. Requires the `ast-grep` feature (included by default). As a warning: the rule is read from the **staged blob only **.
