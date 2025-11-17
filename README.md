# deptrack

dependency tracking for rust monorepos. checks version bumps and changelogs when you modify crates.

> [!NOTE]
> State: WIP - early alpha
> works, but still adding features, if you want to help, open an issue or pr (or even better, start with discussion).
> Feedback welcome!

## what it does

- finds out current repository path, and in this path:
- scans for workspaces
- in each workspace scans for crates
- compares git branches to see what changed
- yells at you if you forgot to bump versions
- checks if changelogs are up to date
- can analyze and search fo cyclic dependencies
- outputs a nice report, json or human
- configurable severity levels for different checks

## install (or not)

```bash
cargo install --path .
```

or just use `cargo run --` if you don't feel like installing

> [!NOTE]
> midn that debug version has extra utilities onboard

## usage

basic version check between branches:

```bash
deptrack check-versions --path /home/esavier/.repos/my-side-projects origin/main my-feature-branch
```

> [!NOTE]
> --help to the rescue

## config

drop a `deptrack.toml` in your repo root if you want custom settings:
this is only needed if you want something other than the defaults

```toml
[changelog]
require = true
check_updated = true
file_name = "CHANGELOG.md"

[severity.direct]
no_version_bump = "error"
missing_changelog = "error"

[severity.transitive]
no_version_bump = "warn"
missing_changelog = "warn"
```

direct = crates you actually modified
transitive = crates that depend on what you modified

## changelog format

supports standard conventional commits style:
(at least i hope so)

[Common-Commit documentation](https://common-changelog.org/)

```markdown
# CHANGELOG

## [1.2.0]

* feat(api): added new endpoint
* fix(db): connection timeout issue
* chore: updated deps
```

or keep-a-changelog format works too

## example output

```plain
version bump analysis:
  total affected crates: 5
  crates with bumps: 3
  crates needing bumps: 2
  bump percentage: 60.0%

changelog analysis:
  total crates analyzed: 5
  crates with valid changelogs: 2
  crates missing changelogs: 1
  crates needing updates: 2
  compliance: 40.0%
```

## todo

- [ ] auto-bump versions maybe?
- [ ] better changelog generation
- [ ] git tag management
- [ ] probably other stuff
- [ ] any and all reasonable community ideas

## license

MIT or whatever, see LICENSE file
