# Mukoko Circles: circles.mukoko.com

> The public front door to Mukoko Circles: a discovery page for every open circle, and the ActivityPub identity domain that puts circles on the fediverse.

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

**Surface:** Circles, part of the [Mukoko](https://mukoko.com) super-app | **Site:** [circles.mukoko.com](https://circles.mukoko.com) | **Tracking:** [#8](https://github.com/mukoko-dev/mukoko-circles/issues/8)

---

## What this repo is, and what it is not

**Circles never has a standalone app** (owner decision, 2026-10-04). People create, join and take part in circles inside the Mukoko super-app. For now that means inside Mukoko Events (formerly nhimbe), where circles lead with their events and calendars.

This repo is **circles.mukoko.com**, which does two jobs:

1. **Discovery.** It lists every discoverable circle (`public` and `broadcast` only, never `private` or `secret`), by category, by size and by search. Each circle gets a shareable page with its own Open Graph card. The **Join** and **Create a circle** buttons send people into the super-app. The point is Mukoko's visibility: every open circle is a page search engines and link previews can find.
2. **ActivityPub identity.** Every discoverable circle is a federated `Group` actor, addressed by its Mukoko handle: the page is `https://circles.mukoko.com/c/{handle}` and WebFinger finds `@{handle}@circles.mukoko.com`. The actor id is stored once and never changes, so a renamed circle keeps its followers; old handles redirect (see [`docs/api-contract.md`](docs/api-contract.md)). Places are never actors here.

It is **not** where circles are run. It has no accounts, no posting, no moderation and no database. All data comes from the Nyuchi API's public discovery endpoint ([`docs/api-contract.md`](docs/api-contract.md)).

## How it is built

The default stack: **Astro** for the pages and **Rust** for the logic, as **one Cloudflare Worker** with static assets.

```text
request ─▶ Rust Worker (worker/, workers-rs) ─┬─▶ Nyuchi API  GET /v1/circles/discover…
                                              ├─▶ static assets (dist/, the Astro build)
                                              └─▶ nyuchi-imaging (resvg): Open Graph PNGs
```

- **`src/` (Astro, on `@bundu/ui` / Mzizi).** Pure `.astro`, no React, no client framework, no JavaScript in the browser (the CSP is `script-src 'none'`). The static pages are `/about`, `/fediverse` and the 404. The dynamic pages are built as **shells** under `/tpl/`, with `{{placeholders}}` in them, so the design stays in Astro.
- **`worker/` (Rust).** Routes every request, calls the API, fills the shells (escaping everything, in one pass), and serves the ActivityPub, WebFinger and NodeInfo documents. Every module except `entry.rs` is plain Rust tested natively. `entry.rs` is the thin wasm I/O layer.
- **Open Graph images** are drawn by the nyuchi-tools image pipeline (`nyuchi-imaging`, pinned by commit) in a Mukoko theme, and cached at the edge per circle and `updatedAt`.

Brand: Mukoko tanzanite as primary (the super-app's mineral), with terracotta as the accent (`--brand-accent`), as the Mzizi canon `circles` row says (`brand-circles.css`). British English throughout.

## URLs

| Path                                                               | What                                                                           |
| ------------------------------------------------------------------ | ------------------------------------------------------------------------------ |
| `/`                                                                | Hero, featured circles, categories, search, how it works                       |
| `/circles`, `/categories/{slug}`, `/search?q=`                     | Lists, paginated by cursor (search is `noindex`)                               |
| `/c/{handle}`                                                      | The circle page, or with `Accept: application/activity+json` the `Group` actor |
| `/c/{handle}/join`, `/create`                                      | Redirects into the super app, by device (see the contract)                     |
| `/c/{handle}/outbox`                                               | `OrderedCollection` of the circle's approved public posts                      |
| `/c/{handle}/posts/{id}`                                           | A post as a `Note` (HTML clients go to the circle page)                        |
| `/c/{handle}/inbox`, `/inbox`                                      | **501** for every method: following from other servers is phase 2              |
| `/.well-known/webfinger`                                           | `acct:{handle}@circles.mukoko.com` resolves to the actor (RFC 7033)            |
| `/.well-known/host-meta`, `/.well-known/nodeinfo`, `/nodeinfo/2.1` | Discovery for fediverse servers                                                |
| `/og/{handle}.png`, `/og/home.png`                                 | 1200×630 Open Graph cards                                                      |
| `/sitemap.xml`, `/robots.txt`                                      | For search engines                                                             |

## Federation, phase by phase

- **Phase 1 (this repo today):** read-only. Look a circle up, see it as a group, read its public posts. Private and secret circles are never federated, and posts are attributed to the circle, never to a person.
- **Phase 2:** the inbox. Accepting `Follow` and delivering new posts to followers needs a key pair per actor (`publicKey` on the actor), HTTP signatures on both sides, a followers collection and a delivery queue. Until then the inbox says so with a 501 and a link to join in Mukoko. It never pretends to accept a follow.

## Develop

Needs Node 22+, Rust (stable) with the `wasm32-unknown-unknown` target, and `worker-build` 0.8 (`scripts/build-worker.sh` installs the last two if they are missing).

```sh
npm install
node scripts/stub-api.mjs &   # the stub Nyuchi API on :8788
npm run dev                   # builds, then wrangler dev --env dev on :8787
```

| Command                                        | Runs                                                                              |
| ---------------------------------------------- | --------------------------------------------------------------------------------- |
| `npm run build`                                | Astro build, then the Worker to wasm                                              |
| `npm test`                                     | `cargo test` (unit + AS2 context checks), the build, then the end-to-end suite    |
| `npm run check`                                | `astro check` and `vp check` (oxfmt, oxlint, type check)                          |
| `npm run shots`                                | Screenshots at 375 and 1280 px, light and dark, with an axe-core pass (needs dev) |
| `cargo clippy --target wasm32-unknown-unknown` | Lints the Worker as it runs (in `worker/`)                                        |

The end-to-end suite (`tests/e2e.test.ts`) runs the built Worker under `wrangler dev` against the stub. That includes a stub that leaks a private and a secret circle, to prove the Worker refuses them.

## Deploy

One Worker, `mukoko-circles`, on the Nyuchi Web Services account, with the custom domain `circles.mukoko.com` (`wrangler.jsonc`). Workers Builds runs `npm run build`, then `npx wrangler deploy`. Observability is on.

Configuration is Worker vars (`SITE_URL`, `NYUCHI_API_URL`, `JOIN_URL_TEMPLATE`, `CREATE_URL`, `APP_WEB_URL`, and optionally `APP_STORE_URL` and `PLAY_STORE_URL`) plus two optional secrets, `NYUCHI_API_CLIENT_ID` and `NYUCHI_API_CLIENT_SECRET`, held in 1Password as `mukoko-dev/mukoko-circles`.

## Ecosystem

| Repo / service                                                            | What it is                                                    |
| ------------------------------------------------------------------------- | ------------------------------------------------------------- |
| [`nyuchi/api-gateway`](https://github.com/nyuchi/api-gateway)             | The Nyuchi API: the only writer and reader of circles data    |
| [`mukoko-dev/nhimbe`](https://github.com/mukoko-dev/nhimbe)               | Mukoko Events, where circles live until the super-app ships   |
| [`mukoko-dev/super-app-web`](https://github.com/mukoko-dev/super-app-web) | The Mukoko super-app for the web                              |
| [`nyuchi/workspace-tools`](https://github.com/nyuchi/workspace-tools)     | nyuchi-tools, whose image pipeline draws the Open Graph cards |

## Licence

Licensed under the [MIT License](LICENSE).
© 2026 Nyuchi Web Services.
