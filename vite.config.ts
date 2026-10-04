// Vite+ configuration: the org's standard check, lint and format settings.
// See nyuchi/.github/.github/workflows/reusable-vite-plus.yml.
import { defineConfig } from "vite-plus";

export default defineConfig({
  // `vp check` reads ONLY this block, not .oxfmtrc.json. These values mirror
  // nyuchi/.github/.oxfmtrc.json, which the org-required `vite-plus / fmt`
  // job uses on Markdown and JSON. Keep the two identical: two formatters
  // with different settings on one file can never both pass.
  fmt: {
    printWidth: 80,
    proseWrap: "preserve",
    tabWidth: 2,
    useTabs: false,
    endOfLine: "lf",
    trailingComma: "all",
    sortPackageJson: false,
    ignorePatterns: ["worker/**", "dist/**", ".astro/**"],
    overrides: [
      {
        files: ["*.md", "*.mdx"],
        options: { embeddedLanguageFormatting: "off" },
      },
    ],
  },
  lint: {
    // Without typeCheck, `vp check` is oxlint only and passes type errors.
    options: { typeAware: true, typeCheck: true },
    ignorePatterns: ["worker/**", "dist/**", ".astro/**"],
  },
  // The end-to-end tests run against `wrangler dev` with a stubbed API; see
  // tests/e2e.test.ts. The Rust unit tests run under `cargo test`.
  test: {
    include: ["tests/**/*.test.ts"],
    environment: "node",
    testTimeout: 30000,
    hookTimeout: 240000,
  },
});
