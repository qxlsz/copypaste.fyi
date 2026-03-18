export { ThemeProvider } from "../theme/ThemeProvider";

import { useTheme as useThemeBase } from "../theme/ThemeContext";

// eslint-disable-next-line react-refresh/only-export-components
export function useTheme() {
  const { theme, toggleTheme } = useThemeBase();
  return { theme, toggle: toggleTheme };
}
