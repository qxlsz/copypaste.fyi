import { forwardRef } from "react";
import clsx from "clsx";

type CardElement = HTMLDivElement;

type BaseProps = React.HTMLAttributes<CardElement>;

export interface CardProps extends BaseProps {
  padding?: "none" | "sm" | "md" | "lg";
  interactive?: boolean;
}

const paddingMap: Record<NonNullable<CardProps["padding"]>, string> = {
  none: "p-0",
  sm: "p-3",
  md: "p-6",
  lg: "p-8",
};

export const Card = forwardRef<CardElement, CardProps>(
  ({ className, padding = "md", interactive = false, ...props }, ref) => (
    <div
      ref={ref}
      className={clsx(
        "rounded-2xl border border-muted/60 bg-surface/90 shadow-soft backdrop-blur-sm transition-colors dark:border-muted/40 dark:bg-surface/80",
        paddingMap[padding],
        interactive && "hover:border-primary/50 hover:shadow-strong",
        className,
      )}
      {...props}
    />
  ),
);

Card.displayName = "Card";
