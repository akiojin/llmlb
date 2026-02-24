import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import path from 'path'

// https://vite.dev/config/
export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },
  build: {
    outDir: '../static',
    emptyOutDir: true,
    rollupOptions: {
      input: {
        main: path.resolve(__dirname, 'index.html'),
        login: path.resolve(__dirname, 'login.html'),
        register: path.resolve(__dirname, 'register.html'),
        'change-password': path.resolve(__dirname, 'change-password.html'),
      },
      output: {
        // Ensure consistent file names for Rust include_dir!
        entryFileNames: 'assets/[name]-[hash].js',
        chunkFileNames: 'assets/[name]-[hash].js',
        assetFileNames: 'assets/[name]-[hash].[ext]',
      },
    },
  },
  base: '/dashboard/',
  server: {
    proxy: {
      '/api': {
        target: 'http://localhost:32768',
        changeOrigin: true,
      },
      '/v1': {
        target: 'http://localhost:32768',
        changeOrigin: true,
      },
    },
  },
})
