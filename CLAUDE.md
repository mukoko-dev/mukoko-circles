# CLAUDE.md

Guidance for AI assistants (and humans) working in this repository.

## Project

**mukoko-circles** — "The African digital community everyone has been looking
for. Create your own community, join others and let's be social."

A social platform for building and joining African digital communities
("circles"). Maintained by Nyuchi Web Services. Licensed MIT.

## Current state

This repository is in its **initial / greenfield stage**. As of this writing it
contains only:

- `README.md` — one-line project description
- `LICENSE` — MIT, Copyright 2026 Nyuchi Web Services
- `.gitignore` — Deno-oriented ignore rules
- `CLAUDE.md` — this file

There is **no application code, build tooling, or dependency manifest yet**. Do
not assume a framework, directory layout, or test setup exists — verify with
`ls`/`git status` before acting. When you add the first real code, update this
file to describe the actual structure.

## Runtime & tooling

The `.gitignore` is scaffolded for **[Deno](https://deno.com/)** (it ignores
`.deno/`, `coverage/`, and `*.lcov`). Treat Deno as the intended runtime unless
the project explicitly changes course.

Expected Deno workflows once code exists:

- `deno task <name>` — run tasks defined in `deno.json` / `deno.jsonc`
- `deno test` — run tests (`*_test.ts` / `*.test.ts` convention)
- `deno fmt` — format code
- `deno lint` — lint code
- `deno check` — type-check
- `deno coverage` — coverage reports (output goes to ignored `coverage/`)

When you introduce the project, create a `deno.json` (or `deno.jsonc`) with a
`tasks` block and an import map, and document the real commands here.

## Conventions

- **Language**: TypeScript on Deno. Prefer Deno standard library and explicit
  versioned imports; pin dependencies via an import map in `deno.json`.
- **Formatting / linting**: use `deno fmt` and `deno lint` defaults unless a
  config says otherwise. Do not hand-format around the formatter.
- **Tests**: colocate tests using Deno's `*_test.ts` naming, run with
  `deno test`.
- **Do not commit** `.deno/`, `coverage/`, or `*.lcov` (already ignored).

## Git workflow

- Default branch: `main`.
- Develop on feature branches; open pull requests against `main`.
- Write clear, descriptive commit messages.
- Keep this file current: whenever the structure, runtime, or workflow
  changes, update CLAUDE.md in the same change.

## For AI assistants

- This repo is nearly empty — **explore before assuming**. Run `ls` and
  `git status` rather than relying on a remembered layout.
- When scaffolding the application, prefer the smallest setup that works and
  expand this document with concrete, verified details (directory map, real
  `deno task` commands, test instructions).
- Replace the "Current state" section above once real code lands.
