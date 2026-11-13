import { useCallback, useEffect, useState } from 'react';

export type Theme = 'dark' | 'light';

const KEY = 'timetrek.theme';

/** Uložená volba (výchozí je tmavý režim). Přístup k úložišti může selhat, proto try/catch. */
export function loadTheme(): Theme {
  try {
    return localStorage.getItem(KEY) === 'light' ? 'light' : 'dark';
  } catch {
    return 'dark';
  }
}

export function applyTheme(theme: Theme) {
  document.documentElement.dataset.theme = theme;
}

export function useTheme() {
  const [theme, setTheme] = useState<Theme>(loadTheme);

  useEffect(() => {
    applyTheme(theme);
    try {
      localStorage.setItem(KEY, theme);
    } catch {
      /* bez uložení: režim platí do zavření */
    }
  }, [theme]);

  const toggle = useCallback(() => setTheme((t) => (t === 'dark' ? 'light' : 'dark')), []);
  return { theme, toggle };
}
