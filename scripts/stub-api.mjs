// A stub of the Nyuchi API's public discovery endpoint, for local
// development, the end-to-end tests and screenshots. It implements the
// contract in docs/api-contract.md over tests/fixtures/api.json.
//
// Two deliberate faults let the tests prove the Worker's own guards:
// - `GET /v1/circles/discover/family-chat` (private) and `/surprise-party`
//   (secret) are served as if the API leaked them. The Worker must 404.
// Every circle has a posts endpoint, as in the API: a circle with no public
// posts returns an empty page. Only harare-runners has posts in the fixtures.
//
//   node scripts/stub-api.mjs [port]     (default 8788)
import { createServer } from "node:http";
import { readFileSync } from "node:fs";

const data = JSON.parse(
  readFileSync(new URL("../tests/fixtures/api.json", import.meta.url), "utf8"),
);
const port = Number(process.argv[2] ?? process.env.STUB_PORT ?? 8788);

const send = (res, status, body) => {
  res.writeHead(status, { "content-type": "application/json" });
  res.end(JSON.stringify(body));
};

const page = (items, url) => {
  const limit = Math.min(
    Math.max(Number(url.searchParams.get("limit") ?? 24), 1),
    50,
  );
  const start = Number(url.searchParams.get("cursor") ?? 0) || 0;
  const slice = items.slice(start, start + limit);
  const next = start + limit < items.length ? String(start + limit) : null;
  return { data: slice, nextCursor: next, total: items.length };
};

export function handler(req, res) {
  const url = new URL(req.url ?? "/", "http://stub");
  const parts = url.pathname.split("/").filter(Boolean);
  if (req.method !== "GET") return send(res, 405, { detail: "GET only" });
  if (parts.join("/") === "health") return send(res, 200, { ok: true });
  if (parts[0] !== "v1" || parts[1] !== "circles" || parts[2] !== "discover") {
    return send(res, 404, { detail: "Not Found" });
  }
  const rest = parts.slice(3);
  if (rest.length === 0) {
    const q = (url.searchParams.get("q") ?? "").toLowerCase();
    const category = url.searchParams.get("category");
    const featured = url.searchParams.get("featured") === "true";
    let items = data.circles
      .filter((c) => !featured || c.featured)
      .filter((c) => !category || c.categories.some((x) => x.slug === category))
      .filter(
        (c) =>
          !q ||
          c.name.toLowerCase().includes(q) ||
          (c.description ?? "").toLowerCase().includes(q),
      );
    items = [...items].sort((a, b) => b.memberCount - a.memberCount);
    if (category && !data.categories.some((c) => c.slug === category)) {
      return send(res, 200, { data: [], nextCursor: null, total: 0 });
    }
    return send(res, 200, page(items, url));
  }
  if (rest.length === 1 && rest[0] === "categories") {
    // As the API: only categories with a circle, most circles first.
    const cats = data.categories
      .filter((c) => c.circleCount > 0)
      .sort(
        (a, b) => b.circleCount - a.circleCount || a.name.localeCompare(b.name),
      );
    return send(res, 200, { data: cats, nextCursor: null, total: cats.length });
  }
  const slug = rest[0];
  const circle =
    data.circles.find((c) => c.slug === slug) ??
    data.leaked.find((c) => c.slug === slug);
  if (!circle) return send(res, 404, { detail: "Circle not found" });
  if (rest.length === 1) return send(res, 200, circle);
  if (rest[1] === "posts") {
    const posts = data.posts[slug] ?? [];
    if (rest.length === 2) return send(res, 200, page(posts, url));
    const post = posts.find((p) => p.id === rest[2]);
    return post
      ? send(res, 200, post)
      : send(res, 404, { detail: "Post not found" });
  }
  return send(res, 404, { detail: "Not Found" });
}

if (import.meta.url === `file://${process.argv[1]}`) {
  createServer(handler).listen(port, "127.0.0.1", () => {
    console.log(`stub Nyuchi API on http://127.0.0.1:${port}`);
  });
}
