import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import './index.css'
import App from './App.tsx'

function applyInitialThemeClass() {
  try {
    const stored = localStorage.getItem('theme')
    const prefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches
    const effective =
      stored === 'dark' ? 'dark' : stored === 'light' ? 'light' : prefersDark ? 'dark' : 'light'

    document.documentElement.classList.toggle('dark', effective === 'dark')
    document.documentElement.style.colorScheme = effective
  } catch {
    // ignore
  }
}

applyInitialThemeClass()

async function setupMcpBridgeListeners() {
  // MCP bridge is only enabled in debug builds on Rust side.
  if (!import.meta.env.DEV) return

  const runtime = window as unknown as { __TAURI__?: unknown; __TAURI_INTERNALS__?: unknown }
  if (!runtime.__TAURI__ && !runtime.__TAURI_INTERNALS__) return

  try {
    const { setupPluginListeners, cleanupPluginListeners } = await import('tauri-plugin-mcp')
    await setupPluginListeners()

    window.addEventListener('beforeunload', () => {
      cleanupPluginListeners().catch(() => {})
    })
  } catch (err) {
    console.warn('MCP bridge listener setup failed:', err)
  }
}

void setupMcpBridgeListeners()

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
  </StrictMode>,
)
