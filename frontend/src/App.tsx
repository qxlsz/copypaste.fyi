import { BrowserRouter, Route, Routes } from "react-router-dom";
import { Toaster } from "sonner";
import { ThemeProvider } from "./context/ThemeContext";
import { AuthProvider } from "./context/AuthContext";
import { Composer } from "./pages/Composer";
import { Viewer } from "./pages/Viewer";
import { Dashboard } from "./pages/Dashboard";
import { Login } from "./pages/Login";

export default function App() {
  return (
    <BrowserRouter>
      <ThemeProvider>
        <AuthProvider>
          <Routes>
            <Route path="/" element={<Composer />} />
            <Route path="/p/:id" element={<Viewer />} />
            <Route path="/dashboard" element={<Dashboard />} />
            <Route path="/login" element={<Login />} />
          </Routes>
          <Toaster position="bottom-right" richColors />
        </AuthProvider>
      </ThemeProvider>
    </BrowserRouter>
  );
}
