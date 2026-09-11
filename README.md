# Mukoko Circles

> Interest groups, fan communities and professional networks — the persistent social fabric of [Mukoko](https://mukoko.com).

[![Lint](https://github.com/mukoko-dev/mukoko-circles/actions/workflows/lint.yml/badge.svg)](https://github.com/mukoko-dev/mukoko-circles/actions/workflows/lint.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
![Status](https://img.shields.io/badge/status-not%20yet%20implemented-lightgrey?style=flat-square)

**Surface:** Circles, one of the Mukoko mini-apps | **Ecosystem:** [mukoko.com/surfaces](https://mukoko.com/surfaces) | **Code in this repo:** none yet

---

## Read this first

**Nothing is implemented here.** The repository holds an MIT licence, the
shared lint configuration, a `.gitignore`, and this file. There is no
application, no package manifest, and no build.

What follows describes what Circles is meant to be, drawn from the places in
the estate where it is already defined — the repo description, the Mukoko
surface registry, and the Mukoko monorepo's own documentation. It is not a
description of anything that runs.

## What Circles is

Circles is the community surface of Mukoko: the place where people create
their own community, join someone else's, and stay in it. Interest groups, fan
communities, professional networks — the persistent social fabric underneath
the more transient surfaces. Where Campfire carries the conversation and
Nhimbe carries the gathering, Circles carries the group itself, which outlasts
both.

It is one of the mini-apps listed in Mukoko's surface registry, and it is
listed as a core app in the [`mukoko-dev/mukoko`](https://github.com/mukoko-dev/mukoko)
monorepo README.

## Status, honestly

Circles has a name, a description, a place in the ecosystem diagram and a
reserved repository. It does not yet have code, and it is not live —
`mukoko.com/circles` does not resolve.

Two things a contributor should know before starting work here:

- **Where Circles will be built is not settled.** The `mukoko-dev/mukoko`
  monorepo README reserves `mini-apps/circles/` for the Circles UI and names
  `mukoko-connect` as its backend. Neither exists: the monorepo has no
  `mini-apps/circles/` directory, and there is no `mukoko-connect` repository.
  This repo and that reserved slot are two plausible homes for the same thing
  and nobody has chosen between them. Ask before assuming.
- **The runtime is implied, not decided.** The only technical choice recorded
  anywhere in this repo is the `.gitignore`, which is Deno's. That is a hint,
  not a commitment, and it predates any code.

## Ecosystem

| Repo / service                                                        | What it is                                             |
| --------------------------------------------------------------------- | ------------------------------------------------------ |
| [`mukoko-dev/mukoko`](https://github.com/mukoko-dev/mukoko)           | The super app — shell, mini-apps, services             |
| [`mukoko-dev/mukoko-auth`](https://github.com/mukoko-dev/mukoko-auth) | Mukoko ID, the identity every surface signs in with    |
| [mukoko.com/surfaces](https://mukoko.com/surfaces)                    | The public list of Mukoko surfaces, Circles among them |

## Licence

Licensed under the [MIT License](LICENSE).
© 2026 Nyuchi Web Services.
