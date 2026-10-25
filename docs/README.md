# Plotium user manual

The Plotium user manual, built with [Starlight](https://starlight.astro.build).
English is the default language (site root); Simplified Chinese lives under
`/zh-cn/`. Pages are matched by filename across languages — an untranslated
page automatically falls back to the English version with a notice banner.

## Layout

- `src/content/docs/` — English pages (default locale)
- `src/content/docs/zh-cn/` — Chinese translations (same filenames)
- `astro.config.mjs` — site title, locales, and sidebar structure

## Writing conventions

- Each page is one of three kinds: a task guide (how to do something), a
  concept page (why it works that way), or a reference (a complete list or
  contract). Keep each section to one kind.
- Property IDs, JSON, and error codes belong only in the Setting IDs and MCP
  pages. A guide links there in at most one line.
- Write a new feature into the page and section that own its task, not at the
  end of the page edited most recently.
- State each fact in one place and link to it from elsewhere.
- A task page that an AI client can also perform ends with a "With an AI
  client" section in the format used by the existing pages.
- When adding or moving a page, update its Simplified Chinese counterpart and
  the sidebar in `astro.config.mjs` in the same change.

## Commands

Run from this directory:

| Command | Action |
| --- | --- |
| `npm install` | Install dependencies |
| `npm run dev` | Local dev server at `localhost:4321` |
| `npm run build` | Build the static site into `dist/` |
| `npm run preview` | Preview the production build locally |

## Deployment (Cloudflare Workers)

Deployed as a Cloudflare Worker serving static assets (`wrangler.jsonc`);
the site URL is <https://plotium.nmrtist.space>. The Workers Builds Git
integration builds on push with:

- **Path (root directory):** `docs`
- **Build command:** `npm run build`
- **Deploy command:** `npx wrangler deploy`
