import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import { api, type IntegrationsOverview, type IntegrationToolStatus, type ThreadMigrationStatus } from "../../shared/api";
import { PageContent } from "../../shell/layout/PageContent";
import { Button } from "../../shared/ui/Button";
import { FormField } from "../../shared/ui/FormControls";
import { Select } from "../../shared/ui/Select";
import { TitledCard } from "../../shared/ui/TitledCard";
import { useMessage } from "../../shared/ui/message";
import styles from "./IntegrationsPage.module.scss";

type ModelOption = { id: string; name: string };

export function IntegrationsPage() {
  const message = useMessage();
  const navigate = useNavigate();
  const [overview, setOverview] = useState<IntegrationsOverview | null>(null);
  const [selected, setSelected] = useState<Record<string, string>>({});
  const [busy, setBusy] = useState<string | null>(null);

  const refresh = async () => {
    try {
      const next = await api.integrations();
      setOverview(next);
      setSelected((current) => {
        const merged = { ...current };
        for (const tool of [next.codex, next.claude, next.dsh]) {
          if (!merged[tool.tool]) merged[tool.tool] = tool.model || next.models[0]?.id || "";
        }
        return merged;
      });
    } catch (cause) {
      message(cause instanceof Error ? cause.message : String(cause));
    }
  };

  useEffect(() => {
    void refresh();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const apply = async (tool: IntegrationToolStatus) => {
    const model = selected[tool.tool];
    if (!model) return;
    try {
      setBusy(tool.tool);
      await api.applyIntegration(tool.tool, model);
      message(t("已接管 {name}，重启对应工具后生效", { name: tool.label }));
      await refresh();
    } catch (cause) {
      message(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(null);
    }
  };

  const restore = async (tool: IntegrationToolStatus) => {
    try {
      setBusy(tool.tool);
      await api.restoreIntegration(tool.tool);
      message(t("已恢复 {name} 的原始配置", { name: tool.label }));
      await refresh();
    } catch (cause) {
      message(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(null);
    }
  };

  const migrateThreads = async () => {
    try {
      setBusy("codex-threads");
      const result = await api.migrateCodexThreads();
      message(t("已把 {count} 个历史会话迁移到当前供应商", { count: result.migrated_threads }));
      await refresh();
    } catch (cause) {
      message(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(null);
    }
  };
  const restoreThreads = async () => {
    try {
      setBusy("codex-threads");
      const result = await api.restoreCodexThreads();
      message(t("已还原 {count} 个历史会话的供应商归属", { count: result.migrated_threads }));
      await refresh();
    } catch (cause) {
      message(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(null);
    }
  };
  if (!overview) {
    return <PageContent title={t("工具接入")} sections={[{
      key: "loading",
      estimatedHeight: 240,
      content: <div className={styles.loading}>{t("正在读取本机工具配置…")}</div>,
    }]} />;
  }

  const ready = overview.external_api_enabled && overview.has_api_key;
  const content = <div className={styles.stack}>
    {!ready && <div className={styles.banner}>
      <div className={styles.bannerText}>
        <strong>{t("外部 API 尚未就绪")}</strong>
        <small>{t("接管工具前需要先开启外部 API 并设置密钥，模型请求会通过它转发。")}</small>
      </div>
      <Button size="small" variant="primary" onClick={() => navigate("/settings")}>
        {t("打开系统设置")}
      </Button>
    </div>}

    <ToolCard status={overview.codex} models={overview.models}
      value={selected[overview.codex.tool] ?? ""}
      busy={busy === overview.codex.tool}
      disabled={!ready}
      onSelect={(value) => setSelected((current) => ({ ...current, [overview.codex.tool]: value }))}
      onApply={() => void apply(overview.codex)}
      onRestore={() => void restore(overview.codex)}
      migration={overview.codex_threads}
      migrationBusy={busy === "codex-threads"}
      onMigrateThreads={() => void migrateThreads()}
      onRestoreThreads={() => void restoreThreads()} />

    <ToolCard status={overview.claude} models={overview.models}
      value={selected[overview.claude.tool] ?? ""}
      busy={busy === overview.claude.tool}
      disabled={!ready}
      onSelect={(value) => setSelected((current) => ({ ...current, [overview.claude.tool]: value }))}
      onApply={() => void apply(overview.claude)}
      onRestore={() => void restore(overview.claude)} />

    <ToolCard status={overview.dsh} models={overview.models}
      value={selected[overview.dsh.tool] ?? ""}
      busy={busy === overview.dsh.tool}
      disabled={!ready}
      onSelect={(value) => setSelected((current) => ({ ...current, [overview.dsh.tool]: value }))}
      onApply={() => void apply(overview.dsh)}
      onRestore={() => void restore(overview.dsh)} />

    <TitledCard title={t("在其他工具中使用")}>
      <div className={styles.addressContent}>
        <div className={styles.addressRow}>
          <span>{t("OpenAI 兼容地址")}</span>
          <code>{overview.base_url}/v1</code>
        </div>
        <div className={styles.addressRow}>
          <span>{t("Anthropic 兼容地址")}</span>
          <code>{overview.base_url}</code>
        </div>
        <div className={styles.addressRow}>
          <span>{t("可代理模型")}</span>
          <span>{t("共 {count} 个（含插件模型）", { count: overview.models.length })}</span>
        </div>
        <small>{t("密钥、开关与更多参数在「系统设置 → 外部 API」中管理。")}</small>
      </div>
    </TitledCard>
  </div>;

  return <PageContent title={t("工具接入")} sections={[{
    key: "integrations",
    estimatedHeight: 1020,
    content,
  }]} />;
}

function ToolCard({ status, models, value, busy, disabled, onSelect, onApply, onRestore, migration, migrationBusy, onMigrateThreads, onRestoreThreads }: {
  status: IntegrationToolStatus;
  models: ModelOption[];
  value: string;
  busy: boolean;
  disabled: boolean;
  onSelect: (value: string) => void;
  onApply: () => void;
  onRestore: () => void;
  migration?: ThreadMigrationStatus;
  migrationBusy?: boolean;
  onMigrateThreads?: () => void;
  onRestoreThreads?: () => void;
}) {
  const options = models.map((model) => ({
    value: model.id,
    label: model.name && model.name !== model.id ? `${model.name} · ${model.id}` : model.id,
  }));
  const current = status.current_model
    ? status.current_provider
      ? `${status.current_model} · ${status.current_provider}`
      : status.current_model
    : t("未读取到");
  return <TitledCard title={status.label} action={
    <div className={styles.cardActions}>
      <Button size="small" variant="secondary" disabled={busy || !status.backup_available || disabled}
        onClick={onRestore}>
        {t("恢复原始配置")}
      </Button>
      <Button size="small" variant="primary" disabled={busy || !value || disabled}
        onClick={onApply}>
        {busy ? t("处理中…") : t("接管配置")}
      </Button>
    </div>
  }>
    <div className={styles.cardContent}>
      <div className={styles.statusRow}>
        <span className={status.applied ? styles.applied : styles.idle}>
          {status.applied ? t("已接管") : t("未接管")}
        </span>
        {!status.installed && <span className={styles.muted}>{t("未检测到该工具")}</span>}
        {status.applied && status.model && <code className={styles.model}>{status.model}</code>}
      </div>
      <div className={styles.metaRow}>
        <span>{t("配置文件")}</span>
        <code>{status.config_path}</code>
      </div>
      <div className={styles.metaRow}>
        <span>{t("当前配置")}</span>
        <span>{current}</span>
      </div>
      {migration && <>
        <div className={styles.metaRow}>
          <span>{t("历史会话归属")}</span>
          <span>{migration.active
            ? t("已迁移 {count} 个会话到当前供应商", { count: migration.migrated_threads })
            : t("未迁移，旧会话仍归属原供应商")}</span>
        </div>
        <div className={styles.cardActions}>
          <Button size="small" variant="secondary"
            disabled={migrationBusy || disabled || !migration.active}
            onClick={onRestoreThreads}>
            {t("还原会话归属")}
          </Button>
          <Button size="small" variant="secondary"
            disabled={migrationBusy || disabled || migration.active || !status.applied}
            onClick={onMigrateThreads}>
            {migrationBusy ? t("处理中…") : t("迁移历史会话")}
          </Button>
        </div>
        <small className={styles.muted}>
          {t("旧会话记录了创建时的供应商，接管后可能出现「模型提供商不匹配」，迁移后可继续使用原会话。")}
        </small>
      </>}
      <FormField label={t("接管后的模型")} hint={t("可选择本项目已配置的模型，以及插件提供的模型额度。")}>
        <Select value={value} options={options} ariaLabel={t("接管后的模型")}
          disabled={busy || disabled || options.length === 0} onChange={onSelect} />
      </FormField>
      {status.notes.length > 0 && <ul className={styles.notes}>
        {status.notes.map((note) => <li key={note}>{note}</li>)}
      </ul>}
    </div>
  </TitledCard>;
}
