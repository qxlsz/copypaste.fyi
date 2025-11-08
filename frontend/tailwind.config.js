import forms from "@tailwindcss/forms";
import typography from "@tailwindcss/typography";

/** @type {import('tailwindcss').Config} */
export default {
  darkMode: ["class", '[data-theme="dark"]'],
  content: ["./index.html", "./src/**/*.{js,ts,jsx,tsx}"],
  theme: {
    extend: {
      colors: {
        border: "var(--color-muted)",
        input: "var(--color-muted)",
        ring: "var(--color-primary)",
        background: "var(--color-background)",
        surface: "var(--color-surface)",
        primary: {
          DEFAULT: "var(--color-primary)",
          foreground: "#f8fafc",
        },
        accent: {
          DEFAULT: "var(--color-accent)",
          foreground: "#f8fafc",
        },
        muted: {
          DEFAULT: "var(--color-muted)",
          foreground: "var(--color-muted-foreground)",
        },
        success: {
          DEFAULT: "var(--color-success)",
          foreground: "#064e3b",
        },
        warning: {
          DEFAULT: "var(--color-warning)",
          foreground: "#78350f",
        },
        danger: {
          DEFAULT: "var(--color-danger)",
          foreground: "#7f1d1d",
        },
        info: {
          DEFAULT: "var(--color-info)",
          foreground: "#0f172a",
        },
      },
      fontFamily: {
        sans: ['"Inter"', "system-ui", "sans-serif"],
        mono: ['"JetBrains Mono"', "monospace"],
      },
      boxShadow: {
        soft: "var(--shadow-soft)",
        strong: "var(--shadow-strong)",
      },
      borderRadius: {
        xl: "1rem",
        "2xl": "1.5rem",
      },
      keyframes: {
        "fade-in": {
          "0%": { opacity: 0 },
          "100%": { opacity: 1 },
        },
        "slide-up": {
          "0%": { transform: "translateY(12px)", opacity: 0 },
          "100%": { transform: "translateY(0)", opacity: 1 },
        },
      },
      animation: {
        "fade-in": "fade-in 200ms ease-out",
        "slide-up": "slide-up 200ms ease-out",
      },
    },
  },
  plugins: [forms, typography],
};
