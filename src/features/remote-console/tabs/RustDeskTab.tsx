import { Copy, Radio } from "lucide-react";

import {
  Button,
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuSeparator,
  ContextMenuTrigger,
} from "@/components/ui";
import { useI18n } from "@/i18n";
import { useRcRustDesk } from "../api";
import { Card, CardTitle } from "../parts";
import { copyText } from "../helpers";

export function RustDeskTab() {
  const { t } = useI18n();
  const rustdeskQuery = useRcRustDesk();
  const rustdesk = rustdeskQuery.data;

  const copyBlock = () => {
    if (!rustdesk) return;
    copyText(
      `${t("remote.rustdeskServer")}: ${rustdesk.domain}\nKey: ${rustdesk.key}\n`,
      t,
    );
  };

  return (
    <ContextMenu>
      <ContextMenuTrigger asChild>
        <Card>
          <CardTitle
            icon={Radio}
            tone="success"
            title={t("remote.rustdesk")}
            actions={
              <Button
                type="button"
                variant="ghost"
                size="sm"
                disabled={!rustdesk}
                onClick={copyBlock}
              >
                <Copy />
                {t("remote.copy")}
              </Button>
            }
          />
          <div className="mt-2.5 flex flex-col gap-1.5">
            <div className="flex items-center justify-between gap-2">
              <span className="text-[0.6875rem] text-muted-foreground">
                {t("remote.rustdeskServer")}
              </span>
              <span className="truncate font-mono text-[0.6875rem] text-foreground">
                {rustdesk?.domain || "—"}
              </span>
            </div>
            <div className="flex items-center justify-between gap-2">
              <span className="text-[0.6875rem] text-muted-foreground">
                {t("remote.rustdeskRelay")}
              </span>
              <span className="truncate font-mono text-[0.6875rem] text-foreground">
                {rustdesk?.relay || "—"}
              </span>
            </div>
            <p className="rounded-lg bg-surface-sunken px-2 py-1.5 font-mono text-[0.625rem] leading-relaxed break-all text-muted-foreground">
              {rustdesk?.key || "—"}
            </p>
          </div>
        </Card>
      </ContextMenuTrigger>
      <ContextMenuContent>
        <ContextMenuItem
          disabled={!rustdesk?.domain}
          onSelect={() => copyText(rustdesk?.domain ?? "", t)}
        >
          <Copy /> {t("remote.copyServerAddress")}
        </ContextMenuItem>
        <ContextMenuItem
          disabled={!rustdesk?.key}
          onSelect={() => copyText(rustdesk?.key ?? "", t)}
        >
          <Copy /> {t("remote.copyKey")}
        </ContextMenuItem>
        <ContextMenuSeparator />
        <ContextMenuItem disabled={!rustdesk} onSelect={copyBlock}>
          <Copy /> {t("remote.copyBlock")}
        </ContextMenuItem>
      </ContextMenuContent>
    </ContextMenu>
  );
}
