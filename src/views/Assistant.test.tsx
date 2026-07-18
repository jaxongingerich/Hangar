// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor, cleanup } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";

// Record every aiChatSend call so we can assert the content that actually
// reaches the backend. aiChatSend resolves on a later tick to mimic a real
// round-trip (and to let onMutate's setDraft("") re-render land first — the
// exact condition that used to send an empty string).
const sendCalls: Array<{ chatId: number; content: string; profileId: string }> = [];

vi.mock("../lib/api", () => ({
  api: {
    aiChatHistory: vi.fn(async () => []),
    aiChatSend: vi.fn(async (chatId: number, content: string, profileId: string) => {
      sendCalls.push({ chatId, content, profileId });
      await new Promise((r) => setTimeout(r, 5));
      return {
        id: 1,
        role: "assistant",
        content: "ok",
        provider: "cli",
        model: "claude",
        ts: new Date().toISOString(),
      };
    }),
    addLog: vi.fn(async () => {}),
    aiDeleteChat: vi.fn(async () => {}),
    readTextFile: vi.fn(async () => ({ name: "f", content: "", truncated: false })),
  },
}));

const pushSpy = vi.fn();
vi.mock("../lib/store", () => ({
  useToasts: () => ({ push: pushSpy }),
}));

// jsdom doesn't implement scrollIntoView; ChatThread calls it on mount.
Element.prototype.scrollIntoView = vi.fn();

import { ChatThread } from "./Assistant";

function renderThread() {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={qc}>
      <ChatThread
        chatId={42}
        profileId="p1"
        profiles={[]}
        projectId={null}
        onDeleted={() => {}}
      />
    </QueryClientProvider>,
  );
}

describe("ChatThread send", () => {
  beforeEach(() => {
    sendCalls.length = 0;
    pushSpy.mockClear();
    cleanup();
  });

  it("sends the typed message content (not an empty string) on click", async () => {
    const user = userEvent.setup();
    renderThread();
    const box = await screen.findByPlaceholderText(/Message/i);
    await user.type(box, "hello from the test");
    await user.click(screen.getByRole("button", { name: "Send" }));

    await waitFor(() => expect(sendCalls.length).toBe(1));
    expect(sendCalls[0]).toEqual({
      chatId: 42,
      content: "hello from the test",
      profileId: "p1",
    });
    // The composer clears after sending.
    await waitFor(() => expect((box as HTMLTextAreaElement).value).toBe(""));
    // No error toast.
    expect(pushSpy).not.toHaveBeenCalled();
  });

  it("sends on Enter with the correct content", async () => {
    const user = userEvent.setup();
    renderThread();
    const box = await screen.findByPlaceholderText(/Message/i);
    await user.type(box, "second message{Enter}");
    await waitFor(() => expect(sendCalls.length).toBe(1));
    expect(sendCalls[0].content).toBe("second message");
  });

  it("does nothing when the box is empty (no empty-message send)", async () => {
    const user = userEvent.setup();
    renderThread();
    const box = await screen.findByPlaceholderText(/Message/i);
    await user.type(box, "{Enter}");
    // Give it a moment; nothing should have been sent.
    await new Promise((r) => setTimeout(r, 30));
    expect(sendCalls.length).toBe(0);
  });

  it("appends attachment bodies to the sent content", async () => {
    const user = userEvent.setup();
    renderThread();
    const box = await screen.findByPlaceholderText(/Message/i);
    await user.type(box, "see file");
    await user.click(screen.getByRole("button", { name: "Send" }));
    await waitFor(() => expect(sendCalls.length).toBe(1));
    expect(sendCalls[0].content.startsWith("see file")).toBe(true);
  });
});
