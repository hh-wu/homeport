import { useState } from "react";

import { Button, Input, Label } from "@/components/ui";
import { useI18n, type TKey } from "@/i18n";
import type { RcConfig } from "@/types/models";

const FIELDS: { key: keyof RcConfig; labelKey: TKey; numeric?: boolean }[] = [
  { key: "frpcPath", labelKey: "remote.cfgFrpcPath" },
  { key: "frpcConfigPath", labelKey: "remote.cfgFrpcConfig" },
  { key: "logPath", labelKey: "remote.cfgLogPath" },
  { key: "relayHost", labelKey: "remote.cfgRelayHost" },
  { key: "sshUser", labelKey: "remote.cfgSshUser" },
  { key: "sshKeyPath", labelKey: "remote.cfgSshKey" },
  { key: "dashboardUrl", labelKey: "remote.cfgDashboardUrl" },
  { key: "dashboardAuth", labelKey: "remote.cfgDashboardAuth" },
  { key: "rustdeskDomain", labelKey: "remote.cfgRustdeskDomain" },
  { key: "rustdeskKey", labelKey: "remote.cfgRustdeskKey" },
  { key: "instanceId", labelKey: "remote.cfgInstanceId" },
  { key: "region", labelKey: "remote.cfgRegion" },
  { key: "pricePerGb", labelKey: "remote.cfgPrice", numeric: true },
];

export function ConfigForm({
  config,
  saving,
  onSave,
}: {
  config: RcConfig;
  saving: boolean;
  onSave: (config: RcConfig) => void;
}) {
  const { t } = useI18n();
  const [draft, setDraft] = useState<RcConfig>(config);

  const update = (key: keyof RcConfig, value: string, numeric?: boolean) => {
    setDraft((current) => ({
      ...current,
      [key]: numeric ? Number(value.replace(",", ".")) || 0 : value,
    }));
  };

  return (
    <div className="mt-2 flex flex-col gap-2.5">
      {FIELDS.map((field) => (
        <div key={field.key} className="flex flex-col gap-1">
          <Label
            htmlFor={`rc-${field.key}`}
            className="text-xs text-muted-foreground"
          >
            {t(field.labelKey)}
          </Label>
          <Input
            id={`rc-${field.key}`}
            value={String(draft[field.key] ?? "")}
            inputMode={field.numeric ? "decimal" : undefined}
            spellCheck={false}
            onChange={(event) =>
              update(field.key, event.target.value, field.numeric)
            }
          />
        </div>
      ))}
      <Button
        type="button"
        variant="primary"
        size="sm"
        loading={saving}
        onClick={() => onSave(draft)}
      >
        {t("remote.cfgSave")}
      </Button>
      <p className="text-pretty text-xs text-muted-foreground">
        {t("remote.cfgHint")}
      </p>
    </div>
  );
}
