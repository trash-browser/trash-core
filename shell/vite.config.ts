import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import { viteSingleFile } from 'vite-plugin-singlefile'
// https://vite.dev/config/
export default defineConfig({
  // The Rust host embeds dist/index.html and serves it from miko://shell/.
  base: './',
  plugins: [react(), tailwindcss(), viteSingleFile()],
})
