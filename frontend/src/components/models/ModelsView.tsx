"use client";

import { useState } from "react";

import { Button } from "@/components/ui/Button";
import { Card } from "@/components/ui/Card";
import { ErrorMessage } from "@/components/ui/ErrorMessage";
import { LoadingText } from "@/components/ui/LoadingText";
import { useModelStatus } from "@/lib/queries/useModelStatus";
import { useModels } from "@/lib/queries/useModels";
import { ActiveModelCard } from "./ActiveModelCard";
import { LoadModelForm } from "./LoadModelForm";
import { ModelTable } from "./ModelTable";

export function ModelsView() {
  const models = useModels();
  const { data: status } = useModelStatus();
  const [selectedId, setSelectedId] = useState("");

  const list = models.data ?? [];
  const loadedId = status?.loaded?.model.id ?? null;
  const loadingId = status?.loading_model_id ?? null;

  return (
    <div className="flex flex-col gap-6">
      <div className="grid gap-6 lg:grid-cols-2">
        <ActiveModelCard />
        <Card title="Load a Model">
          {models.isPending ? (
            <LoadingText>Scanning models…</LoadingText>
          ) : (
            <LoadModelForm
              models={list}
              selectedId={selectedId}
              onSelect={setSelectedId}
              busy={loadingId !== null}
              hasLoadedModel={loadedId !== null}
            />
          )}
        </Card>
      </div>

      <Card
        title="Available Models"
        actions={
          <Button
            size="sm"
            variant="ghost"
            loading={models.isFetching && !models.isPending}
            onClick={() => models.refetch()}
          >
            Rescan
          </Button>
        }
      >
        {models.isPending ? (
          <LoadingText>Scanning models…</LoadingText>
        ) : models.error ? (
          <ErrorMessage error={models.error} title="Couldn't list models" />
        ) : (
          <ModelTable
            models={list}
            loadedId={loadedId}
            loadingId={loadingId}
            selectedId={selectedId}
            onSelect={setSelectedId}
          />
        )}
      </Card>
    </div>
  );
}
