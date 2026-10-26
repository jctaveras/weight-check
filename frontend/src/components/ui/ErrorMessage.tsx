import Link from "next/link";

import { describeError } from "@/lib/errors";

interface ErrorMessageProps {
  error: unknown;
  /** Optional heading, e.g. "Couldn't load models". */
  title?: string;
}

/** Announced error callout with the backend message and a suggested next step. */
export function ErrorMessage({ error, title }: ErrorMessageProps) {
  const { message, hint, action } = describeError(error);
  return (
    <div
      role="alert"
      className="rounded-md border border-danger/40 bg-danger-soft px-4 py-3 text-sm text-danger"
    >
      {title && <p className="font-semibold">{title}</p>}
      <p className="break-words">{message}</p>
      {hint && <p className="mt-1 text-fg/80">{hint}</p>}
      {action && (
        <Link href={action.href} className="mt-2 inline-block font-medium underline underline-offset-2">
          {action.label}
        </Link>
      )}
    </div>
  );
}
