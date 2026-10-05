# The discovery contract with the Nyuchi API

circles.mukoko.com reads **only** the Nyuchi API's public discovery endpoint. It has no database and no other data source. This page is the contract the Worker is built against. It was proposed on [nyuchi/api-gateway#197](https://github.com/nyuchi/api-gateway/issues/197), and the API implements it ([nyuchi/api-gateway#211](https://github.com/nyuchi/api-gateway/pull/211)). The API's copy is [`docs/architecture/circles.md`](https://github.com/nyuchi/api-gateway/blob/staging/docs/architecture/circles.md) ("Public discovery and identity"). A change to one changes both. `scripts/stub-api.mjs` implements the same shapes over `tests/fixtures/api.json` for development and tests.

When the API is unreachable, the data pages answer 503 ("Circles are taking a breather"). That is deliberate: nothing here invents data.

## Who serves what

- **The API serves data:** the routes below, and nothing in ActivityPub.
- **This site is the one ActivityPub host for circles.mukoko.com.** It mints the `Group` actor, the outbox, each `Note`, WebFinger, host-meta and NodeInfo from the data routes. The API serves no ActivityPub or WebFinger route of its own.

## Rules

- **Discoverable means `public` or `broadcast`.** The API never returns a `private` or `secret` circle from any of these routes. The Worker checks again and drops anything else (`Circle::is_discoverable`), and the end-to-end tests prove it against a stub that leaks.
- **No people.** No member lists, and no post authors. On the fediverse a public post is attributed to the circle.
- **Public posts only.** A post appears here only after moderation approves it (`moderationStatus: approved`) and only in a discoverable circle.
- **Unauthenticated reads.** The routes are public. The Worker sends its own `X-Client-Id` and `X-Client-Secret` when they are configured, which identifies the caller for rate limits. It never sends a person's token.

## Routes

All routes are `GET` and return JSON.

### `GET /v1/circles/discover`

| Query      | Meaning                                                                      |
| ---------- | ---------------------------------------------------------------------------- |
| `q`        | Free text over the name and description (up to 100 chars). Case-insensitive. |
| `category` | A category slug.                                                             |
| `featured` | `true`: featured circles only.                                               |
| `limit`    | 1 to 50, default 24.                                                         |
| `cursor`   | Opaque, from `nextCursor`.                                                   |

Sorted by `memberCount`, largest first. Response:

```json
{ "data": [Circle], "nextCursor": "opaque or null", "total": 123 }
```

`total` may be omitted. When present, it counts every match, not the page. The API sends it. `cursor` is opaque: pass back `nextCursor`. A cursor the API did not issue is a 422.

### `GET /v1/circles/discover/categories`

The interest categories (`engagement.interestCategories`) with at least one discoverable circle, most circles first.

```json
{ "data": [{ "slug": "sport", "name": "Sport and fitness", "description": "…", "circleCount": 12 }] }
```

### `GET /v1/circles/discover/{key}`

Returns one `Circle`, or 404 when no discoverable circle answers to `key`. A circle answers to its slug, its current handle and its retired handles, compared case-insensitively. The Worker then checks the answer itself (`Circle::answers_to`): a circle that does not list the requested name as its handle, slug or one of its `aliases` is a 404.

The Worker calls the posts routes below with the circle's **slug**, the permanent key, whatever name the request used.

### `GET /v1/circles/discover/{slug}/posts`

Takes `limit` and `cursor`. Approved public posts, newest first:

```json
{ "data": [Post], "nextCursor": null, "total": 3 }
```

Returns 404 when the circle is not discoverable. A discoverable circle with no public posts returns an empty page.

### `GET /v1/circles/discover/{slug}/posts/{id}`

Returns one `Post`, or 404. A post is only ever served when it is `approved` and `visibility: public`.

## Shapes

`Circle`:

| Field         | Type                        | Notes                                                                                                                                                                     |
| ------------- | --------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `id`          | string                      | The circle id (UUID).                                                                                                                                                     |
| `slug`        | string                      | `^[a-z0-9](?:[a-z0-9-]{0,62}[a-z0-9])?$`. Permanent and unique. The address until the circle has a handle.                                                                |
| `handle`      | string or null              | The circle's current Mukoko handle (the one namespace, 3 to 30 of `A-Za-z0-9_`, in the case its admins chose). The address: `/c/{handle lower-cased}`, `acct:{handle}@…`. |
| `aliases`     | string[]                    | The other names it answers to: retired handles and, once it has a handle, its slug. Requested on HTML, they 301 to the current address.                                   |
| `name`        | string                      |                                                                                                                                                                           |
| `description` | string or null              | Plain text.                                                                                                                                                               |
| `circleType`  | `"public"` or `"broadcast"` |                                                                                                                                                                           |
| `memberCount` | number                      |                                                                                                                                                                           |
| `postCount`   | number                      | Public posts: `approved` with `visibility: public`.                                                                                                                       |
| `inLanguage`  | string or null              | BCP 47 language tag.                                                                                                                                                      |
| `categories`  | `[{ slug, name }]`          |                                                                                                                                                                           |
| `imageUrl`    | https URL or null           | The API sends only https.                                                                                                                                                 |
| `featured`    | boolean                     | Set by platform staff (`POST /v1/admin/circles/{id}/feature`).                                                                                                            |
| `createdAt`   | ISO 8601 (UTC, `Z`)         |                                                                                                                                                                           |
| `updatedAt`   | ISO 8601 (UTC, `Z`)         | Keys the Open Graph image cache.                                                                                                                                          |
| `place`       | `{ name }` or null          |                                                                                                                                                                           |
| `actorUri`    | URL                         | Stored once, from the circle's first address, and never rewritten. The Worker uses it as the actor id when it is a `/c/{key}` URL on this site, else derives one.         |
| `links.join`  | https URL                   | Informational. The Worker no longer routes on it: "Join" follows the Mukoko link rules below.                                                                             |
| `links.app`   | `mukoko://…` or null        | The custom-scheme deep link. Null until the super app ships.                                                                                                              |

`Post`:

| Field           | Type           | Notes                               |
| --------------- | -------------- | ----------------------------------- |
| `id`            | string         |                                     |
| `headline`      | string or null |                                     |
| `articleBody`   | string         | Plain text; blank lines are breaks. |
| `datePublished` | ISO 8601       |                                     |
| `inLanguage`    | string or null |                                     |
| `tags`          | string[]       | Hashtags, without `#`.              |

Every optional field may be missing. The Worker defaults it rather than failing the page.

## Handles and stable actor ids

Owner decision, 2026-10-05: circles are addressed by **Mukoko handle**, as entities are on kweli.mukoko.com (nyuchi/api-gateway `docs/architecture/activitypub.md`, "Handles are the identity" and "Stability").

- The page is `/c/{handle}`, lower-cased; a path in another case 301s to it. `preferredUsername` and the `acct:` keep the case the admins chose.
- The actor `id`, `inbox`, `outbox` and every `Note` id hang off the **stored** `actorUri`. `url` and `preferredUsername` follow the current handle. After a rename they differ, and that is what keeps followers and stored objects working.
- A retired handle or the slug: a browser gets a 301 to `/c/{current}`; an ActivityPub request gets the actor with its stored id; WebFinger answers with the current `subject` and the stored id as `self`.
- Until the API sends `handle`, the slug is the address, and nothing changes for existing circles.

The API side (circle handles in the one registry, `handle` and `aliases` on the discovery shape, resolution by retired handle) is [nyuchi/api-gateway#231](https://github.com/nyuchi/api-gateway/issues/231).

## Deep links

"Join" goes to `https://circles.mukoko.com/c/{key}/join`, which redirects (302, `private, no-store`) by device:

1. **A phone or tablet** goes to the Mukoko link, `JOIN_URL_TEMPLATE` = `https://mukoko.com/open/circles/{handle}`. That page offers the Mukoko app (or its store listing).
2. **Anything else** goes to the circle in the web super app, `WEB_JOIN_URL_TEMPLATE`. Today that is `https://events.mukoko.com/circles/{id}`, because Circles lives inside Mukoko Events until the super app ships. A desktop never goes through `/open`: `/open` sends desktops to the public page, which is this site.
3. `APP_WEB_URL`, when the directory is unreachable.

Templates take `{id}`, `{slug}` and `{handle}` (the circle's path key).

"Create a circle" goes to `/create`, which redirects to `CREATE_URL`. That is `https://events.mukoko.com/circles?create=1`, Mukoko Events' create-circle form ([mukoko-dev/mukoko-events#159](https://github.com/mukoko-dev/mukoko-events/issues/159)). The form signs the person in first if needed, creates the circle with `POST /v1/circles`, and opens its page. A public or broadcast circle appears here within five minutes. `/open` has no "new circle" link, so this stays a web URL.
