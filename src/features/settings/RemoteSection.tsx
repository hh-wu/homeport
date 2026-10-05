import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { FolderOpen } from "lucide-react";

import { Button, Field, Input, Spinner, Textarea } from "@/components/ui";
import { useI18n, type TKey } from "@/i18n";
import { useRcConfig, useRcSaveConfig } from "@/features/remote-console/api";
import { errorMessage, toast } from "@/lib/toast";
import type { RcConfig } from "@/types/models";
import { SETTINGS_GROUP_STACK_CLASS, SettingsGroup } from "./SettingsGroup";

type Draft = Record<keyof RcConfig, string>;

type FieldSpec = {
  key: keyof RcConfig;
  labelKey: TKey;
  hintKey?: TKey;
  numeric?: boolean;
  picker?: { titleKey: TKey; extensions?: string[] };
};

const GROUPS: {
  titleKey: TKey;
  fields: FieldSpec[];
  longFields?: { key: keyof RcConfig; labelKey: TKey; hintKey: TKey }[];
}[] = [
  {
    titleKey: "settings.remote.frpc",
    fields: [
      {
        key: "frpcPath",
        labelKey: "remote.cfgFrpcPath",
        picker: { titleKey: "remote.pickFrpc", extensions: ["exe"] },
      },
      {
        key: "frpcConfigPath",
        labelKey: "remote.cfgFrpcConfig",
        picker: { titleKey: "remote.pickFrpcConfig", extensions: ["toml"] },
      },
      {
        key: "visitorConfigPath",
        labelKey: "remote.cfgVisitorConfig",
        hintKey: "remote.cfgVisitorConfigHint",
        picker: { titleKey: "remote.pickVisitorConfig", extensions: ["toml"] },
      },
      {
        key: "logPath",
        labelKey: "remote.cfgLogPath",
        picker: { titleKey: "remote.pickLog" },
      },
    ],
  },
  {
    titleKey: "settings.remote.relay",
    fields: [
      { key: "relayHost", labelKey: "remote.cfgRelayHost" },
      { key: "sshUser", labelKey: "remote.cfgSshUser" },
      {
        key: "sshKeyPath",
        labelKey: "remote.cfgSshKey",
        picker: { titleKey: "remote.pickSshKey" },
      },
    ],
  },
  {
    titleKey: "settings.remote.dashboard",
    fields: [
      { key: "dashboardUrl", labelKey: "remote.cfgDashboardUrl" },
      { key: "dashboardAuth", labelKey: "remote.cfgDashboardAuth" },
    ],
    longFields: [
      {
        key: "portMap",
        labelKey: "remote.cfgPortMap",
        hintKey: "remote.cfgPortMapHint",
      },
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

  const commitIfChanged = (next: Draft) => {
    if (JSON.stringify(next) !== JSON.stringify(toDraft(config))) {
      commit(next);
    }
  };

  const browse = async (field: FieldSpec) => {
    if (!field.picker) return;
    try {
      const selected = await open({
        title: t(field.picker.titleKey),
        multiple: false,
        filters: field.picker.extensions
          ? [
              {
                name: field.picker.extensions.join("/").toUpperCase(),
                extensions: field.picker.extensions,
              },
              { name: t("remote.filterAll"), extensions: ["*"] },
            ]
          : [{ name: t("remote.filterAll"), extensions: ["*"] }],
      });
      const path = Array.isArray(selected) ? selected[0] : selected;
      if (!path) return;
      const next = { ...draft, [field.key]: path };
      setDraft(next);
      commitIfChanged(next);
    } catch (error) {
      toast.error(t("remote.pickFailed"), errorMessage(error));
    }
  };

  return (
    <div className={SETTINGS_GROUP_STACK_CLASS}>
      {GROUPS.map((group) => (
        <SettingsGroup key={group.titleKey} title={t(group.titleKey)}>
          {group.fields.map((field) => (
            <Field
              key={field.key}
              label={t(field.labelKey)}
              hint={field.hintKey ? t(field.hintKey) : undefined}
            >
              {field.picker ? (
                <div className="flex items-center gap-2">
                  <Input
                    value={draft[field.key]}
                    spellCheck={false}
                    autoComplete="off"
                    className="flex-1"
                    onChange={(event) =>
                      setDraft((current) => ({
                        ...current,
                        [field.key]: event.target.value,
                      }))
                    }
                    onBlur={() => commitIfChanged(draft)}
                  />
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    className="shrink-0"
                    onClick={() => void browse(field)}
                  >
                    <FolderOpen />
                    {t("remote.browse")}
                  </Button>
                </div>
              ) : (
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
                  onBlur={() => commitIfChanged(draft)}
                />
              )}
            </Field>
          ))}
          {group.longFields?.map((field) => (
            <Field
              key={field.key}
              label={t(field.labelKey)}
              hint={t(field.hintKey)}
            >
              <Textarea
                value={draft[field.key]}
                rows={4}
                spellCheck={false}
                autoComplete="off"
                className="font-mono text-xs"
                onChange={(event) =>
                  setDraft((current) => ({
                    ...current,
                    [field.key]: event.target.value,
                  }))
                }
                onBlur={() => commitIfChanged(draft)}
              />
            </Field>
          ))}
        </SettingsGroup>
      ))}
      <p className="text-xs text-muted-foreground">{t("remote.cfgHint")}</p>
    </div>
  );
}
