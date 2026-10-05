import { useState } from "react";

import { Button, Field, Input, Spinner } from "@/components/ui";
import { useI18n, type TKey } from "@/i18n";
import { useRcConfig, useRcSaveConfig } from "@/features/remote-console/api";
import { errorMessage, toast } from "@/lib/toast";
import type { RcConfig } from "@/types/models";
import { SETTINGS_GROUP_STACK_CLASS, SettingsGroup } from "./SettingsGroup";

type Draft = Record<keyof RcConfig, string>;

const GROUPS: {
  titleKey: TKey;
  fields: { key: keyof RcConfig; labelKey: TKey; numeric?: boolean }[];
}[] = [
  {
    titleKey: "settings.remote.frpc",
    fields: [
      { key: "frpcPath", labelKey: "remote.cfgFrpcPath" },
      { key: "frpcConfigPath", labelKey: "remote.cfgFrpcConfig" },
      { key: "logPath", labelKey: "remote.cfgLogPath" },
    ],
  },
  {
    titleKey: "settings.remote.relay",
    fields: [
      { key: "relayHost", labelKey: "remote.cfgRelayHost" },
      { key: "sshUser", labelKey: "remote.cfgSshUser" },
      { key: "sshKeyPath", labelKey: "remote.cfgSshKey" },
    ],
  },
  {
    titleKey: "settings.remote.dashboard",
    fields: [
      { key: "dashboardUrl", labelKey: "remote.cfgDashboardUrl" },
      { key: "dashboardAuth", labelKey: "remote.cfgDashboardAuth" },
    ],
  },
  {
    titleKey: "settings.remote.rustdesk",
    fields: [
      { key: "rustdeskDomain", labelKey: "remote.cfgRustdeskDomain" },
      { key: "rustdeskKey", labelKey: "remote.cfgRustdeskKey" },
    ],
  },
  {
    titleKey: "settings.remote.cloud",
    fields: [
      { key: "instanceId", labelKey: "remote.cfgInstanceId" },
      { key: "region", labelKey: "remote.cfgRegion" },
      { key: "pricePerGb", labelKey: "remote.cfgPrice", numeric: true },
    ],
  },
];

function toDraft(config: RcConfig): Draft {
  return Object.fromEntries(
    Object.entries(config).map(([key, value]) => [key, String(value)]),
  ) as Draft;
}

function toConfig(draft: Draft): RcConfig {
  return { ...draft, pricePerGb: Number(draft.pricePerGb) || 0 } as RcConfig;
}

export function RemoteSection() {
  const { t } = useI18n();
  const query = useRcConfig();

  if (query.isLoading) {
    return (
      <div className="flex items-center justify-center py-10">
        <Spinner />
      </div>
    );
  }

  if (query.isError || !query.data) {
    return (
      <div className="flex flex-col items-start gap-3 rounded-xl border border-border-subtle bg-card p-5">
        <p className="text-sm text-danger">
          {query.error ? errorMessage(query.error) : t("common.loadError")}
        </p>
        <Button
          type="button"
          variant="outline"
          size="sm"
          onClick={() => void query.refetch()}
        >
          {t("common.retry")}
        </Button>
      </div>
    );
  }

  return <RemoteForm config={query.data} />;
}

function RemoteForm({ config }: { config: RcConfig }) {
  const { t } = useI18n();
  const save = useRcSaveConfig();
  const [draft, setDraft] = useState<Draft>(() => toDraft(config));

  const commit = (next: Draft) => {
    save.mutate(toConfig(next), {
      onSuccess: () => toast.success(t("remote.cfgSaved")),
      onError: (error) =>
        toast.error(t("remote.cfgFailed"), errorMessage(error)),
    });
  };

  return (
    <div className={SETTINGS_GROUP_STACK_CLASS}>
      {GROUPS.map((group) => (
        <SettingsGroup key={group.titleKey} title={t(group.titleKey)}>
          {group.fields.map((field) => (
            <Field key={field.key} label={t(field.labelKey)}>
              <Input
                value={draft[field.key]}
                inputMode={field.numeric ? "decimal" : undefined}
                spellCheck={false}
                autoComplete="off"
                onChange={(event) =>
                  setDraft((current) => ({
                    ...current,
                    [field.key]: event.target.value,
                  }))
                }
                onBlur={() => {
                  if (
                    JSON.stringify(draft) !== JSON.stringify(toDraft(config))
                  ) {
                    commit(draft);
                  }
                }}
              />
            </Field>
          ))}
        </SettingsGroup>
      ))}
      <p className="text-xs text-muted-foreground">{t("remote.cfgHint")}</p>
    </div>
  );
}
