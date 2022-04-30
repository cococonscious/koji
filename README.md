<div align="center">
  <h1>🦊 koji</h1>

  An interactive CLI for creating [conventional commits](https://www.conventionalcommits.org/en/v1.0.0/),
  built on [cocogitto](https://github.com/oknozor/cocogitto) and inspired by
  [cz-cli](https://github.com/commitizen/cz-cli).
  
  [![Current Release](https://img.shields.io/github/v/release/its-danny/koji)](https://github.com/its-danny/koji/releases)
  [![GitHub Workflow Status](https://img.shields.io/github/workflow/status/its-danny/koji/CI)](https://github.com/its-danny/koji/actions)
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
- Use emoji 👋 (or, shortcodes)
- Autocomplete for commit scope
- Run as a git hook
- Custom commit types

## Installation

```bash
curl -sS https://webinstall.dev/koji | bash
```

Make sure to have both [git](https://git-scm.com/) and openssl installed first.

**Note:** Refer to [this comment](https://github.com/its-danny/koji/issues/53#issuecomment-1076690486)
for getting it to work on an M1 Macbook Pro.

## Usage

```bash
# Do some work
cd dev/nasa
git add stars

# Create a conventional commit
koji
```

## Using as a git hook

If you're using [rusty-hook](https://github.com/swellaby/rusty-hook), set this
in your `.rusty-hook.toml` file.

```toml
prepare-commit-msg = "koji --hook"
```

Similar should work for any hook runner, just make sure you're using
it with the `prepare-commit-msg` hook as it writes the commit
message to `COMMIT_EDITMSG`.

When using it as a hook, any message passed to `git -m` will be used
for the commit summary. Writing your commit as a conventional commit,
e.g. `git commit -m "feat(space): delete some stars"`, will bypass
koji altogether.

## Configuration

Options:

- `emoji`
- `autocomplete`
- `commit_types`

Config files are prioritized in the following order:

- Passed in via `--config`
- `.koji.toml` in the working directory
- `~/.config/koji/config.toml`
- The [default](https://github.com/its-danny/koji/blob/main/meta/config/koji-default.toml) config

You can find a few examples of commit types [here](https://github.com/its-danny/koji/blob/main/meta/config).
