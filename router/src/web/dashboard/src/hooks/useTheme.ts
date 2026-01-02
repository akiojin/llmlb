import { useState, useEffect, useCallback } from 'react'

type Theme = 'dark' | 'light'

const THEME_STORAGE_KEY = 'llm-router-theme'

/**
 * テーマ管理フック
 * - localStorage からテーマを読み込み
 * - システム設定 (prefers-color-scheme) のフォールバック
 * - 変更時に localStorage へ保存
 */
export function useTheme() {
  const [theme, setTheme] = useState<Theme>(() => {
    // SSR対応: windowが存在しない場合はデフォルト値を返す
    if (typeof window === 'undefined') return 'dark'

    // localStorage から保存済み設定を取得
    const saved = localStorage.getItem(THEME_STORAGE_KEY)
    if (saved === 'light' || saved === 'dark') {
      return saved
    }

    // システム設定をフォールバック
    return window.matchMedia('(prefers-color-scheme: dark)').matches
      ? 'dark'
      : 'light'
  })

  // テーマ変更時にDOMとlocalStorageを更新
  useEffect(() => {
    document.documentElement.classList.toggle('dark', theme === 'dark')
    localStorage.setItem(THEME_STORAGE_KEY, theme)
  }, [theme])

  // システム設定変更時の監視
  useEffect(() => {
    const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)')

    const handleChange = (e: MediaQueryListEvent) => {
      // localStorage に明示的な設定がない場合のみ追従
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
