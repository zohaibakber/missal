import type { DesktopTheme } from "#/desktop-window";
import { createContext, use, useState } from "react";

type ThemeProviderState = {
  theme: DesktopTheme;
  setTheme: (theme: DesktopTheme) => void;
};

const ThemeProviderContext = createContext<ThemeProviderState | null>(null);

export function ThemeProvider({ children }: { children: React.ReactNode }) {
  const [theme, setThemeState] = useState(window.electronPreferences.theme);

  const setTheme = (next: DesktopTheme) => {
    window.electronPreferences.setTheme(next);
    setThemeState(next);
  };

  return <ThemeProviderContext value={{ theme, setTheme }}>{children}</ThemeProviderContext>;
}

export function useTheme() {
  const context = use(ThemeProviderContext);
  if (!context) throw new Error("useTheme must be used within a ThemeProvider");
  return context;
}
