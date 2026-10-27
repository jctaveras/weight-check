import { createFileRoute } from "@tanstack/react-router";

import { ActiveModelCard } from "@/components/models/ActiveModelCard";
import { RecentRuns } from "@/components/runs/RecentRuns";
import { SystemPanel } from "@/components/system/SystemPanel";
import { PageHeader } from "@/components/ui/PageHeader";

export const Route = createFileRoute("/")({
  component: DashboardPage,
});

function DashboardPage() {
  return (
    <>
      <PageHeader
        title="Dashboard"
        description="Live hardware telemetry, the loaded model and your latest benchmark runs."
      />
      <div className="flex flex-col gap-6">
        <div className="grid gap-6 lg:grid-cols-2">
          <SystemPanel />
          <ActiveModelCard showManageLink />
        </div>
        <RecentRuns />
      </div>
    </>
  );
}
