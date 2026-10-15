import { useQuery, useQueryClient } from '@tanstack/react-query';
import { useState, type FormEvent } from 'react';
import { AppShell } from '../../app/AppShell';
import { ApiError } from '../../shared/api';
import { countOrUnknown, humanMoment } from '../../shared/formatting';
import { RequestIdentities } from '../../shared/requestIdentity';
import { ConnectionDialog } from './ConnectionDialog';
import { FeedbackLine } from './FeedbackLine';
import {
  fetchModelSettings,
  probeModel,
  saveConfig,
  setConnectionEnabled,
  type Invocation,
  type ModelConfig,
  type ModelConnection,
  type ModelEntry,
  type ModelSettings,
} from './modelSettingsApi';
import {
  CONFIG_DEFAULTS,
  CONFIG_LIMITS,
  CONNECTION_APIS,
  feedbackForError,
  feedbackForProbe,
  isCallable,
  modelStateLabel,
  type Feedback,
} from './modelSettingsModel';
import styles from './ModelSettingsPage.module.css';

const QUERY_KEY = ['model-settings'] as const;

const OPERATION_LABEL: Record<string, string> = {
  connect: '连接测试',
  discover: '模型发现',
  probe: '能力测试',
  analyze: '研究分析',
  embed: '向量生成',
};
const INVOCATION_STATE_LABEL: Record<string, string> = {
  running: '进行中',
  succeeded: '成功',
  failed: '失败',
};

function apiLabel(api: string): string {
  return CONNECTION_APIS.find(option => option.value === api)?.label ?? api;
}

type DialogState = { connection: ModelConnection | null; model: ModelEntry | undefined } | null;

