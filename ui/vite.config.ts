/// <reference types="vitest/config" />
import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import { readFileSync } from 'node:fs'
import { createRequire } from 'node:module'
import { dirname, join } from 'node:path'
import type { Plugin } from 'vite'

// MapLibre's worker imports a sibling module by relative name, so both files must keep their names.
function maplibreWorker(): Plugin {
  const dir = () => dirname(createRequire(import.meta.url).resolve('maplibre-gl/package.json'))
  const files = ['maplibre-gl-worker.mjs', 'maplibre-gl-shared.mjs']
  return {
    name: 'kintree-maplibre-worker',
    buildStart() {
      for (const f of files)
        this.emitFile({ type: 'asset', fileName: `maplibre/${f}`, source: readFileSync(join(dir(), 'dist', f)) })
    },
    configureServer(server) {
      server.middlewares.use((req, res, next) => {
        const f = files.find((x) => req.url?.split('?')[0] === `/maplibre/${x}`)
        if (!f) return next()
        res.setHeader('Content-Type', 'text/javascript')
        res.end(readFileSync(join(dir(), 'dist', f)))
      })
    },
  }
}

export default defineConfig({
  plugins: [react(), tailwindcss(), maplibreWorker()],
  server: { port: 5173, proxy: { '/api': 'http://127.0.0.1:8787' } },
  build: { chunkSizeWarningLimit: 900 },
  test: {
    environment: 'jsdom',
    globals: true,
    setupFiles: ['./src/test-setup.ts'],
    exclude: ['e2e/**', 'node_modules/**'],
  },
})
