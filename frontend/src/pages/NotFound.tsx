import { Link } from "react-router-dom";

export const NotFoundPage = () => (
  <div className="mx-auto max-w-md space-y-4 py-16 text-center">
    <p className="font-mono text-5xl font-semibold text-muted-foreground">
      404
    </p>
    <h1 className="text-xl font-semibold tracking-tight text-text">
      Paste not found or page does not exist
    </h1>
    <p className="text-sm text-muted-foreground">
      The link may have expired, been burned after reading, or never existed.
    </p>
    <Link
      to="/"
      className="inline-flex items-center justify-center rounded-md bg-accent px-4 py-2 text-sm font-medium text-accent-foreground transition hover:bg-accent/90 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent focus-visible:ring-offset-2 focus-visible:ring-offset-background"
    >
      Back to home
    </Link>
  </div>
);