export function ModelSettingsPage() {
  const queryClient = useQueryClient();
  const settings = useQuery({ queryKey: QUERY_KEY, queryFn: ({ signal }) => fetchModelSettings(signal) });
  const [identities] = useState(() => new RequestIdentities());
  const [feedback, setFeedback] = useState<Feedback | null>(null);
  const [busyAction, setBusyAction] = useState<string | null>(null);
  const [dialog, setDialog] = useState<DialogState>(null);

  function reload() {
    void queryClient.invalidateQueries({ queryKey: QUERY_KEY });
  }

  /** 写操作：同一时间只进行一个；只在服务器回执后报告结果，读取刷新失败另行显示。 */
  async function runAction(action: string, perform: () => Promise<Feedback>) {
    if (busyAction !== null) return;
    setBusyAction(action);
    setFeedback(null);
    try {
      const result = await perform();
      identities.settle(action);
      setFeedback(result);
      reload();
    } catch (error) {
      if (error instanceof ApiError && error.answered) identities.settle(action);
      setFeedback(feedbackForError(error));
    } finally {
      setBusyAction(null);
    }
  }

  function toggleConnection(connection: ModelConnection) {
    void runAction(`toggle:${connection.connectionRef}`, async () => {
      await setConnectionEnabled({
        connectionRef: connection.connectionRef,
        expectedRevision: connection.revision,
        enabled: !connection.enabled,
      });
      return { tone: 'success', text: connection.enabled ? '连接已停用。' : '连接已启用。', code: null };
    });
  }

  function testModel(model: ModelEntry) {
    const action = `probe:${model.modelRef}`;
    void runAction(action, async () => {
      const payload = { connectionVersionRef: model.connectionVersionRef, modelRef: model.modelRef };
      const invocationRef = identities.keyFor(action, JSON.stringify(payload));
      return feedbackForProbe(await probeModel({ invocationRef, operation: 'probe', ...payload }));
    });
  }

  const data = settings.data;
  return (
    <AppShell
      active={null}
      boundaryLabel="本机服务状态"
      crumb={
        <>
          设置 <span className="v7-slash">/</span> <b>模型与 AI</b>
        </>
      }
      meta={<span>本机工作空间</span>}
    >
      <div className={`v7-shell ${styles.shell}`}>
        <main className={styles.page} aria-labelledby="model-settings-title">
          <title>模型与 AI · Linggan Intelligence</title>
          <header className={styles.toolbar}>
            <div>
              <h1 id="model-settings-title">评论研究的模型设置</h1>
              <p>保存连接或默认配置不会发送评论。只有手动测试和研究批次的受控执行会发起模型调用。</p>
            </div>
            <button type="button" onClick={reload} disabled={settings.isFetching}>
              {settings.isFetching ? '正在读取…' : '刷新'}
            </button>
          </header>
          <FeedbackLine feedback={feedback} id="settings-feedback" />
          {settings.isError ? <ReadFailure error={settings.error} hasData={data !== undefined} /> : null}
          {data === undefined ? (
            settings.isPending ? (
              <p className={styles.note} role="status">
                正在读取模型设置…
              </p>
            ) : null
          ) : (
            <div className={styles.surface}>
              <ConnectionsSection
                data={data}
                busyAction={busyAction}
                onAdd={() => setDialog({ connection: null, model: undefined })}
                onEdit={connection =>
                  setDialog({
                    connection,
                    model: data.models.find(model => model.connectionVersionRef === connection.versionRef),
                  })
                }
                onToggle={toggleConnection}
                onTest={testModel}
              />
              <DefaultConfigSection
                key={data.config?.configRef ?? 'no-config'}
                data={data}
                busy={busyAction !== null}
                onSave={config => {
                  const action = 'config';
                  void runAction(action, async () => {
                    const payload = { expectedConfigRef: data.config?.configRef ?? null, ...config };
                    const configRef = identities.keyFor(action, JSON.stringify(payload));
                    await saveConfig({ configRef, ...payload });
                    return { tone: 'success', text: '默认研究模型已保存；没有启动评论研究。', code: null };
                  });
                }}
              />
              <WorkerSection data={data} />
            </div>
          )}
        </main>
      </div>
      {dialog ? (
        <ConnectionDialog
          connection={dialog.connection}
          model={dialog.model}
          onWritten={reload}
          onSavedWithoutTest={result => {
            setDialog(null);
            setFeedback(result);
          }}
          onClose={() => setDialog(null)}
        />
      ) : null}
    </AppShell>
  );
}

function ReadFailure({ error, hasData }: { error: Error; hasData: boolean }) {
  const detail = feedbackForError(error);
  return (
    <p role="alert" className={`${styles.feedback} ${styles.error}`}>
      <b className={styles.feedbackTone}>读取失败</b>
      <span>
        {hasData ? '最新读取没有成功，下方仍是上一次读取到的内容。' : '模型设置没有读取成功。'}
        {detail.text}
      </span>
      {detail.code ? <code className={styles.code}>{detail.code}</code> : null}
    </p>
  );
}

interface ConnectionsSectionProps {
  data: ModelSettings;
  busyAction: string | null;
  onAdd: () => void;
  onEdit: (connection: ModelConnection) => void;
  onToggle: (connection: ModelConnection) => void;
  onTest: (model: ModelEntry) => void;
}

