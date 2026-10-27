"use client";

import { useRouter } from "next/navigation";
import { useId, useRef, useState, type FormEvent } from "react";

import { Button } from "@/components/ui/Button";
import { ErrorMessage } from "@/components/ui/ErrorMessage";
import type { BenchmarkDescriptor } from "@/lib/api/types";
import { useStartRun } from "@/lib/queries/useStartRun";
import { parseConfig, stringifyConfig } from "./configJson";

interface RunBenchmarkFormProps {
  benchmark: BenchmarkDescriptor;
}

export function RunBenchmarkForm({ benchmark }: RunBenchmarkFormProps) {
  const id = useId();
  const router = useRouter();
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const defaults = stringifyConfig(benchmark.default_config);
  const [text, setText] = useState(defaults);
  const [parseError, setParseError] = useState<string | null>(null);
  const start = useStartRun();

  function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const parsed = parseConfig(text);
    if (!parsed.ok) {
      setParseError(parsed.error);
      textareaRef.current?.focus();
      return;
    }
    setParseError(null);
    start.mutate(
      { benchmark_id: benchmark.id, config: parsed.value },
      {
        onSuccess: (run) => {
          // The page stays mounted (hidden) under Cache Components; clear the result.
          start.reset();
          router.push(`/runs/${run.id}`);
        },
      },
    );
  }

  const errorId = `${id}-error`;
  const hintId = `${id}-hint`;

  return (
    <form onSubmit={handleSubmit} noValidate className="flex flex-col gap-3">
      <div className="flex flex-col gap-1.5">
        <label htmlFor={id} className="text-sm font-medium">
          Config (JSON)
        </label>
        <textarea
          ref={textareaRef}
          id={id}
          name="config"
          value={text}
          onChange={(e) => setText(e.target.value)}
          rows={Math.min(14, Math.max(4, defaults.split("\n").length + 1))}
          spellCheck={false}
          autoComplete="off"
          autoCapitalize="off"
          translate="no"
          aria-invalid={parseError ? true : undefined}
          aria-describedby={parseError ? `${hintId} ${errorId}` : hintId}
          className="w-full min-w-0 resize-y rounded-md border border-border bg-surface-muted p-3 font-mono text-xs leading-relaxed text-fg aria-invalid:border-danger"
        />
        <p id={hintId} className="text-xs text-muted">
          Edit fields to override defaults. Unknown fields are rejected by the backend.
        </p>
        {parseError && (
          <p id={errorId} className="text-xs text-danger">
            {parseError}
          </p>
        )}
      </div>

      {start.error && <ErrorMessage error={start.error} title="Couldn't start the run" />}

      <div className="flex flex-wrap gap-2">
        <Button type="submit" loading={start.isPending}>
          {start.isPending ? "Starting…" : "Start Run"}
        </Button>
        <Button
          variant="ghost"
          disabled={text === defaults || start.isPending}
          onClick={() => {
            setText(defaults);
            setParseError(null);
          }}
        >
          Reset to Defaults
        </Button>
      </div>
    </form>
  );
}
