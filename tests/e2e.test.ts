// End to end: the built Worker under `wrangler dev`, against the stub of the
// Nyuchi API (scripts/stub-api.mjs). Needs `npm run build` first; `npm test`
// does that.
//
// What it holds the Worker to:
// - pages render with live data and no placeholder survives;
// - content negotiation on /c/{slug}: HTML for people, the Group actor for
//   ActivityPub clients;
// - WebFinger (RFC 7033) and NodeInfo 2.1 shapes;
// - private and secret circles never show, even when the API leaks them;
// - the inbox answers 501, never a fake 200;
// - names are escaped, and the CSP forbids scripts.
import { spawn, type ChildProcess } from "node:child_process";
import { createServer, type Server } from "node:http";
import { existsSync } from "node:fs";
import { afterAll, beforeAll, describe, expect, it } from "vite-plus/test";
import { handler } from "../scripts/stub-api.mjs";

const STUB_PORT = 8788;
const PORT = 8799;
const BASE = `http://127.0.0.1:${PORT}`;
const SITE = "https://circles.mukoko.com";
const AP = "application/activity+json";

let stub: Server;
let wrangler: ChildProcess;

async function waitFor(url: string, ms: number): Promise<void> {
  const until = Date.now() + ms;
  while (Date.now() < until) {
    try {
      const r = await fetch(url);
      if (r.status < 500) return;
    } catch {
      // not up yet
    }
    await new Promise((r) => setTimeout(r, 500));
  }
  throw new Error(`${url} did not come up`);
}

beforeAll(async () => {
  if (
    !existsSync("worker/build/index.js") ||
    !existsSync("dist/tpl/home.html")
  ) {
    throw new Error("Build first: npm run build");
  }
  stub = createServer(handler);
  await new Promise<void>((r) => stub.listen(STUB_PORT, "127.0.0.1", r));
  wrangler = spawn(
    "npx",
    [
      "wrangler",
      "dev",
      "--env",
      "dev",
      "--port",
      String(PORT),
      "--ip",
      "127.0.0.1",
      "--show-interactive-dev-session=false",
    ],
    {
      stdio: "ignore",
      env: { ...process.env, WRANGLER_SEND_METRICS: "false" },
    },
  );
  await waitFor(`${BASE}/about`, 180_000);
});

afterAll(async () => {
  wrangler?.kill("SIGTERM");
  await new Promise<void>((r) => (stub ? stub.close(() => r()) : r()));
});

const get = (path: string, accept?: string) =>
  fetch(BASE + path, {
    redirect: "manual",
    headers: accept ? { accept } : {},
  });

function noPlaceholders(html: string) {
  // The kwaito-forever fixture's description contains a literal "{{title}}"
  // on purpose: it must come through as text, so it is the one allowed.
  html = html.replaceAll("&lt;/script&gt; {{title}}", "");
  expect(html).not.toMatch(/\{\{\{?[a-z_]+\}?\}\}/);
}

