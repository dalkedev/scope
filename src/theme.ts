import { useCallback, useLayoutEffect, useState } from 'react';

export type ThemePreference = 'light' | 'dark' | 'system';
export const THEME_STORAGE_KEY = 'scope-theme';
const DARK_MEDIA_QUERY = '(prefers-color-scheme: dark)';

export function readThemePreference(): ThemePreference {
  try {
    const saved = window.localStorage.getItem(THEME_STORAGE_KEY);
    if (saved === 'light' || saved === 'dark') return saved;
  } catch {
    // Appearance still works when the WebView denies persistent storage.
  }
  return 'system';
}

function systemThemeQuery(): MediaQueryList | null {
  try {
    return window.matchMedia(DARK_MEDIA_QUERY);
  } catch {
    return null;
  }
}

export function useThemePreference() {
  const [preference, setPreference] = useState<ThemePreference>(readThemePreference);

  useLayoutEffect(() => {
    const query = systemThemeQuery();
    const apply = () => {
      const theme = preference === 'system' ? (query?.matches ? 'dark' : 'light') : preference;
      document.documentElement.dataset.theme = theme;
      document.documentElement.style.colorScheme = theme;
      // The preload supplies a canvas before the stylesheet has loaded.
      document.documentElement.style.removeProperty('background-color');
    };
    apply();
    if (preference !== 'system' || !query) return;
    if (typeof query.addEventListener === 'function') {
      query.addEventListener('change', apply);
      return () => query.removeEventListener('change', apply);
    }
    query.addListener(apply);
    return () => query.removeListener(apply);
  }, [preference]);

  const choosePreference = useCallback((next: ThemePreference) => {
    try {
      window.localStorage.setItem(THEME_STORAGE_KEY, next);
    } catch {
      // Keep the user's choice for this running session if saving is unavailable.
    }
    setPreference(next);
  }, []);

  return [preference, choosePreference] as const;
}
