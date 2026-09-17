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
  let referrerHost = "";
  try {
    referrerHost = document.referrer ? new URL(document.referrer).host : "";
  } catch {
    referrerHost = "";
  }
  const body = JSON.stringify({
    path: pageBucket(pathname),
    referrer: referrerHost,
    language: navigator.language || "",
  });
  const url = `${API_BASE}/collect`;
  void fetch(url, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body,
    keepalive: true,
    credentials: "omit",
    redirect: "error",
  }).catch(() => {
    /* visit counts are best-effort */
  });
};
