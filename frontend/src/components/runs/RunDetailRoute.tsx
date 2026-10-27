"use client";

import { useParams } from "next/navigation";

import { RunDetail } from "./RunDetail";

export function RunDetailRoute() {
  const { id } = useParams<{ id: string }>();
  // Key by id so local state never leaks between runs kept alive by Activity.
  return <RunDetail key={id} id={id} />;
}
