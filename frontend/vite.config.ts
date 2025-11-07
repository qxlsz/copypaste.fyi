import { defineConfig, loadEnv, type UserConfig } from 'vite'
import react from '@vitejs/plugin-react'
import mkcert from 'vite-plugin-mkcert'

// https://vite.dev/config/
export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, process.cwd(), '')

  if (mode === 'production' && !env.VITE_API_BASE) {
    throw new Error(
      'Missing VITE_API_BASE environment variable for production builds. Configure it in your Vercel project settings.'
    )
  }

  const config: UserConfig = {
    plugins: [react()],
  }

  if (mode === 'development') {
    config.plugins?.push(mkcert())
    config.server = {
      host: '127.0.0.1',
      port: 5173,
      proxy: {
        '/api': {
          target: 'http://127.0.0.1:8000',
          changeOrigin: true,
        },
      },
    }
  }

  return config
})
