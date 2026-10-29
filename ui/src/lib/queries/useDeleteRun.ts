import { useMutation, useQueryClient } from "@tanstack/react-query";

import { deleteRun } from "@/lib/api/runs";
import { queryKeys } from "./keys";

export function useDeleteRun() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: deleteRun,
    onSuccess: (_, id) => {
      queryClient.removeQueries({ queryKey: queryKeys.run(id) });
      // Lists and comparisons (all under "runs") may include the deleted run.
      queryClient.invalidateQueries({ queryKey: queryKeys.runsAll });
    },
  });
}
