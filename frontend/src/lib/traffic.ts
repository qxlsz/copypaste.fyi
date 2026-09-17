import { API_BASE } from "../api/client";

export const pageBucket = (pathname: string): string => {
  if (pathname === "/") return "/";
  if (pathname === "/about" || pathname === "/stats" || pathname === "/login") {
    return pathname;
  }
  if (pathname.startsWith("/p/") || pathname.startsWith("/raw/")) return "/p/";
  return "/other";
};

export const collectVisit = (pathname: string): void => {
  const body = JSON.stringify({
    path: pageBucket(pathname),
    referrer: document.referrer || "",
    device: navigator.userAgent,
    language: navigator.language || "",
  });
  const url = `${API_BASE}/collect`;
  void fetch(url, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body,
    keepalive: true,
    credentials: "omit",
  }).catch(() => {
    /* visit counts are best-effort */
  });
};
