import type { ReactNode } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  useEffect: vi.fn<(effect: () => void | (() => void)) => void>(),
  loadJobErrors: vi.fn(),
  contentProps: null as null | { onCloseAutoFocus: (event: { preventDefault: () => void }) => void },
}));

vi.mock("react", async (importOriginal) => ({
  ...await importOriginal<typeof import("react")>(),
  useEffect: mocks.useEffect,
}));
vi.mock("react-i18next", async (importOriginal) => ({
  ...await importOriginal<typeof import("react-i18next")>(),
  useTranslation: () => ({ t: (key: string) => key }),
}));
vi.mock("./job-errors", () => ({ loadJobErrors: mocks.loadJobErrors }));
vi.mock("@/components/ui/dialog", () => {
  const Container = ({ children }: { children: ReactNode }) => <div>{children}</div>;
  return {
    Dialog: Container,
    DialogHeader: Container,
    DialogTitle: Container,
    DialogDescription: Container,
    DialogFooter: Container,
    DialogContent: ({ children, ...props }: { children: ReactNode; onCloseAutoFocus: (event: { preventDefault: () => void }) => void }) => {
      mocks.contentProps = props;
      return <div>{children}</div>;
    },
  };
});

import JobErrorsDialog from "./JobErrorsDialog";

function render(onClose = vi.fn(), onReturnFocus = vi.fn()) {
  renderToStaticMarkup(<JobErrorsDialog jobId="job-1" onClose={onClose} onReturnFocus={onReturnFocus} />);
  return { onClose, onReturnFocus, cleanup: mocks.useEffect.mock.calls[0][0]() };
}

describe("job error dialog lifecycle", () => {
  beforeEach(() => {
    vi.resetAllMocks();
    mocks.contentProps = null;
    mocks.loadJobErrors.mockResolvedValue(undefined);
  });

  it.each([
    { code: "job.not_found" },
    { error: { code: "job.not_found" } },
  ])("closes immediately when the backend says the task was removed", async (error) => {
    mocks.loadJobErrors.mockRejectedValueOnce(error);
    const { onClose } = render();
    await vi.waitFor(() => expect(onClose).toHaveBeenCalledOnce());
  });

  it("keeps the dialog open for recoverable loading errors", async () => {
    const failure = Promise.reject(new Error("temporary query failure"));
    mocks.loadJobErrors.mockReturnValueOnce(failure);
    const { onClose } = render();
    await failure.catch(() => {});
    expect(onClose).not.toHaveBeenCalled();
  });

  it("aborts on close and ignores a late not-found error from the previous dialog", async () => {
    let reject!: (error: unknown) => void;
    const pending = new Promise<void>((_resolve, rejectPromise) => { reject = rejectPromise; });
    mocks.loadJobErrors.mockReturnValueOnce(pending);
    const { cleanup, onClose } = render();
    cleanup?.();
    expect(mocks.loadJobErrors.mock.calls[0][2].aborted).toBe(true);
    reject({ error: { code: "job.not_found" } });
    await pending.catch(() => {});
    expect(onClose).not.toHaveBeenCalled();
  });

  it("restores focus through its opener callback when the dialog closes", () => {
    const { onReturnFocus } = render();
    const event = { preventDefault: vi.fn() };
    mocks.contentProps!.onCloseAutoFocus(event);
    expect(event.preventDefault).toHaveBeenCalledOnce();
    expect(onReturnFocus).toHaveBeenCalledOnce();
  });
});
