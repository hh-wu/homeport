import { useState } from "react";
import { Copy, Plus, Trash2 } from "lucide-react";

import {
  Button,
  ErrorState,
  Field,
  Input,
  LoadingState,
  Select,
  SwitchField,
  Tooltip,
} from "@/components/ui";
import { useI18n } from "@/i18n";
import { errorMessage, toast } from "@/lib/toast";
import type { McpPolicy, McpStateView } from "@/types/models";
import { DraftInput } from "@/features/settings/DraftInput";
import {
  useGenerateMcpToken,
  useMcpState,
  useMcpTokens,
  useRevokeMcpToken,
  useSetMcpConfig,
} from "./api";
import {
  SettingsGroup,
  SETTINGS_GROUP_STACK_CLASS,
} from "@/features/settings/SettingsGroup";

const POLICIES: McpPolicy[] = [
  "confirmAll",
  "confirmWrite",
  "confirmDangerous",
  "bypass",
];

export function McpSection() {
  const { t } = useI18n();
  const { data: state, isLoading, isError, refetch } = useMcpState();

  if (isLoading) return <LoadingState label={t("common.loading")} />;
  if (isError || !state) {
    return (
      <ErrorState
        title={t("common.loadError")}
        retryLabel={t("common.retry")}
        onRetry={() => void refetch()}
      />
    );
  }
  return <McpForm state={state} />;
}

function McpForm({ state }: { state: McpStateView }) {
  const { t } = useI18n();
  const setConfig = useSetMcpConfig();

  const save = (
    patch: Partial<{ enabled: boolean; port: number; policy: McpPolicy }>,
  ) => {
    setConfig.mutate(
      {
        enabled: state.enabled,
        port: state.port,
        policy: state.policy,
        ...patch,
      },
      {
        onError: (error) =>
          toast.error(t("settings.mcp.saveError"), errorMessage(error)),
      },
    );
  };

  const commitPort = (raw: string) => {
    const port = Number(raw);
    if (!Number.isInteger(port) || port < 1024 || port > 65535) {
      toast.error(t("settings.mcp.portInvalid"));
      return;
    }
    if (port !== state.port) save({ port });
  };

  return (
    <div className={SETTINGS_GROUP_STACK_CLASS}>
      <SettingsGroup
        title={t("settings.mcp.serverTitle")}
        description={t("settings.mcp.serverDescription")}
      >
        <div className="ui-content-stack">
          <SwitchField
            fieldLabel={t("settings.mcp.enableLabel")}
            label={t("settings.mcp.enable")}
            hint={t("settings.mcp.enableHint")}
            checked={state.enabled}
            onCheckedChange={(enabled) => save({ enabled })}
          />
          <Field
            label={t("settings.mcp.portLabel")}
            hint={t("settings.mcp.portHint")}
          >
            <DraftInput
              value={String(state.port)}
              onCommit={commitPort}
              maxLength={5}
              inputMode="numeric"
              autoComplete="off"
              spellCheck={false}
            />
          </Field>
          <Field
            label={t("settings.mcp.policyLabel")}
            hint={t("settings.mcp.policyHint")}
          >
            <Select
              value={state.policy}
              onValueChange={(policy) => save({ policy: policy as McpPolicy })}
              options={POLICIES.map((policy) => ({
                value: policy,
                label: t(`settings.mcp.policy.${policy}`),
              }))}
            />
          </Field>
          <p className="text-xs text-muted-foreground">
            {state.running && state.listeningPort
              ? t("settings.mcp.statusRunning", { port: state.listeningPort })
              : t("settings.mcp.statusStopped")}
          </p>
          {state.running && state.listeningPort && (
            <EndpointHint port={state.listeningPort} />
          )}
        </div>
      </SettingsGroup>

      <TokenGroup />
    </div>
  );
}

function EndpointHint({ port }: { port: number }) {
  const { t } = useI18n();
  const url = `http://127.0.0.1:${port}/mcp`;
  return (
    <div className="ui-surface-card flex items-center gap-2 px-3 py-2">
      <code className="min-w-0 flex-1 truncate font-mono text-xs">{url}</code>
      <Tooltip content={t("settings.mcp.copyEndpoint")}>
        <Button
          size="icon"
          variant="ghost"
          aria-label={t("settings.mcp.copyEndpoint")}
          onClick={() => void navigator.clipboard.writeText(url)}
        >
          <Copy className="size-3.5" />
        </Button>
      </Tooltip>
    </div>
  );
}

function TokenGroup() {
  const { t } = useI18n();
  const { data: tokens } = useMcpTokens();
  const generate = useGenerateMcpToken();
  const revoke = useRevokeMcpToken();
  const [name, setName] = useState("");
  const [issued, setIssued] = useState<{ name: string; token: string } | null>(
    null,
  );

  const submit = () => {
    const trimmed = name.trim();
    if (!trimmed) return;
    generate.mutate(trimmed, {
      onSuccess: (token) => {
        setIssued({ name: trimmed, token });
        setName("");
      },
      onError: (error) =>
        toast.error(t("settings.mcp.saveError"), errorMessage(error)),
    });
  };

  return (
    <SettingsGroup
      title={t("settings.mcp.tokensTitle")}
      description={t("settings.mcp.tokensDescription")}
    >
      <div className="ui-content-stack">
        {issued && (
          <div className="ui-surface-card flex flex-col gap-2 px-3 py-2">
            <p className="text-xs text-muted-foreground">
              {t("settings.mcp.tokenCreatedHint", { name: issued.name })}
            </p>
            <div className="flex items-center gap-2">
              <code className="min-w-0 flex-1 truncate font-mono text-xs">
                {issued.token}
              </code>
              <Tooltip content={t("settings.mcp.copyToken")}>
                <Button
                  size="icon"
                  variant="ghost"
                  aria-label={t("settings.mcp.copyToken")}
                  onClick={() =>
                    void navigator.clipboard.writeText(issued.token)
                  }
                >
                  <Copy className="size-3.5" />
                </Button>
              </Tooltip>
            </div>
          </div>
        )}

        <div className="flex items-center gap-2">
          <Input
            value={name}
            placeholder={t("settings.mcp.tokenNamePlaceholder")}
            maxLength={64}
            autoComplete="off"
            spellCheck={false}
            onChange={(event) => setName(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter") submit();
            }}
          />
          <Button
            variant="secondary"
            disabled={!name.trim() || generate.isPending}
            onClick={submit}
          >
            <Plus className="size-3.5" />
            {t("settings.mcp.generateToken")}
          </Button>
        </div>

        {tokens && tokens.length > 0 && (
          <div className="ui-surface-card overflow-hidden">
            {tokens.map((token) => (
              <div
                key={token.name}
                className="flex min-h-8 items-center gap-2 px-3 py-1"
              >
                <span className="min-w-0 flex-1 truncate text-xs font-medium">
                  {token.name}
                </span>
                <span className="shrink-0 text-2xs text-muted-foreground">
                  {token.createdAt.slice(0, 10)}
                </span>
                <Tooltip content={t("settings.mcp.revokeToken")}>
                  <Button
                    size="icon"
                    variant="ghost"
                    aria-label={t("settings.mcp.revokeToken")}
                    onClick={() =>
                      revoke.mutate(token.name, {
                        onError: (error) =>
                          toast.error(
                            t("settings.mcp.saveError"),
                            errorMessage(error),
                          ),
                      })
                    }
                  >
                    <Trash2 className="size-3.5" />
                  </Button>
                </Tooltip>
              </div>
            ))}
          </div>
        )}
      </div>
    </SettingsGroup>
  );
}
