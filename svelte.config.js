import adapter from '@sveltejs/adapter-static'
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte'

// SvelteKit SPA for Tauri: no SSR, static output to ../dist so
// src-tauri/tauri.conf.json frontendDist keeps pointing at the same place.
const config = {
  preprocess: vitePreprocess(),
  kit: {
    adapter: adapter({
      pages: 'dist',
      assets: 'dist',
      fallback: 'index.html',
    }),
    alias: {
      '@': 'src',
    },
    files: {
      assets: 'public',
    },
  },
}

export default config
