import { Trash2 } from "lucide-react";
import { Fragment, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import {
  cleanupJobs,
  formatBackendError,
  isFinishedJob,
  refreshBackendSnapshot,
  selectCleanupJobIds,
  useBackendCommandState,
  useBackendRuntimeState,
  type TaskCleanupGroup,
  type TaskCleanupResult,
} from "@/features/backend";
import { Button } from "./ui/button";
import {
  Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle,
} from "./ui/dialog";
import {
  DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuLabel,
  DropdownMenuSeparator, DropdownMenuTrigger,
} from "./ui/dropdown-menu";

const GROUPS: TaskCleanupGroup[] = ["succeeded", "unsuccessful", "all"];

export default function TaskCleanupMenu() {
  const { t } = useTranslation();
  const backend = useBackendRuntimeState();
  const commands = useBackendCommandState();
  const triggerRef = useRef<HTMLButtonElement>(null);
  const focusFallbackRef = useRef<HTMLSpanElement>(null);
  const [batch, setBatch] = useState<{ ids: string[]; activeCount: number } | null>(null);
  const [result, setResult] = useState<TaskCleanupResult | null>(null);
  const [refreshing, setRefreshing] = useState(false);
  const jobs = backend.snapshot?.jobs ?? [];
  const ready = backend.syncStatus === "ready" && backend.snapshot !== null;
  const pending = commands.cleanupPending || refreshing;
  const removable = selectCleanupJobIds(jobs, "all").length;

  function openConfirmation(group: TaskCleanupGroup) {
    const ids = selectCleanupJobIds(jobs, group);
    if (!ready || pending || ids.length === 0) return;
    setResult(null);
    setBatch({ ids, activeCount: jobs.filter((job) => !isFinishedJob(job)).length });
  }

  async function removeConfirmed(ids: readonly string[]) {
    if (!ready || pending) return;
    const next = await cleanupJobs(ids);
    setResult(result ? {
      ...next,
      completedIds: [...result.completedIds, ...next.completedIds],
      skippedIds: [...result.skippedIds, ...next.skippedIds],
      minimumRevision: Math.max(result.minimumRevision, next.minimumRevision),
    } : next);
  }

  async function retryRefresh() {
    if (!result || pending) return;
    setRefreshing(true);
    try {
      await refreshBackendSnapshot(result.minimumRevision);
      setResult({ ...result, refreshError: null });
    } catch (error: unknown) {
      setResult({ ...result, refreshError: error });
    } finally {
      setRefreshing(false);
    }
  }

  return <span ref={focusFallbackRef} tabIndex={-1} className="inline-flex outline-none">
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button
          ref={triggerRef}
          size="icon"
          disabled={!ready || pending || removable === 0}
          aria-label={t("taskCleanup.menu")}
          title={t("taskCleanup.menu")}
          className="h-7 w-12 rounded-lg border bg-background text-muted-foreground hover:bg-muted-foreground/10"
        >
          <Trash2 size={16} />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent
        align="end"
        className="w-52 max-w-[calc(100vw-2rem)]"
        onCloseAutoFocus={(event) => { if (batch) event.preventDefault(); }}
      >
        <DropdownMenuLabel className="px-2 text-muted-foreground">{t("taskCleanup.menu")}</DropdownMenuLabel>
        {GROUPS.map((group) => {
          const count = selectCleanupJobIds(jobs, group).length;
          return <Fragment key={group}>
            {group === "all" && <DropdownMenuSeparator />}
            <DropdownMenuItem
              className="gap-4 px-2 py-1.5"
              disabled={!ready || pending || count === 0}
              onSelect={() => openConfirmation(group)}
            >
              <span className="flex-1">{t(`taskCleanup.${group}`)}</span>
              <span className="min-w-5 text-right text-xs tabular-nums text-muted-foreground">{count}</span>
            </DropdownMenuItem>
          </Fragment>;
        })}
      </DropdownMenuContent>
    </DropdownMenu>

    <Dialog open={batch !== null} onOpenChange={(open) => {
      if (!open && !pending) setBatch(null);
    }}>
      <DialogContent
        className="flex max-h-[85vh] flex-col sm:max-w-lg"
        showCloseButton={!pending}
        onCloseAutoFocus={(event) => {
          event.preventDefault();
          // Clearing the last record disables the trigger. Keep the keyboard's
          // place in the toolbar so the next Tab still follows the menu.
          const target = triggerRef.current?.disabled ? focusFallbackRef.current : triggerRef.current;
          target?.focus();
        }}
      >
        <DialogHeader className="pr-6">
          <DialogTitle>{t(result ? "taskCleanup.resultTitle" : "taskCleanup.confirmTitle")}</DialogTitle>
          <DialogDescription>
            {t("taskCleanup.description", { count: batch?.ids.length ?? 0 })}
          </DialogDescription>
        </DialogHeader>
        <div className="min-h-0 space-y-3 overflow-y-auto break-words" aria-live="polite">
          {!result && <p>{t("taskCleanup.activeRetained", { count: batch?.activeCount ?? 0 })}</p>}
          {commands.cleanupPending && <p role="status">{t("taskCleanup.pending")}</p>}
          {result && <>
            <p>{t("taskCleanup.summary", {
              completed: result.completedIds.length,
              skipped: result.skippedIds.length,
              failed: result.failures.length,
            })}</p>
            {result.skippedIds.length > 0 && <p className="text-muted-foreground">{t("taskCleanup.skippedExplanation")}</p>}
            {result.failures.length > 0 && <ul className="space-y-2">
              {result.failures.map(({ jobId, error }) => <li key={jobId} className="rounded border p-2">
                <p className="break-all font-mono text-xs">{jobId}</p>
                <p className="text-destructive">{formatBackendError(error, t)}</p>
              </li>)}
            </ul>}
            {result.refreshError !== null && <div role="alert" className="space-y-2 rounded border p-2">
              <p className="text-destructive">{t("taskCleanup.refreshFailed")}</p>
              <p>{formatBackendError(result.refreshError, t)}</p>
              <Button variant="outline" disabled={pending} onClick={() => void retryRefresh()}>
                {t("taskCleanup.retryRefresh")}
              </Button>
            </div>}
          </>}
        </div>
        <DialogFooter className="shrink-0">
          <Button variant="outline" disabled={pending} onClick={() => setBatch(null)}>
            {t(result ? "close" : "cancel")}
          </Button>
          {!result && <Button disabled={!ready || pending} onClick={() => void removeConfirmed(batch?.ids ?? [])}>
            {t("taskCleanup.confirm")}
          </Button>}
          {result && result.failures.length > 0 && <Button
            disabled={!ready || pending}
            onClick={() => void removeConfirmed(result.failures.map(({ jobId }) => jobId))}
          >
            {t("taskCleanup.retryFailed", { count: result.failures.length })}
          </Button>}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  </span>;
}
