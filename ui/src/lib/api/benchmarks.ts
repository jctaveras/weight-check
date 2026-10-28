import { call } from "./client";
import type { CategoryCatalog } from "./types";

export const getCatalog = () => call<CategoryCatalog[]>("benchmark_catalog");
