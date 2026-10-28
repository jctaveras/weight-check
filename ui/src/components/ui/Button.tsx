import type { ButtonHTMLAttributes } from "react";

import { buttonClasses, type Variant } from "./buttonClasses";
import { Spinner } from "./Spinner";

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: Variant;
  /** Shows a spinner and disables the button while a request is in flight. */
  loading?: boolean;
  size?: "sm" | "md";
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
