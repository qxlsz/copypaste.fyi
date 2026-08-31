import { Link } from "react-router-dom";

export const NotFoundPage = () => (
  <section className="flex min-h-[50vh] flex-col items-center justify-center px-6">
    <div className="w-full max-w-sm space-y-4">
      <h1 className="text-2xl font-medium tracking-tight text-text">
        Page not found
      </h1>
      <p className="text-sm leading-relaxed text-muted-foreground">
        The link may have expired, burned after reading, or never existed.
      </p>
      <Link
        to="/"
        className="inline-flex h-12 items-center rounded-md bg-accent px-4 text-sm font-medium text-accent-foreground sm:h-11"
      >
        New paste
      </Link>
    </div>
  </section>
);
