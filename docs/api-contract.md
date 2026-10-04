# The discovery contract with the Nyuchi API

circles.mukoko.com reads **only** the Nyuchi API's public discovery endpoint. It has no database and no other data source. This page is the contract the Worker is built against. It was proposed on [nyuchi/api-gateway#197](https://github.com/nyuchi/api-gateway/issues/197) and is stubbed for development and tests by `scripts/stub-api.mjs` over `tests/fixtures/api.json`.

Until the API ships these routes, production answers 503 ("Circles are taking a breather") on every data page. That is deliberate: nothing here invents data.

## Rules

- **Discoverable means `public` or `broadcast`.** The API never returns a `private` or `secret` circle from any of these routes. The Worker checks again and drops anything else (`Circle::is_discoverable`), and the end-to-end tests prove it against a stub that leaks.
- **No people.** No member lists, and no post authors. On the fediverse a public post is attributed to the circle.
- **Public posts only.** A post appears here only after moderation approves it (`moderationStatus: approved`) and only in a discoverable circle.
- **Unauthenticated reads.** The routes are public. The Worker sends its own `X-Client-Id` and `X-Client-Secret` when they are configured, which identifies the caller for rate limits. It never sends a person's token.

## Routes

All routes are `GET` and return JSON.

### `GET /v1/circles/discover`

| Query      | Meaning                                                    |
| ---------- | ---------------------------------------------------------- |
| `q`        | Free text over the name and description (up to 100 chars). |
| `category` | A category slug.                                           |
| `featured` | `true`: featured circles only.                             |
| `limit`    | 1 to 50, default 24.                                       |
| `cursor`   | Opaque, from `nextCursor`.                                 |

Sorted by `memberCount`, largest first. Response:

```json
{ "data": [Circle], "nextCursor": "opaque or null", "total": 123 }
```

`total` may be omitted. When present, it counts every match, not the page.

### `GET /v1/circles/discover/categories`

```json
{ "data": [{ "slug": "sport", "name": "Sport and fitness", "description": "…", "circleCount": 12 }] }
```

### `GET /v1/circles/discover/{slug}`

Returns one `Circle`, or 404 when no discoverable circle has that slug.

### `GET /v1/circles/discover/{slug}/posts`

Takes `limit` and `cursor`. Approved public posts, newest first:

```json
{ "data": [Post], "nextCursor": null, "total": 3 }
```

Returns 404 when the circle is not discoverable. Until this route exists, the outbox page answers 501. The actor, WebFinger and the circle page still work.

### `GET /v1/circles/discover/{slug}/posts/{id}`

Returns one `Post`, or 404.

## Shapes

`Circle`:

| Field         | Type                        | Notes                                                                        |
| ------------- | --------------------------- | ---------------------------------------------------------------------------- |
| `id`          | string                      | The circle id (UUID).                                                        |
| `slug`        | string                      | `^[a-z0-9](?:[a-z0-9-]{0,62}[a-z0-9])?$`. Also the WebFinger name. Unique.   |
| `name`        | string                      |                                                                              |
| `description` | string or null              | Plain text.                                                                  |
| `circleType`  | `"public"` or `"broadcast"` |                                                                              |
| `memberCount` | number                      |                                                                              |
| `postCount`   | number                      | Public posts.                                                                |
| `inLanguage`  | string or null              | BCP 47 language tag.                                                         |
| `categories`  | `[{ slug, name }]`          |                                                                              |
| `imageUrl`    | https URL or null           |                                                                              |
| `featured`    | boolean                     |                                                                              |
| `createdAt`   | ISO 8601                    |                                                                              |
| `updatedAt`   | ISO 8601                    | Keys the Open Graph image cache.                                             |
| `place`       | `{ name }` or null          |                                                                              |
| `actorUri`    | URL                         | `https://circles.mukoko.com/c/{slug}`. The Worker always mints this itself.  |
| `links.join`  | https URL                   | The universal link that opens the circle in the super app (web when no app). |
| `links.app`   | `mukoko://…` or null        | The custom-scheme deep link.                                                 |

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

## Deep links

"Join" goes to `https://circles.mukoko.com/c/{slug}/join`. That page redirects (302) to the first of these that is safe:

1. `links.join` from the API, when it is an https URL.
2. `JOIN_URL_TEMPLATE` (a Worker var) with `{id}` and `{slug}` filled in. Today that is `https://events.mukoko.com/circles/{id}`, because Circles lives inside Mukoko Events until the super app ships.
3. `APP_WEB_URL`, when the directory is unreachable.

"Create a circle" goes to `/create`, which redirects to `CREATE_URL`.

An https universal link is the "smart" part of the fallback. When the Mukoko app is installed and claims the domain, the operating system opens the app; otherwise the browser opens the web app. App Store and Google Play links appear in the footer once `APP_STORE_URL` and `PLAY_STORE_URL` are set.
