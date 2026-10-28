import { useId, type ReactNode } from "react";

import { inputClasses } from "./Field";

interface SelectFieldProps {
  label: string;
  value: string;
  onChange: (value: string) => void;
  children: ReactNode;
}

/** Labelled `<select>` for filters and pickers. */
export function SelectField({ label, value, onChange, children }: SelectFieldProps) {
  const id = useId();
  return (
    <div className="flex min-w-0 flex-col gap-1.5">
      <label htmlFor={id} className="text-xs font-medium text-muted">
        {label}
      </label>
      <select id={id} value={value} onChange={(e) => onChange(e.target.value)} className={inputClasses}>
        {children}
      </select>
    </div>
  );
}