function ConnectionsSection({ data, busyAction, onAdd, onEdit, onToggle, onTest }: ConnectionsSectionProps) {
  const busy = busyAction !== null;
  return (
    <section aria-labelledby="connections-title">
      <div className={styles.sectionHeading}>
        <h2 id="connections-title">供应商连接</h2>
        <button type="button" onClick={onAdd} disabled={busy}>
          添加连接
        </button>
      </div>
      <p>连接只保存到本机工作空间。测试会发送一条合成 V1 JSON 请求，不会发送评论。</p>
      {data.connections.length === 0 ? (
        <p className={styles.note}>还没有供应商连接。</p>
      ) : (
        <div className={styles.tableWrap}>
          <table>
            <thead>
              <tr>
                <th scope="col">连接</th>
                <th scope="col">模型</th>
                <th scope="col">状态</th>
              </tr>
            </thead>
            <tbody>
              {data.connections.map(connection => {
                const models = data.models.filter(model => model.connectionVersionRef === connection.versionRef);
                return (
                  <tr key={connection.connectionRef}>
                    <td>
                      <strong>{connection.name}</strong>
                      <p className={styles.note}>
                        {apiLabel(connection.api)} · {connection.baseUrl}
                      </p>
                      <button type="button" onClick={() => onEdit(connection)} disabled={busy}>
                        编辑
                      </button>
                      <button type="button" onClick={() => onToggle(connection)} disabled={busy}>
                        {busyAction === `toggle:${connection.connectionRef}` ? '处理中…' : connection.enabled ? '停用' : '启用'}
                      </button>
                    </td>
                    <td>
                      {models.length === 0 ? (
                        <p className={styles.note}>尚未添加模型 ID。</p>
                      ) : (
                        models.map(model => (
                          <p key={model.modelRef}>
                            <strong>{model.modelId}</strong> · {modelStateLabel(model)}
                            <button type="button" onClick={() => onTest(model)} disabled={busy}>
                              {busyAction === `probe:${model.modelRef}` ? '测试中…' : '测试'}
                            </button>
                          </p>
                        ))
                      )}
                    </td>
                    <td>
                      {connection.enabled ? '已启用' : '已停用'}
                      {connection.test?.failureCode ? (
                        <p className={styles.note}>
                          最近一次连接测试未通过 <code className={styles.code}>{connection.test.failureCode}</code>
                        </p>
                      ) : null}
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      )}
    </section>
  );
}

type ConfigValues = Omit<ModelConfig, 'configRef'>;
type NumericField = Exclude<keyof ConfigValues, 'modelRef'>;
/** 输入框里是文字：清空时保持空字符串交给表单校验，不把 NaN 写进状态。提交时才换算成数字。 */
type ConfigForm = { modelRef: string } & Record<NumericField, string>;
const NUMERIC_FIELDS: readonly NumericField[] = ['inputTokenLimit', 'outputTokenLimit', 'timeoutSeconds', 'maxAttempts'];

interface DefaultConfigSectionProps {
  data: ModelSettings;
  busy: boolean;
  /** 只在所选模型已通过 V1 JSON 测试时调用；服务器仍会再校验一次。 */
  onSave: (config: ConfigValues) => void;
}

function DefaultConfigSection({ data, busy, onSave }: DefaultConfigSectionProps) {
  const config = data.config;
  const [form, setForm] = useState<ConfigForm>(() => ({
    modelRef: config?.modelRef ?? '',
    inputTokenLimit: String(config?.inputTokenLimit ?? CONFIG_DEFAULTS.inputTokenLimit),
    outputTokenLimit: String(config?.outputTokenLimit ?? CONFIG_DEFAULTS.outputTokenLimit),
    timeoutSeconds: String(config?.timeoutSeconds ?? CONFIG_DEFAULTS.timeoutSeconds),
    maxAttempts: String(config?.maxAttempts ?? CONFIG_DEFAULTS.maxAttempts),
  }));
  const [localError, setLocalError] = useState<Feedback | null>(null);
  const savedModel = config ? data.models.find(model => model.modelRef === config.modelRef) : undefined;
  const guidance = config
    ? `默认研究模型已保存：${savedModel?.modelId ?? '模型记录不可用'}。保存不会启动研究。`
    : '尚未保存默认研究模型。先添加并测试连接。';

  function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!event.currentTarget.reportValidity()) return;
    const model = data.models.find(item => item.modelRef === form.modelRef);
    if (!isCallable(model)) {
      setLocalError({ tone: 'error', text: '请选择已通过 V1 JSON 测试的模型。', code: null });
      return;
    }
    setLocalError(null);
    const numbers = Object.fromEntries(NUMERIC_FIELDS.map(name => [name, Number(form[name])])) as Record<NumericField, number>;
    onSave({ modelRef: form.modelRef, ...numbers });
  }

  const numberField = (name: NumericField, label: string) => (
    <label>
      {label}
      <input
        name={name}
        type="number"
        required
        min={CONFIG_LIMITS[name].min}
        max={CONFIG_LIMITS[name].max}
        value={form[name]}
        onChange={event => setForm(current => ({ ...current, [name]: event.target.value }))}
      />
    </label>
  );

  return (
    <section aria-labelledby="default-config-title">
      <h2 id="default-config-title">研究模型与调用上限</h2>
      <p className={styles.note} role="status">
        {guidance}
      </p>
      <form onSubmit={submit}>
        <label>
          研究模型
          <select
            name="modelRef"
            required
            value={form.modelRef}
            onChange={event => setForm(current => ({ ...current, modelRef: event.target.value }))}
          >
            <option value="">选择已测试的研究模型</option>
            {data.models.map(model => (
              <option key={model.modelRef} value={model.modelRef} disabled={!isCallable(model)}>
                {`${model.connectionName} / ${model.modelId} · ${modelStateLabel(model)}`}
              </option>
            ))}
          </select>
        </label>
        <div className={styles.fields}>
          {numberField('inputTokenLimit', '输入上限（Token）')}
          {numberField('outputTokenLimit', '输出上限（Token）')}
          {numberField('timeoutSeconds', '单次等待（秒）')}
          {numberField('maxAttempts', '最多尝试次数')}
        </div>
        <p className={styles.note}>
          这些限制约束每一次 V1 语义提取或归并调用；评论研究页保存的单轮额度还会覆盖该轮的向量候选调用。
        </p>
        <FeedbackLine feedback={localError} />
        <button type="submit" disabled={busy}>
          保存默认配置
        </button>
      </form>
    </section>
  );
}

function WorkerSection({ data }: { data: ModelSettings }) {
  const worker = data.worker;
  const state = worker.recent
    ? `执行器最近活动：${humanMoment(worker.lastSeenAt)}。`
    : `执行器未在最近 90 秒报告活动。${worker.lastError ? `上次错误：${worker.lastError}` : ''}`;
  return (
    <section aria-labelledby="worker-title">
      <h2 id="worker-title">后台执行器与调用记录</h2>
      <p role="status">{state}</p>
      {data.invocations.length === 0 ? (
        <p className={styles.note}>还没有调用记录。</p>
      ) : (
        <div className={styles.tableWrap}>
          <table>
            <thead>
              <tr>
                <th scope="col">时间</th>
                <th scope="col">调用</th>
                <th scope="col">状态与使用量</th>
              </tr>
            </thead>
            <tbody>
              {data.invocations.map(row => (
                <InvocationRow key={row.invocationRef} row={row} />
              ))}
            </tbody>
          </table>
        </div>
      )}
      <p className={styles.note}>调用记录保留使用量和安全传输诊断，不保存评论正文或模型原始输出。</p>
    </section>
  );
}

function InvocationRow({ row }: { row: Invocation }) {
  return (
    <tr>
      <td>{humanMoment(row.createdAt)}</td>
      <td>
        {OPERATION_LABEL[row.operation] ?? row.operation}
        {row.modelId ? ` · ${row.modelId}` : ''}
      </td>
      <td>
        {INVOCATION_STATE_LABEL[row.state] ?? row.state}
        {row.failureCode ? (
          <p className={styles.note}>
            失败原因 <code className={styles.code}>{row.failureCode}</code>
          </p>
        ) : null}
        <p className={styles.note}>
          输入 {countOrUnknown(row.inputTokens)} · 输出 {countOrUnknown(row.outputTokens)} · 计入 {row.chargedTokens} Token
        </p>
      </td>
    </tr>
  );
}
