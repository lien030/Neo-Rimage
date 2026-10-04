import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { formatBackendError } from "@/features/backend";
import type { AppError, ProcessingStage } from "@/lib/ipc";
import { loadJobErrors, type JobErrorProgress } from "./job-errors";
import { inputFileName } from "./worker-display";

interface JobErrorsState {
  progress: JobErrorProgress | null;
  loading: boolean;
  error: unknown;
}

export default function JobErrorsDialog({
  jobId,
  onClose,
  onReturnFocus,
}: {
  jobId: string;
  onClose: () => void;
  onReturnFocus: () => void;
}) {
  const { t } = useTranslation();
  const [refresh, setRefresh] = useState(0);
  const [state, setState] = useState<JobErrorsState>({
    progress: null,
    loading: true,
    error: null,
  });

  useEffect(() => {
    const controller = new AbortController();
    setState({ progress: null, loading: true, error: null });
    void loadJobErrors(
      jobId,
      (progress) => setState({ progress, loading: true, error: null }),
      controller.signal,
    ).then(
      () => {
        if (!controller.signal.aborted) {
          setState((current) => ({ ...current, loading: false }));
        }
      },
      (error: unknown) => {
        if (!controller.signal.aborted) {
          const detail = error && typeof error === "object" && "error" in error
            ? error.error
            : error;
          if (detail && typeof detail === "object" && "code" in detail && detail.code === "job.not_found") {
            onClose();
          } else {
            setState((current) => ({ ...current, loading: false, error }));
          }
        }
      },
    );
    return () => controller.abort();
  }, [jobId, refresh, onClose]);

  return (
    <Dialog open onOpenChange={(open) => { if (!open) onClose(); }}>
      <DialogContent
        className="flex max-h-[85vh] min-h-0 flex-col overflow-hidden sm:max-w-2xl"
        onCloseAutoFocus={(event) => {
          event.preventDefault();
          onReturnFocus();
        }}
      >
        <DialogHeader className="min-w-0 pr-6">
          <DialogTitle>{t("jobErrorsTitle")}</DialogTitle>
          <DialogDescription className="break-all text-xs">
            {t("jobErrorsDescription", { jobId })}
          </DialogDescription>
        </DialogHeader>
        <JobErrorsContent {...state} />
        <DialogFooter showCloseButton>
          <Button variant="outline" onClick={() => setRefresh((value) => value + 1)}>
            {t("jobErrorsRefresh")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

export function JobErrorsContent({ progress, loading, error }: JobErrorsState) {
  const { t } = useTranslation();
  return (
    <div className="min-h-0 min-w-0 flex-1 space-y-3 overflow-y-auto pr-1">
      <div role="status" aria-live="polite" className="text-xs text-muted-foreground">
        {loading && <p>{t("jobErrorsLoading")}</p>}
        {progress && (
          <>
            <p>{t("jobErrorsScanProgress", { checked: progress.checkedItems, total: progress.totalItems })}</p>
            <p>{t("jobErrorsFound", { count: progress.items.length })}</p>
          </>
        )}
      </div>
      {error !== null && (
        <div role="alert" className="space-y-1 break-words text-xs text-destructive">
          <p>{t("jobErrorsLoadFailed")} {formatBackendError(error, t)}</p>
          {progress && <p>{t("jobErrorsPartialHint")}</p>}
        </div>
      )}
      {progress?.jobError && (
        <article className="min-w-0 space-y-2 rounded-lg border p-3">
          <p className="font-medium">{t("jobErrorsTaskError")}</p>
          <StructuredError error={progress.jobError} />
        </article>
      )}
      <ul className="space-y-2">
        {progress?.items.map((item) => (
          <li key={item.id} className="min-w-0 space-y-2 rounded-lg border p-3">
            <p className="break-all font-medium">{inputFileName(item.inputPath)}</p>
            <StructuredError
              error={item.error}
              inputPath={item.inputPath}
              outputPath={item.outputPath}
              stage={item.stage}
            />
          </li>
        ))}
      </ul>
      {!loading && error === null && progress?.items.length === 0 && !progress.jobError && (
        <p className="py-5 text-center text-sm text-muted-foreground">{t("jobErrorsEmpty")}</p>
      )}
    </div>
  );
}

function StructuredError({
  error,
  inputPath,
  outputPath,
  stage,
}: {
  error: AppError | null;
  inputPath?: string;
  outputPath?: string | null;
  stage?: ProcessingStage | null;
}) {
  const { t } = useTranslation();
  const errorStage = error?.context.stage ?? stage;
  const fields: Array<[string, string | null | undefined]> = [
    ["jobErrorsInputPath", inputPath],
    ["jobErrorsOutputPath", outputPath],
    ["jobErrorsErrorCode", error?.code],
    ["jobErrorsCategory", error?.category],
    ["jobErrorsStage", errorStage ? t("processingStage." + errorStage) : null],
    ["jobErrorsContextPath", error?.context.path],
    ["jobErrorsDiagnosticMessage", error?.fallbackMessage],
    ["jobErrorsDiagnosticId", error?.diagnosticId],
  ];

  return (
    <>
      <p className="whitespace-pre-wrap break-words text-sm text-destructive">
        {error ? formatBackendError(error, t) : t("jobErrorsMissingDetail")}
      </p>
      <details className="text-xs">
        <summary className="cursor-pointer text-muted-foreground">
          {t("jobErrorsTechnicalDetails")}
        </summary>
        <dl className="mt-2 space-y-2">
          {fields.map(([key, value]) => value ? (
            <div key={key} className="min-w-0">
              <dt className="font-medium text-muted-foreground">{t(key)}</dt>
              <dd className="select-text whitespace-pre-wrap break-all">{value}</dd>
            </div>
          ) : null)}
        </dl>
      </details>
    </>
  );
}
