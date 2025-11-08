import { Link, useLocation } from "react-router-dom";

export const FloatingAboutButton = () => {
  const location = useLocation();

  // Don't show on the about page itself
  if (location.pathname === "/about") {
    return null;
  }

  return (
    <Link
      to="/about"
      className="fixed bottom-6 right-6 z-50 group"
      aria-label="About"
    >
      <div className="relative">
        {/* Tooltip */}
        <div className="absolute bottom-full right-0 mb-2 hidden group-hover:block">
          <div className="bg-gray-900 dark:bg-gray-700 text-white text-sm px-2 py-1 rounded whitespace-nowrap">
            Learn more about copypaste.fyi
          </div>
        </div>

        {/* Button */}
        <div className="flex items-center justify-center w-14 h-14 bg-gradient-to-br from-indigo-500 to-purple-600 rounded-full shadow-lg hover:shadow-xl transition-all duration-300 hover:scale-110">
          <svg
            className="w-6 h-6 text-white"
            fill="none"
            stroke="currentColor"
            viewBox="0 0 24 24"
            xmlns="http://www.w3.org/2000/svg"
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth={2}
              d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"
            />
          </svg>
        </div>

        {/* Pulse animation */}
        <div className="absolute inset-0 rounded-full bg-gradient-to-br from-indigo-500 to-purple-600 animate-ping opacity-20"></div>
      </div>
    </Link>
  );
};
