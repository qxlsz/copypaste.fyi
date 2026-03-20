import { Link } from "react-router-dom";
import { Moon, Sun, Plus, LayoutDashboard, LogOut, LogIn } from "lucide-react";
import { useTheme } from "../context/ThemeContext";
import { useAuth } from "../context/AuthContext";
import { useNavigate } from "react-router-dom";

interface Props {
  actions?: React.ReactNode;
}

export function Header({ actions }: Props) {
  const { theme, toggleTheme } = useTheme();
  const { user, logout } = useAuth();
  const navigate = useNavigate();

  return (
    <header className="flex items-center justify-between px-4 py-3 border-b border-[#262626] bg-[#0a0a0a]">
      <Link to="/" className="flex items-center gap-2 group">
        <span className="w-3.5 h-3.5 rounded-full bg-gradient-to-br from-[#3b82f6] to-[#6366f1] flex-shrink-0 shadow-[0_0_8px_rgba(59,130,246,0.5)] group-hover:shadow-[0_0_12px_rgba(59,130,246,0.7)] transition-shadow" />
        <span className="font-semibold text-sm tracking-tight">
          <span className="bg-gradient-to-r from-white to-white/70 bg-clip-text text-transparent">copypaste</span><span className="text-[#3b82f6]/60 font-normal">.fyi</span>
        </span>
      </Link>

      <div className="flex items-center gap-1">
        {actions}
        <Link
          to="/"
          className="p-1.5 text-[#a1a1a1] hover:text-[#fafafa] transition-colors rounded"
          title="New paste"
        >
          <Plus size={16} />
        </Link>
        {user ? (
          <>
            <Link
              to="/dashboard"
              className="p-1.5 text-[#a1a1a1] hover:text-[#fafafa] transition-colors rounded"
              title="Dashboard"
            >
              <LayoutDashboard size={16} />
            </Link>
            <button
              onClick={() => {
                logout();
                navigate("/");
              }}
              className="p-1.5 text-[#a1a1a1] hover:text-[#fafafa] transition-colors rounded"
              title="Logout"
            >
              <LogOut size={16} />
            </button>
          </>
        ) : (
          <Link
            to="/login"
            className="p-1.5 text-[#a1a1a1] hover:text-[#fafafa] transition-colors rounded"
            title="Login"
          >
            <LogIn size={16} />
          </Link>
        )}
        <button
          onClick={toggleTheme}
          className="p-1.5 text-[#a1a1a1] hover:text-[#fafafa] transition-colors rounded"
          title="Toggle theme"
        >
          {theme === "dark" ? <Sun size={16} /> : <Moon size={16} />}
        </button>
      </div>
    </header>
  );
}
