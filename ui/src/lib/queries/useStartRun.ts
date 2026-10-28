import { useMutation, useQueryClient } from "@tanstack/react-query";

import { startRun } from "@/lib/api/runs";
import { queryKeys } from "./keys";

export function useStartRun() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: startRun,
    onSuccess: (run) => {
      queryClient.invalidateQueries({ queryKey: queryKeys.runsAll });
      queryClient.invalidateQueries({ queryKey: queryKeys.modelStatus });
      queryClient.setQueryData(queryKeys.run(run.id), { ...run, metrics: [] });
    },
  });
}
