import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { Suspense, lazy, useState } from "react";
import { BrowserRouter, Route, Routes } from "react-router-dom";
import { Toaster } from "sonner";

import { ErrorBoundary } from "./components/ErrorBoundary";
import { Layout } from "./components/Layout";
import { PrivacyJourney } from "./components/PrivacyJourney";
import { PasteFormPage } from "./pages/PasteForm";

import { ThemeProvider } from "./theme/ThemeProvider";
import { useTheme } from "./theme/ThemeContext";

// Route-level code splitting: heavy dependencies (Mermaid in About, d3 in
// Stats) stay out of the initial bundle. The composer stays eager so the
// primary flow paints immediately.
const PasteViewPage = lazy(() =>
  import("./pages/PasteView").then((m) => ({ default: m.PasteViewPage })),
);
const StatsPage = lazy(() =>
  import("./pages/Stats").then((m) => ({ default: m.StatsPage })),
);
const DashboardPage = lazy(() =>
  import("./pages/Dashboard").then((m) => ({ default: m.DashboardPage })),
);
const LoginPage = lazy(() =>
  import("./pages/Login").then((m) => ({ default: m.LoginPage })),
);
const AboutPage = lazy(() =>
  import("./pages/About").then((m) => ({ default: m.AboutPage })),
);
const NotFoundPage = lazy(() =>
  import("./pages/NotFound").then((m) => ({ default: m.NotFoundPage })),
);

const RouteFallback = () => (
  <div
    className="flex min-h-[40vh] items-center justify-center"
    role="status"
    aria-label="Loading page"
  >
    <span className="h-5 w-5 animate-spin rounded-full border-2 border-border border-t-accent" />
  </div>
);

export function App() {
  return (
    <>
      <Suspense fallback={<RouteFallback />}>
        <Routes>
          <Route path="/login" element={<LoginPage />} />
          <Route path="/" element={<Layout />}>
            <Route index element={<PasteFormPage />} />
            <Route
              path="p/:id"
              element={
                <ErrorBoundary>
                  <PasteViewPage />
                </ErrorBoundary>
              }
            />
            <Route path="dashboard" element={<DashboardPage />} />
            <Route path="stats" element={<StatsPage />} />
            <Route path="about" element={<AboutPage />} />
            <Route path="*" element={<NotFoundPage />} />
          </Route>
        </Routes>
      </Suspense>
      <PrivacyJourney />
    </>
  );
}

const ThemedToaster = () => {
  const { theme } = useTheme();
  return (
    <Toaster richColors closeButton position="bottom-right" theme={theme} />
  );
};

export default function AppWithProviders() {
  const [queryClient] = useState(() => new QueryClient());

  return (
    <BrowserRouter>
      <QueryClientProvider client={queryClient}>
        <ThemeProvider>
          <App />
          <ThemedToaster />
        </ThemeProvider>
      </QueryClientProvider>
    </BrowserRouter>
  );
}
