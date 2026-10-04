// @ts-check
import { defineConfig } from "astro/config";
import tailwindcss from "@tailwindcss/vite";

/**
 * circles.mukoko.com, the Astro half.
 *
 * Static output, no islands and no client-side framework: the browser gets
 * HTML and CSS, and no JavaScript at all (the CSP says `script-src 'none'`).
 *
 * Two kinds of page come out of this build:
 *
 * - Real pages (`/about`, `/fediverse`, `/404`), served as they are.
 * - Shells under `/tpl/`, with `{{placeholders}}` in them. The Rust Worker
 *   (`worker/`) fills them with live data from the Nyuchi API for the home
 *   page, the lists and every circle page, and never serves `/tpl/` itself.
 *   The design lives here; the data and the logic live in Rust.
 *
 * `format: "file"` emits `/about.html`, which the asset server resolves from
 * `/about` without a redirect, and gives the Worker stable shell paths.
 * Stylesheets are never inlined, so the CSP can stay `style-src 'self'`.
 */
export default defineConfig({
  output: "static",
  site: "https://circles.mukoko.com",
  build: {
    format: "file",
    inlineStylesheets: "never",
  },
  vite: { plugins: [tailwindcss()] },
  devToolbar: { enabled: false },
});
