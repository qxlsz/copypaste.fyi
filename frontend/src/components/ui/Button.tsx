import { forwardRef } from "react";
import clsx from "clsx";

const baseClasses =
  "inline-flex items-center justify-center font-medium transition focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-offset-2 disabled:pointer-events-none disabled:opacity-60 rounded-lg";

const variantClasses: Record<NonNullable<ButtonProps["variant"]>, string> = {
  primary:
    "bg-primary text-primary-foreground shadow-soft hover:bg-primary/90 focus-visible:ring-primary/40",
  secondary:
    "bg-surface text-slate-900 shadow-soft dark:text-slate-100 hover:bg-surface/80 focus-visible:ring-primary/20",
  outline:
    "border border-muted text-slate-900 dark:text-slate-100 hover:border-primary/50 hover:text-primary focus-visible:ring-primary/30",
  ghost:
    "text-slate-700 dark:text-slate-300 hover:bg-muted/40 focus-visible:ring-primary/10",
  destructive:
    "bg-danger text-danger-foreground hover:bg-danger/90 focus-visible:ring-danger/40",
};

const sizeClasses: Record<NonNullable<ButtonProps["size"]>, string> = {
  sm: "h-8 px-3 text-xs rounded-md",
  md: "h-10 px-4 text-sm",
  lg: "h-12 px-6 text-base rounded-xl",
  icon: "h-10 w-10 p-0",
};

export interface ButtonProps
  extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: "primary" | "secondary" | "outline" | "ghost" | "destructive";
  size?: "sm" | "md" | "lg" | "icon";
  block?: boolean;
}

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(
  (
    { className, variant = "primary", size = "md", block = false, ...props },
    ref,
  ) => {
    return (
      <button
        ref={ref}
        className={clsx(
          baseClasses,
          variantClasses[variant],
          sizeClasses[size],
          block && "w-full",
          className,
        )}
        {...props}
      />
    );
  },
);

Button.displayName = "Button";
