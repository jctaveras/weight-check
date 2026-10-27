import { useMutation, useQueryClient } from "@tanstack/react-query";

import { loadModel } from "@/lib/api/models";
import { queryKeys } from "./keys";

export function useLoadModel() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: loadModel,
    // Refresh immediately so other views pick up `loading_model_id`.
    onMutate: () => queryClient.invalidateQueries({ queryKey: queryKeys.modelStatus }),
    onSettled: () =>
      Promise.all([
        queryClient.invalidateQueries({ queryKey: queryKeys.modelStatus }),
        queryClient.invalidateQueries({ queryKey: queryKeys.system }),
      ]),
  });
}
