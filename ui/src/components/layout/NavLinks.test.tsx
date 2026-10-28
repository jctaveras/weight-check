import { screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { renderWithRouter } from "@/test/render";
import { NavLinks } from "./NavLinks";

const link = (name: string) => screen.getByRole("link", { name });

describe("NavLinks", () => {
  it("marks only the dashboard on /", async () => {
    await renderWithRouter(<NavLinks />, { path: "/" });
    expect(link("Dashboard")).toHaveAttribute("aria-current", "page");
    expect(link("Runs")).not.toHaveAttribute("aria-current");
  });

  it("marks a section on its subpages and with search params", async () => {
    await renderWithRouter(<NavLinks />, { path: "/runs/abc?x=1" });
    expect(link("Runs")).toHaveAttribute("aria-current", "page");
    expect(link("Runs")).toHaveClass("text-fg");
    expect(link("Dashboard")).not.toHaveAttribute("aria-current");
    expect(link("Dashboard")).toHaveClass("text-muted");
  });
});

it("links the Compare page", async () => {
  await renderWithRouter(<NavLinks />, { path: "/compare?benchmark=x" });
  expect(link("Compare")).toHaveAttribute("aria-current", "page");
  expect(link("Runs")).not.toHaveAttribute("aria-current");
});
