import adapter from '@sveltejs/adapter-static'
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte'

// GitHub project Pages base path. Override with PAGES_BASE=/ for a custom domain.
const base = process.env.PAGES_BASE ?? '/skill-sync'

const config = {
  preprocess: vitePreprocess(),
  kit: {
    adapter: adapter({
      // SPA mode: the locale is chosen client-side, so render on the client
      // only and serve a single fallback index.html.
      fallback: 'index.html',
    }),
    paths: { base },
    files: {
      assets: 'static',
    },
  },
}

export default config
