import type { ReactNode } from "react";
import { Link } from "react-router-dom";
import { Moon, Sun, Plus, LayoutDashboard } from "lucide-react";
import { useTheme } from "../context/ThemeContext";
import { useAuth } from "../context/AuthContext";

interface Props {
  actions?: ReactNode;
}

export function Header({ actions }: Props) {
  const { theme, toggle } = useTheme();
  const { user } = useAuth();

  return (
    <header className="flex items-center justify-between px-4 py-3 border-b border-[#262626]">
      <Link to="/" className="flex items-center gap-2">
        <span className="w-5 h-5 rounded-full bg-[#3b82f6]" />
        <span className="font-semibold text-sm">copypaste</span>
      </Link>

      <div className="flex items-center gap-2">
        {actions}
        <Link
          to="/"
          className="p-1.5 text-[#a1a1a1] hover:text-[#fafafa] transition-colors"
          title="New paste"
        >
          <Plus size={18} />
        </Link>
        {user && (
          <Link
            to="/dashboard"
            className="p-1.5 text-[#a1a1a1] hover:text-[#fafafa] transition-colors"
          >
            <LayoutDashboard size={18} />
          </Link>
        )}
        <button
          onClick={toggle}
          className="p-1.5 text-[#a1a1a1] hover:text-[#fafafa] transition-colors"
        >
          {theme === "dark" ? <Sun size={18} /> : <Moon size={18} />}
        </button>
      </div>
    </header>
  );
}
