import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { RunStatusBadge } from "./RunStatusBadge";

describe("RunStatusBadge", () => {
  it.each([
    ["queued", "Queued"],
    ["running", "Running"],
    ["completed", "Completed"],
    ["failed", "Failed"],
  ] as const)("labels %s as %s", (status, label) => {
    render(<RunStatusBadge status={status} />);
    expect(screen.getByText(label)).toBeInTheDocument();
  });
});
