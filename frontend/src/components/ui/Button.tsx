import type { ButtonHTMLAttributes } from "react";

import { Spinner } from "./Spinner";

type Variant = "primary" | "secondary" | "danger" | "ghost";

const VARIANTS: Record<Variant, string> = {
  primary: "bg-accent text-accent-fg hover:brightness-110 active:brightness-95",
  secondary:
    "border border-border bg-surface text-fg hover:bg-surface-muted active:brightness-95",
  danger: "bg-danger text-white hover:brightness-110 active:brightness-95 dark:text-bg",
  ghost: "text-muted hover:bg-surface-muted hover:text-fg",
};

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: Variant;
  /** Shows a spinner and disables the button while a request is in flight. */
  loading?: boolean;
  size?: "sm" | "md";
}

export function buttonClasses(variant: Variant = "primary", size: "sm" | "md" = "md") {
  const sizing = size === "sm" ? "h-8 px-3 text-sm" : "h-10 px-4 text-sm";
  return `inline-flex items-center justify-center gap-2 rounded-md font-medium whitespace-nowrap transition-[background-color,filter,color] duration-150 disabled:cursor-not-allowed disabled:opacity-50 ${sizing} ${VARIANTS[variant]}`;
}

export function Button({
  variant = "primary",
  size = "md",
  loading = false,
  disabled,
  className = "",
  type = "button",
  children,
  ...rest
}: ButtonProps) {
  return (
    <button
      type={type}
      disabled={disabled || loading}
      aria-busy={loading || undefined}
      className={`${buttonClasses(variant, size)} ${className}`}
      {...rest}
    >
      {loading && <Spinner />}
      {children}
    </button>
  );
}
