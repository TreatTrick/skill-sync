import { sveltekit } from '@sveltejs/kit/vite'
import tailwindcss from '@tailwindcss/vite'
import { fileURLToPath, URL } from 'node:url'
import { defineConfig } from 'vite'

// https://vite.dev/config/
export default defineConfig({
  plugins: [sveltekit(), tailwindcss()],
  // Tauri dev: keep the dev server on a fixed port so tauri.conf.json devUrl matches.
  server: {
    port: 5173,
    strictPort: true,
  },
  // Tauri expects a clear console; HMR noise is suppressed.
  clearScreen: false,
  envPrefix: ['VITE_', 'TAURI_'],
  resolve: {
    alias: {
      '@': fileURLToPath(new URL('./src', import.meta.url)),
    },
  },
  build: {
    target: 'esnext',
  },
})
