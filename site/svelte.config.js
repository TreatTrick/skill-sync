import adapter from '@sveltejs/adapter-static'
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte'

// GitHub project Pages base path. Override with PAGES_BASE=/ for a custom domain.
const base = process.env.PAGES_BASE ?? '/skill-sync'

const config = {
  preprocess: vitePreprocess(),
  kit: {
    adapter: adapter({
      // Prerender the single route so search engines receive the full title,
      // H1, and body in the initial HTML. No SPA fallback is needed.
    }),
    paths: { base },
    files: {
      assets: 'static',
    },
  },
}

export default config
