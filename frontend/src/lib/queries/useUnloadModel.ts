import { useMutation, useQueryClient } from "@tanstack/react-query";

import { unloadModel } from "@/lib/api/models";
import { queryKeys } from "./keys";

export function useUnloadModel() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: unloadModel,
    onSettled: () =>
      Promise.all([
        queryClient.invalidateQueries({ queryKey: queryKeys.modelStatus }),
        queryClient.invalidateQueries({ queryKey: queryKeys.system }),
      ]),
  });
}
