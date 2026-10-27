import { call } from "./client";
import type { SystemSnapshot } from "./types";

export const getSystem = () => call<SystemSnapshot>("system_snapshot");
