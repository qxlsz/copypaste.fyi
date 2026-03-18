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
      <Link to="/" className="flex items-center gap-2">
        <span className="w-4 h-4 rounded-full bg-[#3b82f6] flex-shrink-0" />
        <span className="font-semibold text-sm tracking-tight">copypaste</span>
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
