import { useMutation, useQueryClient } from "@tanstack/react-query";

import { cancelRun } from "@/lib/api/runs";
import { queryKeys } from "./keys";

export function useCancelRun() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: cancelRun,
    onSuccess: (run) => {
      // The run keeps polling until it reports `cancelled`; refresh lists and the model's
      // busy state now so they catch up as soon as it does.
      queryClient.invalidateQueries({ queryKey: queryKeys.run(run.id) });
      queryClient.invalidateQueries({ queryKey: queryKeys.runsAll });
      queryClient.invalidateQueries({ queryKey: queryKeys.modelStatus });
    },
  });
}
