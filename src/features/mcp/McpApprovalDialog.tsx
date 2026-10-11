import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogToolbar } from "@/components/ui/dialog";
import { useDialogSnapshot } from "@/components/ui/use-dialog-snapshot";
import { useI18n } from "@/i18n";
import type { McpApprovalEvent } from "@/types/models";
import { useMcpApprovalStore } from "./approval";

export function McpApprovalDialog() {
  const queue = useMcpApprovalStore((state) => state.queue);
  const respond = useMcpApprovalStore((state) => state.respond);
  const current = queue[0];
  const shown = useDialogSnapshot(Boolean(current), current);

  return (
    <Dialog
      open={Boolean(current)}
      onOpenChange={(open) =>
        !open && current && respond(current.requestId, false)
      }
    >
      <DialogContent
        showClose={false}
        className="flex w-[var(--dialog-width-md)] max-w-[92vw] flex-col gap-0 p-0 sm:p-0"
      >
        {shown && (
          <McpApprovalForm
            key={shown.requestId}
            request={shown}
            onRespond={(approved) => respond(shown.requestId, approved)}
          />
        )}
      </DialogContent>
    </Dialog>
  );
}

function McpApprovalForm({
  request,
  onRespond,
}: {
  request: McpApprovalEvent;
  onRespond: (approved: boolean) => void;
}) {
  const { t } = useI18n();

  return (
    <>
      <DialogToolbar>
        {t("mcp.approval.title", { client: request.client })}
      </DialogToolbar>
      <div className="ui-dialog-body">
        <p className="text-sm leading-relaxed text-muted-foreground">
          {t("mcp.approval.description", {
            client: request.client,
            risk: t(`mcp.approval.risk.${request.risk}`),
          })}
        </p>
        <pre className="ui-surface-card max-h-48 overflow-auto whitespace-pre-wrap break-all p-3 font-mono text-xs">
          {request.command}
        </pre>
        <p className="text-xs text-muted-foreground">
          {t("mcp.approval.hosts", { hosts: request.hosts.join(", ") })}
        </p>
        <div className="flex justify-end gap-2">
          <Button
            type="button"
            variant="ghost"
            onClick={() => onRespond(false)}
          >
            {t("mcp.approval.deny")}
          </Button>
          <Button
            type="button"
            variant="primary"
            autoFocus
            onClick={() => onRespond(true)}
          >
            {t("mcp.approval.approve")}
          </Button>
        </div>
      </div>
    </>
  );
}