describe("pages", () => {
  it("home lists featured circles, categories and the count", async () => {
    const r = await get("/");
    expect(r.status).toBe(200);
    expect(r.headers.get("content-security-policy")).toContain(
      "script-src 'none'",
    );
    const html = await r.text();
    noPlaceholders(html);
    expect(html).toContain("Lagos Design Guild");
    expect(html).toContain('href="/categories/sport"');
    expect(html).toContain("9 open circles");
    expect(html).toContain(
      '<link rel="canonical" href="https://circles.mukoko.com/">',
    );
    expect(html).toContain('"@type":"SearchAction"');
    expect(html).not.toMatch(/<script(?![^>]*application\/ld\+json)/);
  });

  it("escapes what the API sends", async () => {
    const html = await (await get("/c/kwaito-forever")).text();
    expect(html).toContain("Kwaito Forever &lt;3 &amp; &quot;friends&quot;");
    expect(html).toContain("&lt;script&gt;alert(1)&lt;/script&gt; {{title}}");
    expect(html).not.toContain("<script>alert(1)");
  });

  it("a category page filters and names the category", async () => {
    const r = await get("/categories/tech");
    expect(r.status).toBe(200);
    const html = await r.text();
    noPlaceholders(html);
    expect(html).toContain(
      '<h1 class="font-serif text-h1 text-balance">Technology</h1>',
    );
    expect(html).toContain("Nairobi JS");
    expect(html).not.toContain("Harare Runners");
    expect((await get("/categories/no-such-thing")).status).toBe(404);
  });

  it("search is noindex and keeps the query", async () => {
    const html = await (await get("/search?q=runs")).text();
    expect(html).toContain('content="noindex, follow"');
    expect(html).toContain('value="runs"');
    expect(html).toContain("Harare Runners");
    const empty = await (await get("/search?q=zzzz")).text();
    expect(empty).toContain('data-state="empty"');
    expect((await get("/search?q=")).status).toBe(302);
  });

  it("all circles paginates with a cursor", async () => {
    const html = await (await get("/circles")).text();
    expect(html).toContain("9 circles");
    expect(html).not.toContain('rel="next"');
  });

  it("a circle page has OG tags, the actor link and public posts", async () => {
    const r = await get("/c/harare-runners");
    expect(r.status).toBe(200);
    expect(r.headers.get("vary")).toContain("Accept");
    const html = await r.text();
    noPlaceholders(html);
    expect(html).toContain(
      '<meta property="og:image" content="https://circles.mukoko.com/og/harare-runners.png">',
    );
    expect(html).toContain(
      '<link rel="alternate" type="application/activity+json" href="https://circles.mukoko.com/c/harare-runners">',
    );
    expect(html).toContain('href="/c/harare-runners/join"');
    expect(html).toContain("Long run this Saturday");
    expect(html).toContain("@harare-runners@circles.mukoko.com");
  });

  it("private and secret circles never show, even if the API leaks them", async () => {
    for (const slug of ["family-chat", "surprise-party"]) {
      expect((await get(`/c/${slug}`)).status).toBe(404);
      expect((await get(`/c/${slug}`, AP)).status).toBe(404);
      expect(
        (
          await get(
            `/.well-known/webfinger?resource=acct:${slug}@circles.mukoko.com`,
          )
        ).status,
      ).toBe(404);
      expect((await get(`/og/${slug}.png`)).status).toBe(404);
    }
    const sitemap = await (await get("/sitemap.xml")).text();
    expect(sitemap).not.toContain("family-chat");
    expect(sitemap).toContain(
      `<loc>${SITE}/c/harare-runners</loc><lastmod>2026-09-30</lastmod>`,
    );
  });

  it("unknown circles and shells 404", async () => {
    expect((await get("/c/no-such-circle")).status).toBe(404);
    expect((await get("/c/Not_A_Slug")).status).toBe(404);
    expect((await get("/tpl/home.html")).status).toBe(404);
  });

  it("static pages carry the security headers", async () => {
    const r = await get("/about");
    expect(r.status).toBe(200);
    expect(r.headers.get("content-security-policy")).toContain(
      "script-src 'none'",
    );
    expect(r.headers.get("x-content-type-options")).toBe("nosniff");
  });
});

describe("buttons", () => {
  it("join redirects into the super app", async () => {
    const r = await get("/c/harare-runners/join");
    expect(r.status).toBe(302);
    expect(r.headers.get("location")).toBe(
      "https://events.mukoko.com/circles/0b8f6a52-1d0e-4c55-9a51-5f0d1c1a0001",
    );
  });

  it("create goes to the create flow", async () => {
    const r = await get("/create");
    expect(r.status).toBe(302);
    expect(r.headers.get("location")).toBe(
      "https://events.mukoko.com/circles?create=1",
    );
  });
});

