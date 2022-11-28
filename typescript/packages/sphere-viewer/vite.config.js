import wasm from 'vite-plugin-wasm';
import { defineConfig } from 'vite';

export default defineConfig({
  plugins: [
    wasm()
  ],
  build: {
    target: 'esnext'
  },
  base: './'
});