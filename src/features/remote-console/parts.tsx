import {
  Badge,
  Button,
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuTrigger,
} from "@/components/ui";
import { Copy } from "lucide-react";
import type { LucideIcon } from "lucide-react";

import { useI18n } from "@/i18n";
import { cn } from "@/lib/utils";
import { TONE_CLASS, copyText, type Tone } from "./helpers";

export function Card({
  children,
  className,
  ...props
}: React.ComponentProps<"section">) {
  return (
    <section
      className={cn(
        "flex flex-col rounded-lg border border-border-subtle bg-surface-raised p-3",
        className,
      )}
      {...props}
    >
      {children}
    </section>
  );
}

export function CardTitle({
  icon: Icon,
  tone,
  title,
  actions,
}: {
  icon: LucideIcon;
  tone: Tone;
  title: string;
  actions?: React.ReactNode;
}) {
  return (
    <div className="flex items-center justify-between gap-2">
      <div className="flex min-w-0 items-center gap-2">
        <span
          className={cn(
            "flex size-6 shrink-0 items-center justify-center rounded-md",
            TONE_CLASS[tone],
          )}
        >
          <Icon className="size-3.5" strokeWidth={1.9} />
        </span>
        <h3 className="truncate text-[0.8125rem] font-semibold tracking-[-0.01em] text-foreground">
          {title}
        </h3>
      </div>
      {actions && (
        <div className="flex shrink-0 items-center gap-1">{actions}</div>
      )}
    </div>
  );
}

export function StatTile({
  icon: Icon,
  tone,
  label,
  value,
  unit,
  copyValue,
}: {
  icon: LucideIcon;
  tone: Tone;
  label: string;
  value: string;
  unit?: string;
  copyValue?: string;
}) {
  const { t } = useI18n();
  const tile = (
    <div className="flex min-w-0 flex-col gap-1.5 rounded-lg border border-border-subtle bg-surface p-2.5">
      <span
        className={cn(
          "flex size-6 shrink-0 items-center justify-center rounded-md",
          TONE_CLASS[tone],
        )}
      >
        <Icon className="size-3.5" strokeWidth={1.9} />
      </span>
      <span className="truncate text-[0.6875rem] text-muted-foreground">
        {label}
      </span>
      <span className="flex items-baseline gap-0.5">
        <span className="truncate text-sm font-semibold text-foreground">
          {value}
        </span>
        {unit && (
          <span className="text-[0.625rem] text-muted-foreground">{unit}</span>
        )}
      </span>
    </div>
  );

  if (!copyValue && value === "—") return tile;

  return (
    <ContextMenu>
      <ContextMenuTrigger asChild>{tile}</ContextMenuTrigger>
      <ContextMenuContent>
        <ContextMenuItem
          onSelect={() =>
            copyText(copyValue ?? `${value}${unit ? ` ${unit}` : ""}`, t)
          }
        >
          <Copy /> {t("remote.copyValue")}
        </ContextMenuItem>
      </ContextMenuContent>
    </ContextMenu>
  );
}

export function StatusDot({ ok }: { ok: boolean }) {
  return (
    <span
      aria-hidden
      className={cn(
        "inline-block size-1.5 shrink-0 rounded-full",
        ok ? "bg-success" : "bg-destructive",
      )}
    />
  );
}

export function ProxyBadge({
  stale,
  online,
}: {
  stale: boolean;
  online: boolean;
}) {
  const { t } = useI18n();
  return (
    <Badge variant={stale ? "outline" : online ? "success" : "destructive"}>
      {stale
        ? t("remote.stale")
        : online
          ? t("remote.online")
          : t("remote.offline")}
    </Badge>
  );
}

export function RefreshButton({
  loading,
  label,
  onClick,
}: {
  loading?: boolean;
  label: string;
  onClick: () => void;
}) {
  return (
    <Button
      type="button"
      variant="ghost"
      size="sm"
      loading={loading}
      onClick={onClick}
    >
      {label}
    </Button>
  );
}
