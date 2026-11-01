import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';

export default defineConfig({
  plugins: [sveltekit()],
  server: {
    port: 5173,
    strictPort: true
  },
  // Tauri host env when running inside the desktop shell
  clearScreen: false,
  envPrefix: ['VITE_', 'TAURI_']
});
