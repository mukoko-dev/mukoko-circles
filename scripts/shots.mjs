// Screenshots at 375 and 1280 px, light and dark, plus an axe-core
// accessibility pass on every page. Needs the Worker running against the stub
// API (npm run dev, with node scripts/stub-api.mjs alongside).
//
//   node scripts/shots.mjs [base]   (default http://127.0.0.1:8787)
// Writes shots/*.png and fails if axe finds a serious or critical problem.
import { chromium } from "playwright";
import { mkdirSync, readFileSync } from "node:fs";
import { createRequire } from "node:module";

const base = process.argv[2] ?? "http://127.0.0.1:8787";
const axeSource = readFileSync(
  createRequire(import.meta.url).resolve("axe-core/axe.min.js"),
  "utf8",
);
const pages = {
  home: "/",
  circle: "/c/harare-runners",
  broadcast: "/c/mukoko-news-desk",
  all: "/circles",
  category: "/categories/music",
  search_empty: "/search?q=zzzz",
  not_found: "/c/no-such-circle",
  about: "/about",
  fediverse: "/fediverse",
};
mkdirSync("shots", { recursive: true });
const browser = await chromium.launch();
let failures = 0;
for (const scheme of ["light", "dark"]) {
  for (const width of [375, 1280]) {
    const ctx = await browser.newContext({
      viewport: { width, height: 900 },
      colorScheme: scheme,
      deviceScaleFactor: 1,
    });
    const page = await ctx.newPage();
    for (const [name, path] of Object.entries(pages)) {
      await page.goto(base + path, { waitUntil: "networkidle" });
      await page.screenshot({
        path: `shots/${name}-${width}-${scheme}.png`,
        fullPage: true,
      });
      // axe runs in the page via evaluate, which the CSP's script-src 'none'
      // does not govern (Playwright evaluates through the DevTools protocol).
      await page.evaluate(axeSource);
      const result = await page.evaluate(async () => {
        // @ts-expect-error axe is injected above
        const r = await window.axe.run(document, {
          resultTypes: ["violations"],
        });
        return r.violations.map((v) => ({
          id: v.id,
          impact: v.impact,
          nodes: v.nodes.length,
          target: v.nodes[0]?.target,
        }));
      });
      const bad = result.filter(
        (v) => v.impact === "serious" || v.impact === "critical",
      );
      if (bad.length) {
        failures += bad.length;
        console.log(`${name} ${width} ${scheme}:`, JSON.stringify(bad));
      }
      const overflow = await page.evaluate(
        () => document.documentElement.scrollWidth > window.innerWidth,
      );
      if (overflow) {
        failures++;
        console.log(`${name} ${width} ${scheme}: horizontal overflow`);
      }
    }
    await ctx.close();
  }
}
await browser.close();
console.log(
  failures
    ? `${failures} problem(s)`
    : "no serious accessibility problems, no overflow",
);
process.exit(failures ? 1 : 0);
