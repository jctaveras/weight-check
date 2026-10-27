import { useId, type InputHTMLAttributes, type ReactNode } from "react";

export const inputClasses =
  "h-10 w-full min-w-0 rounded-md border border-border bg-surface px-3 text-sm text-fg placeholder:text-muted/70 aria-invalid:border-danger";

interface FieldProps extends InputHTMLAttributes<HTMLInputElement> {
  label: ReactNode;
  hint?: ReactNode;
  error?: string;
}

/** Labelled input with an optional hint and inline error. */
export function Field({ label, hint, error, id, className = "", ...rest }: FieldProps) {
  const generated = useId();
  const inputId = id ?? generated;
  const hintId = hint ? `${inputId}-hint` : undefined;
  const errorId = error ? `${inputId}-error` : undefined;
  return (
    <div className={`flex min-w-0 flex-col gap-1.5 ${className}`}>
      <label htmlFor={inputId} className="text-sm font-medium">
        {label}
      </label>
      <input
        id={inputId}
        aria-invalid={error ? true : undefined}
        aria-describedby={[hintId, errorId].filter(Boolean).join(" ") || undefined}
        className={inputClasses}
        {...rest}
      />
      {hint && (
        <p id={hintId} className="text-xs text-muted">
          {hint}
        </p>
      )}
      {error && (
        <p id={errorId} className="text-xs text-danger">
          {error}
        </p>
      )}
    </div>
  );
}

type NumberFieldProps = Omit<FieldProps, "type" | "inputMode">;

/** Integer input; empty means "use the backend default". */
export function NumberField(props: NumberFieldProps) {
  return <Field type="number" inputMode="numeric" step={1} autoComplete="off" {...props} />;
}
