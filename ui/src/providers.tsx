import { QueryClientProvider } from "@tanstack/react-query";
import { useState, type ReactNode } from "react";

import { makeQueryClient } from "./queryClient";

export function Providers({ children }: { children: ReactNode }) {
  const [queryClient] = useState(makeQueryClient);
  return <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>;
}
