"use client";

import { Badge } from "@/components/ui/Badge";
import { Card } from "@/components/ui/Card";
import { ErrorMessage } from "@/components/ui/ErrorMessage";
import { LoadingText } from "@/components/ui/LoadingText";
import { useSystem } from "@/lib/queries/useSystem";
import { GpuStats } from "./GpuStats";
import { HostStats } from "./HostStats";

export function SystemPanel() {
  const { data, error, isPending } = useSystem();

  return (
    <Card
      title="System"
      actions={
        data && (
          <>
            {data.os && <Badge>{data.os}</Badge>}
            <Badge tone="info">{data.backend}</Badge>
          </>
        )
      }
    >
      {isPending ? (
        <LoadingText>Reading telemetry…</LoadingText>
      ) : error && !data ? (
        <ErrorMessage error={error} title="Couldn't read system telemetry" />
      ) : data ? (
        <div className="grid gap-6 sm:grid-cols-2">
          <HostStats host={data.host} />
          <GpuStats gpu={data.gpu} />
        </div>
      ) : null}
    </Card>
  );
}
