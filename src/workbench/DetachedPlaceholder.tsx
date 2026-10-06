import { PictureInPicture2, Pin } from "lucide-react";

import { Button } from "@/components/ui";
import { useI18n } from "@/i18n";
import { dockPanel, type PopoutPanel } from "./popout";

export function DetachedPlaceholder({ panel }: { panel: PopoutPanel }) {
  const { t } = useI18n();
  return (
    <div className="flex h-full w-full flex-col items-center justify-center gap-2.5 bg-surface text-muted-foreground">
      <PictureInPicture2 className="size-5" />
      <p className="text-xs">{t("popout.detached")}</p>
      <Button
        type="button"
        variant="ghost"
        size="sm"
        onClick={() => void dockPanel(panel)}
      >
        <Pin />
        {t("popout.dockBack")}
      </Button>
    </div>
  );
}
