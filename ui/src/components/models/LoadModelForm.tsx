import { useId, useRef, useState, type FormEvent } from "react";

import { Button } from "@/components/ui/Button";
import { ErrorMessage } from "@/components/ui/ErrorMessage";
import { NumberField, inputClasses } from "@/components/ui/Field";
import type { ModelInfo } from "@/lib/api/types";
import { useLoadModel } from "@/lib/queries/useLoadModel";
import { parseLoadForm, type LoadFormField } from "./loadForm";

interface LoadModelFormProps {
  models: ModelInfo[];
  selectedId: string;
  onSelect: (id: string) => void;
  /** Another load is in progress (possibly from another tab). */
  busy: boolean;
  hasLoadedModel: boolean;
}

export function LoadModelForm({
  models,
  selectedId,
  onSelect,
  busy,
  hasLoadedModel,
}: LoadModelFormProps) {
  const selectId = useId();
  const formRef = useRef<HTMLFormElement>(null);
  const [errors, setErrors] = useState<Partial<Record<LoadFormField, string>>>({});
  const load = useLoadModel();

  function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const data = new FormData(event.currentTarget);
    const value = (name: LoadFormField) => String(data.get(name) ?? "");
    const result = parseLoadForm({
      model_id: value("model_id"),
      ctx_size: value("ctx_size"),
      n_gpu_layers: value("n_gpu_layers"),
      parallel: value("parallel"),
      threads: value("threads"),
    });
    if (!result.ok) {
      setErrors(result.errors);
      const first = Object.keys(result.errors)[0];
      formRef.current?.querySelector<HTMLElement>(`[name="${first}"]`)?.focus();
      return;
    }
    setErrors({});
    load.mutate(result.request);
  }

  const selected = models.find((m) => m.id === selectedId);

  return (
    <form ref={formRef} onSubmit={handleSubmit} noValidate className="flex flex-col gap-4">
      <div className="flex min-w-0 flex-col gap-1.5">
        <label htmlFor={selectId} className="text-sm font-medium">
          Model
        </label>
        <select
          id={selectId}
          name="model_id"
          value={selectedId}
          onChange={(e) => onSelect(e.target.value)}
          aria-invalid={errors.model_id ? true : undefined}
          aria-describedby={errors.model_id ? `${selectId}-error` : undefined}
          className={inputClasses}
        >
          <option value="">Choose a model…</option>
          {models.map((m) => (
            <option key={m.id} value={m.id}>
              {m.name}
            </option>
          ))}
        </select>
        {errors.model_id && (
          <p id={`${selectId}-error`} className="text-xs text-danger">
            {errors.model_id}
          </p>
        )}
      </div>

      <div className="grid gap-4 sm:grid-cols-2">
        <NumberField
          label="Context Size"
          name="ctx_size"
          min={128}
          placeholder="4096"
          hint="Tokens; default 4096."
          error={errors.ctx_size}
        />
        <NumberField
          label="GPU Layers"
          name="n_gpu_layers"
          min={0}
          placeholder={selected?.block_count ? `e.g. ${Math.ceil(selected.block_count / 2)}` : "999"}
          hint={
            selected?.block_count
              ? `Model has ${selected.block_count} layers; default offloads all.`
              : "Default 999 offloads all layers."
          }
          error={errors.n_gpu_layers}
        />
        <NumberField
          label="Parallel Slots"
          name="parallel"
          min={1}
          max={64}
          placeholder="1"
          hint="Concurrent requests (1–64)."
          error={errors.parallel}
        />
        <NumberField
          label="Threads"
          name="threads"
          placeholder="auto"
          hint="CPU threads; blank lets llama-server decide."
          error={errors.threads}
        />
      </div>

      {load.error && <ErrorMessage error={load.error} title="Couldn't load the model" />}
      {load.isSuccess && !load.isPending && (
        <p role="status" className="text-sm text-success">
          Loaded {load.data.model.name}.
        </p>
      )}

      <div className="flex flex-wrap items-center gap-3">
        <Button type="submit" loading={load.isPending} disabled={busy && !load.isPending}>
          {load.isPending ? "Loading Model…" : hasLoadedModel ? "Swap Model" : "Load Model"}
        </Button>
        {hasLoadedModel && !load.isPending && (
          <p className="text-xs text-muted">Loading replaces the current model.</p>
        )}
        {load.isPending && (
          <p className="text-xs text-muted">Waiting for llama-server to become healthy…</p>
        )}
      </div>
    </form>
  );
}
