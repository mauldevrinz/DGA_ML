import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

// https://vite.dev/config/
export default defineConfig({
  plugins: [react()],
  server: {
    host: '0.0.0.0',
    port: 5173,
    watch: {
      ignored: ['**/TEENSYCODE/teensy-rust/target/**', '**/target/**', '**/node_modules/**', '**/dist/**'],
    },
  },
})
