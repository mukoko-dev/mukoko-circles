# AGENTS.md — mukoko-circles

The org rules (nyuchi/.github `AGENTS.md`) apply; this file adds to them.

## The pages are the Mzizi Discover Standard

circles.mukoko.com is built from `@bundu/ui/discover/*` (the Mzizi Discover
Standard, [docs.mzizi.dev/patterns/discover-standard](https://docs.mzizi.dev/patterns/discover-standard),
mzizi-dev/mzizi-registry#413): `DiscoverShell` and `DiscoverMeta` in
`src/layouts/Shell.astro`, and `DiscoverHero`, `DiscoverSearch`,
`DiscoverSection`, `ResultGrid`, `DiscoverCard`, `CategoryChip(s)`, `LoadMore`
and `OpenInApp` in the `/tpl/` shells. News, events, weather and the
super-app on the web use the same components, so a page here moves there
unchanged.

## Upstream first (hard rule)

Owner, 2026-10-04: "anything new that is not in Mzizi, or altered from the
Mzizi ones, we need to adjust Mzizi so the design is always updating so we
maintain consistency."

- A component this site needs that Mzizi does not have, or a change to one it
  has, goes upstream immediately: the contract in mzizi-dev/mzizi-registry
  (`contracts/`), the build in mzizi-dev/packages-npm (`@bundu/ui`).
- A local copy is allowed only while that upstream PR is open, marked at the
  top of the file with `TODO(mzizi): <upstream PR URL>`, and is deleted when
  the release is installed. `src/styles/global.css` defines no components.
- The one local component is `src/components/Mark.astro`, the Circles logo:
  brand content for `DiscoverShell`'s `brand` slot, not a UI component.

## No inline styles

The CSP is `style-src 'self'` with no `style-src-attr`, and `script-src 'none'`.
Nothing here or in `@bundu/ui` writes a `style` attribute; the e2e tests check
every page.
