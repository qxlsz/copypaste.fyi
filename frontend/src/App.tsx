import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { useState } from "react";
import { BrowserRouter, Route, Routes } from "react-router-dom";

import { Layout } from "./components/Layout";
import { PrivacyJourney } from "./components/PrivacyJourney";
import { PasteFormPage } from "./pages/PasteForm";
import { PasteViewPage } from "./pages/PasteView";
import { StatsPage } from "./pages/Stats";
import { DashboardPage } from "./pages/Dashboard";
import { LoginPage } from "./pages/Login";
import { AboutPage } from "./pages/About";

import { ThemeProvider } from "./theme/ThemeProvider";

export function App() {
  return (
    <>
      <Routes>
        <Route path="/login" element={<LoginPage />} />
        <Route path="/" element={<Layout />}>
          <Route index element={<PasteFormPage />} />
          <Route path="p/:id" element={<PasteViewPage />} />
          <Route path="dashboard" element={<DashboardPage />} />
          <Route path="stats" element={<StatsPage />} />
          <Route path="about" element={<AboutPage />} />
        </Route>
      </Routes>
      <PrivacyJourney />
    </>
  );
}

export default function AppWithProviders() {
  const [queryClient] = useState(() => new QueryClient());

  return (
    <BrowserRouter>
      <QueryClientProvider client={queryClient}>
        <ThemeProvider>
          <App />
        </ThemeProvider>
      </QueryClientProvider>
    </BrowserRouter>
  );
}
