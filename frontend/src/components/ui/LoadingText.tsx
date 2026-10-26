import { Spinner } from "./Spinner";

interface LoadingTextProps {
  children: string;
}

/** Inline loading indicator; text should end with "…". */
export function LoadingText({ children }: LoadingTextProps) {
  return (
    <p role="status" className="flex items-center gap-2 text-sm text-muted">
      <Spinner />
      {children}
    </p>
  );
}
