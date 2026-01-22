import { useState, useEffect, useCallback } from 'react'

type Theme = 'dark' | 'light'

const THEME_STORAGE_KEY = 'llmlb-theme'

/**
 * Theme management hook
 * - Load theme from localStorage
 * - Fallback to system preference (prefers-color-scheme)
 * - Save to localStorage on change
 */
export function useTheme() {
  const [theme, setTheme] = useState<Theme>(() => {
    // SSR support: return default if window is undefined
    if (typeof window === 'undefined') return 'dark'

    // Get saved setting from localStorage
    const saved = localStorage.getItem(THEME_STORAGE_KEY)
    if (saved === 'light' || saved === 'dark') {
      return saved
    }

    // Fallback to system preference
    return window.matchMedia('(prefers-color-scheme: dark)').matches
      ? 'dark'
      : 'light'
  })

  // Update DOM and localStorage when theme changes
  useEffect(() => {
    document.documentElement.classList.toggle('dark', theme === 'dark')
    localStorage.setItem(THEME_STORAGE_KEY, theme)
  }, [theme])

  // Watch for system preference changes
  useEffect(() => {
    const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)')

    const handleChange = (e: MediaQueryListEvent) => {
      // Only follow system preference if no explicit setting in localStorage
      const saved = localStorage.getItem(THEME_STORAGE_KEY)
      if (!saved) {
        setTheme(e.matches ? 'dark' : 'light')
      }
    }

    mediaQuery.addEventListener('change', handleChange)
    return () => mediaQuery.removeEventListener('change', handleChange)
  }, [])

  const toggleTheme = useCallback(() => {
    setTheme((prev) => (prev === 'dark' ? 'light' : 'dark'))
  }, [])

  return { theme, setTheme, toggleTheme }
}