describe("ActivityPub", () => {
  it("serves the Group actor on content negotiation", async () => {
    for (const accept of [
      AP,
      'application/ld+json; profile="https://www.w3.org/ns/activitystreams"',
    ]) {
      const r = await get("/c/harare-runners", accept);
      expect(r.status).toBe(200);
      expect(r.headers.get("content-type")).toContain(AP);
      const a = await r.json();
      expect(a["@context"][0]).toBe("https://www.w3.org/ns/activitystreams");
      expect(a.id).toBe(`${SITE}/c/harare-runners`);
      expect(a.type).toBe("Group");
      expect(a.preferredUsername).toBe("harare-runners");
      expect(a.inbox).toBe(`${SITE}/c/harare-runners/inbox`);
      expect(a.outbox).toBe(`${SITE}/c/harare-runners/outbox`);
    }
  });

  it("a browser Accept header gets HTML", async () => {
    const r = await get(
      "/c/harare-runners",
      "text/html,application/xhtml+xml,*/*;q=0.8",
    );
    expect(r.headers.get("content-type")).toContain("text/html");
  });

  it("outbox and its first page", async () => {
    const o = await (await get("/c/harare-runners/outbox")).json();
    expect(o.type).toBe("OrderedCollection");
    expect(o.first).toBe(`${SITE}/c/harare-runners/outbox?page=true`);
    const p = await (await get("/c/harare-runners/outbox?page=true")).json();
    expect(p.type).toBe("OrderedCollectionPage");
    expect(p.orderedItems).toHaveLength(3);
    expect(p.orderedItems[0].type).toBe("Create");
    expect(p.orderedItems[0].object.attributedTo).toBe(
      `${SITE}/c/harare-runners`,
    );
  });

  it("a circle with no public posts has an empty outbox page", async () => {
    const r = await get("/c/mbira-collective/outbox?page=true");
    expect(r.status).toBe(200);
    expect((await r.json()).orderedItems).toEqual([]);
  });

  it("a post resolves to a Note", async () => {
    const id = "a1000000-0000-4000-8000-000000000001";
    const r = await get(`/c/harare-runners/posts/${id}`, AP);
    expect(r.status).toBe(200);
    const n = await r.json();
    expect(n.type).toBe("Note");
    expect(n.id).toBe(`${SITE}/c/harare-runners/posts/${id}`);
    expect((await get(`/c/harare-runners/posts/${id}`)).status).toBe(303);
  });

  it("the inbox is honest: 501 for every method", async () => {
    for (const method of ["GET", "POST"]) {
      const r = await fetch(`${BASE}/c/harare-runners/inbox`, {
        method,
        headers: { "content-type": AP },
        body:
          method === "POST" ? JSON.stringify({ type: "Follow" }) : undefined,
      });
      expect(r.status).toBe(501);
      const body = await r.json();
      expect(body.error).toBe("not_implemented");
      expect(body.join).toBe(`${SITE}/c/harare-runners/join`);
    }
    expect(
      (await fetch(`${BASE}/inbox`, { method: "POST", body: "{}" })).status,
    ).toBe(501);
  });
});

describe("WebFinger and NodeInfo", () => {
  it("resolves acct: to the actor (RFC 7033)", async () => {
    const r = await get(
      "/.well-known/webfinger?resource=acct:harare-runners@circles.mukoko.com",
    );
    expect(r.status).toBe(200);
    expect(r.headers.get("content-type")).toBe("application/jrd+json");
    expect(r.headers.get("access-control-allow-origin")).toBe("*");
    const j = await r.json();
    expect(j.subject).toBe("acct:harare-runners@circles.mukoko.com");
    const self = j.links.find((l: { rel: string }) => l.rel === "self");
    expect(self).toEqual({
      rel: "self",
      type: AP,
      href: `${SITE}/c/harare-runners`,
    });
  });

  it("400 without a resource, 404 for other hosts and unknown circles", async () => {
    expect((await get("/.well-known/webfinger")).status).toBe(400);
    expect(
      (
        await get(
          "/.well-known/webfinger?resource=acct:harare-runners@mastodon.social",
        )
      ).status,
    ).toBe(404);
    expect(
      (
        await get(
          "/.well-known/webfinger?resource=acct:nobody@circles.mukoko.com",
        )
      ).status,
    ).toBe(404);
  });

  it("host-meta points at WebFinger", async () => {
    const xml = await (await get("/.well-known/host-meta")).text();
    expect(xml).toContain(
      `template="${SITE}/.well-known/webfinger?resource={uri}"`,
    );
  });

  it("NodeInfo 2.1", async () => {
    const links = await (await get("/.well-known/nodeinfo")).json();
    expect(links.links[0]).toEqual({
      rel: "http://nodeinfo.diaspora.software/ns/schema/2.1",
      href: `${SITE}/nodeinfo/2.1`,
    });
    const n = await (await get("/nodeinfo/2.1")).json();
    expect(n.version).toBe("2.1");
    expect(n.software.name).toMatch(/^[a-z0-9-]+$/);
    expect(n.protocols).toEqual(["activitypub"]);
    expect(n.metadata.circles).toBe(9);
  });
});

describe("Open Graph images", () => {
  it("renders a 1200×630 PNG per circle", async () => {
    const r = await get("/og/harare-runners.png");
    expect(r.status).toBe(200);
    expect(r.headers.get("content-type")).toBe("image/png");
    const b = new Uint8Array(await r.arrayBuffer());
    const view = new DataView(b.buffer);
    expect(view.getUint32(16)).toBe(1200);
    expect(view.getUint32(20)).toBe(630);
  });
});
