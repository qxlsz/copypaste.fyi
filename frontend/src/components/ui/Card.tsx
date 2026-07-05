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
        "rounded-lg border border-border bg-surface transition-colors",
        paddingMap[padding],
        interactive && "hover:border-accent/50",
        className,
      )}
      {...props}
    />
  ),
);

Card.displayName = "Card";
