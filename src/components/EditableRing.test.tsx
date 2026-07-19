// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor, cleanup } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";

const setProgress = vi.fn(async () => {});
const setProgressMode = vi.fn(async () => {});

vi.mock("../lib/api", () => ({
  api: {
    setProgress: (...a: unknown[]) => setProgress(...(a as [])),
    setProgressMode: (...a: unknown[]) => setProgressMode(...(a as [])),
  },
}));

const pushSpy = vi.fn();
vi.mock("../lib/store", () => ({
  useToasts: () => ({ push: pushSpy }),
}));

import { EditableRing } from "./EditableRing";

function renderRing(progress = 30, mode: "manual" | "milestones" = "manual") {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={qc}>
      <EditableRing
        project={{ id: 7, progress, progress_mode: mode }}
        color="#fff"
      />
    </QueryClientProvider>,
  );
}

describe("EditableRing", () => {
  beforeEach(() => {
    cleanup();
    setProgress.mockClear();
    setProgressMode.mockClear();
    pushSpy.mockClear();
  });

  // The whole point of the feature: the number is clickable and typeable.
  it("clicking the number lets you type a percentage and saves it", async () => {
    const user = userEvent.setup();
    renderRing(30);

    await user.click(screen.getByTitle("Click to type a percentage"));
    const input = screen.getByLabelText("Progress percentage");
    await user.clear(input);
    await user.type(input, "65{Enter}");

    await waitFor(() => expect(setProgress).toHaveBeenCalledWith(7, 65));
  });

  // Typing a number while milestones drive progress has to opt out of that
  // mode, or the next milestone change silently overwrites what was typed.
  it("switches off milestone mode so a typed number sticks", async () => {
    const user = userEvent.setup();
    renderRing(30, "milestones");

    await user.click(screen.getByTitle("Click to type a percentage"));
    const input = screen.getByLabelText("Progress percentage");
    await user.clear(input);
    await user.type(input, "80{Enter}");

    await waitFor(() => expect(setProgressMode).toHaveBeenCalledWith(7, "manual"));
  });

  it("does not touch milestone mode when progress is already manual", async () => {
    const user = userEvent.setup();
    renderRing(30, "manual");

    await user.click(screen.getByTitle("Click to type a percentage"));
    const input = screen.getByLabelText("Progress percentage");
    await user.clear(input);
    await user.type(input, "45{Enter}");

    await waitFor(() => expect(setProgress).toHaveBeenCalled());
    expect(setProgressMode).not.toHaveBeenCalled();
  });

  it("clamps out-of-range numbers instead of writing them", async () => {
    const user = userEvent.setup();
    renderRing(30);

    await user.click(screen.getByTitle("Click to type a percentage"));
    const input = screen.getByLabelText("Progress percentage");
    await user.clear(input);
    await user.type(input, "500{Enter}");

    await waitFor(() => expect(setProgress).toHaveBeenCalledWith(7, 100));
  });

  it("Escape cancels without saving", async () => {
    const user = userEvent.setup();
    renderRing(30);

    await user.click(screen.getByTitle("Click to type a percentage"));
    const input = screen.getByLabelText("Progress percentage");
    await user.clear(input);
    await user.type(input, "99{Escape}");

    await waitFor(() =>
      expect(screen.queryByLabelText("Progress percentage")).toBeNull(),
    );
    expect(setProgress).not.toHaveBeenCalled();
  });

  // Clicking away is a normal way to finish typing, not a reason to lose it.
  it("saves on blur", async () => {
    const user = userEvent.setup();
    renderRing(30);

    await user.click(screen.getByTitle("Click to type a percentage"));
    const input = screen.getByLabelText("Progress percentage");
    await user.clear(input);
    await user.type(input, "55");
    await user.tab();

    await waitFor(() => expect(setProgress).toHaveBeenCalledWith(7, 55));
  });
});
