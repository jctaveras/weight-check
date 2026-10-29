import { screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { mockInvoke, renderWithQuery } from "@/test/render";
import { DeleteRunButton } from "./DeleteRunButton";

describe("DeleteRunButton", () => {
  it("deletes only after the inline confirmation", async () => {
    const invoke = mockInvoke({ delete_run: () => null });
    const onDeleted = vi.fn();
    renderWithQuery(<DeleteRunButton runId="run-1" onDeleted={onDeleted} />);

    await userEvent.click(screen.getByRole("button", { name: "Delete Run" }));
    expect(invoke).not.toHaveBeenCalled();
    const confirm = screen.getByRole("button", { name: "Confirm Delete" });
    expect(confirm).toHaveFocus();

    await userEvent.click(confirm);
    expect(invoke).toHaveBeenCalledWith("delete_run", { id: "run-1" });
    await vi.waitFor(() => expect(onDeleted).toHaveBeenCalledOnce());
  });

  it("Keep Run and Escape back out without deleting", async () => {
    const invoke = mockInvoke({ delete_run: () => null });
    renderWithQuery(<DeleteRunButton runId="run-1" />);

    await userEvent.click(screen.getByRole("button", { name: "Delete Run" }));
    await userEvent.click(screen.getByRole("button", { name: "Keep Run" }));
    expect(screen.getByRole("button", { name: "Delete Run" })).toHaveFocus();

    await userEvent.click(screen.getByRole("button", { name: "Delete Run" }));
    await userEvent.keyboard("{Escape}");
    expect(screen.queryByRole("button", { name: "Confirm Delete" })).not.toBeInTheDocument();
    expect(invoke).not.toHaveBeenCalled();
  });

  it("shows the error when the core refuses", async () => {
    mockInvoke({
      delete_run: () => {
        throw { code: "busy", message: "cancel the run before deleting it" };
      },
    });
    renderWithQuery(<DeleteRunButton runId="run-1" />);
    await userEvent.click(screen.getByRole("button", { name: "Delete Run" }));
    await userEvent.click(screen.getByRole("button", { name: "Confirm Delete" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("cancel the run before deleting it");
  });
});
