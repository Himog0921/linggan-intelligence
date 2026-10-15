import { useEffect, useId, useRef, useState, type FormEvent } from 'react';
import { ApiError } from '../../shared/api';
import { RequestIdentities } from '../../shared/requestIdentity';
import { FeedbackLine } from './FeedbackLine';
import { probeModel, saveConnection, saveModel, type ModelConnection, type ModelEntry } from './modelSettingsApi';
import { CONNECTION_APIS, feedbackForError, feedbackForProbe, type Feedback } from './modelSettingsModel';
import styles from './ModelSettingsPage.module.css';

interface ConnectionDialogProps {
  /** null 表示添加新连接。 */
  connection: ModelConnection | null;
  /** 该连接当前版本下已登记的模型，用于预填模型 ID。 */
  model: ModelEntry | undefined;
  /** 每一步写入后都刷新读取结果；读取是否成功由页面另行显示。 */
  onWritten: () => void;
  /** 只保存不测试时，关闭对话框并把回执交给页面。 */
  onSavedWithoutTest: (feedback: Feedback) => void;
  onClose: () => void;
}

interface Fields {
  name: string;
  api: string;
  baseUrl: string;
  apiKey: string;
  modelId: string;
  localEndpoint: boolean;
}

/**
 * 本次对话框里已经被服务器完整接纳（连接 + 模型）的结果。之后再保存要以它的修订号为基础，
 * 否则必然版本冲突；输入没变时「保存并测试」只重试测试，不再建新版本。
 */
interface SavedVersion {
  versionRef: string;
  revision: number;
  modelRef: string;
  fieldsFingerprint: string;
}

export function ConnectionDialog({ connection, model, onWritten, onSavedWithoutTest, onClose }: ConnectionDialogProps) {
  const dialogRef = useRef<HTMLDialogElement>(null);
  const titleId = useId();
  // 关闭后把焦点还给打开它的按钮，键盘用户不会被丢回页面顶部。
  const [returnFocusTo] = useState(() => (document.activeElement instanceof HTMLElement ? document.activeElement : null));
  const [identities] = useState(() => new RequestIdentities());
  // 新连接的编号在打开对话框时确定一次，重试不会因此建出第二条连接。
  const [connectionRef] = useState(() => connection?.connectionRef ?? crypto.randomUUID());
  const [saved, setSaved] = useState<SavedVersion | null>(null);
  const [fields, setFields] = useState<Fields>({
    name: connection?.name ?? '',
    api: connection?.api ?? CONNECTION_APIS[0]?.value ?? '',
    baseUrl: connection?.baseUrl ?? '',
    apiKey: '',
    modelId: model?.modelId ?? '',
    localEndpoint: connection?.localEndpoint ?? false,
  });
  const [busy, setBusy] = useState(false);
  const [feedback, setFeedback] = useState<Feedback | null>(null);

  useEffect(() => {
    const dialog = dialogRef.current;
    if (dialog && !dialog.open) dialog.showModal();
    return () => {
      if (dialog?.open) dialog.close();
    };
  }, []);

  function update<K extends keyof Fields>(name: K, value: Fields[K]) {
    setFields(current => ({ ...current, [name]: value }));
  }

  function dismiss() {
    if (dialogRef.current?.open) dialogRef.current.close();
    returnFocusTo?.focus();
  }

  function requestClose() {
    if (busy) return;
    dismiss();
    onClose();
  }

  async function submit(withTest: boolean) {
    if (busy) return;
    const form = dialogRef.current?.querySelector('form');
    if (form && !form.reportValidity()) return;
    setBusy(true);
    setFeedback(null);
    const name = fields.name.trim();
    const baseUrl = fields.baseUrl.trim();
    const modelId = fields.modelId.trim();
    const fieldsFingerprint = JSON.stringify({ name, api: fields.api, baseUrl, apiKey: fields.apiKey, localEndpoint: fields.localEndpoint, modelId });
    let step: 'connection' | 'model' | 'probe' = 'connection';
    try {
      let target = saved !== null && saved.fieldsFingerprint === fieldsFingerprint ? saved : null;
      if (target === null) {
        const connectionPayload = {
          connectionRef,
          expectedRevision: saved?.revision ?? connection?.revision ?? 0,
          name,
          api: fields.api,
          baseUrl,
          apiKey: fields.apiKey,
          localEndpoint: fields.localEndpoint,
        };
        // 结果未确认后用同样的输入重试，沿用原编号，服务器按重放处理，不会多出一个版本。
        const versionRef = identities.keyFor('connection', JSON.stringify(connectionPayload));
        const version = await saveConnection({ versionRef, ...connectionPayload });
        step = 'model';
        // 模型条目属于某一个连接版本：同一版本、同一模型 ID 的重复登记由服务器合并为一条。
        const modelPayload = { connectionVersionRef: version.versionRef, modelId };
        const modelRef = identities.keyFor('model', JSON.stringify(modelPayload));
        const entry = await saveModel({ modelRef, ...modelPayload });
        identities.settle('connection');
        identities.settle('model');
        target = { versionRef: version.versionRef, revision: version.revision, modelRef: entry.modelRef, fieldsFingerprint };
        setSaved(target);
        onWritten();
      }
      if (!withTest) {
        dismiss();
        onSavedWithoutTest({ tone: 'success', text: '供应商连接已保存；尚未发送测试请求。', code: null });
        return;
      }
      step = 'probe';
      const probePayload = { connectionVersionRef: target.versionRef, modelRef: target.modelRef };
      const invocationRef = identities.keyFor('probe', JSON.stringify(probePayload));
      const outcome = await probeModel({ invocationRef, operation: 'probe', ...probePayload });
      identities.settle('probe');
      setFeedback(feedbackForProbe(outcome));
      onWritten();
    } catch (error) {
      if (error instanceof ApiError && error.answered) identities.settle(step);
      setFeedback(feedbackForError(error));
    } finally {
      setBusy(false);
    }
  }

  function onSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    void submit(false);
  }

  const editing = connection !== null;
  // 由其他入口建立的连接（例如本机 WeMM 运行时）可能用不在列表里的协议：如实显示，不悄悄改成第一项。
  const apiOptions = CONNECTION_APIS.some(option => option.value === fields.api)
    ? CONNECTION_APIS
    : [...CONNECTION_APIS, { value: fields.api, label: fields.api }];
  return (
    <dialog
      ref={dialogRef}
      className={styles.dialog}
      aria-labelledby={titleId}
      onCancel={event => {
        event.preventDefault();
        requestClose();
      }}
    >
      <form className={styles.dialogForm} onSubmit={onSubmit}>
        <header className={styles.sectionHeading}>
          <h2 id={titleId}>{editing ? '编辑供应商连接' : '添加供应商连接'}</h2>
          <button type="button" onClick={requestClose} disabled={busy}>
            关闭
          </button>
        </header>
        <div className={styles.dialogFields}>
          <label>
            连接名称
            <input name="name" required maxLength={100} value={fields.name} onChange={event => update('name', event.target.value)} />
          </label>
          <label>
            接口协议
            <select name="api" value={fields.api} onChange={event => update('api', event.target.value)}>
              {apiOptions.map(option => (
                <option key={option.value} value={option.value}>
                  {option.label}
                </option>
              ))}
            </select>
          </label>
          <label>
            API 地址
            <input name="baseUrl" type="url" required maxLength={1000} value={fields.baseUrl} onChange={event => update('baseUrl', event.target.value)} />
          </label>
          <label>
            {editing ? 'API Key（留空沿用）' : 'API Key'}
            <input
              name="apiKey"
              type="password"
              autoComplete="new-password"
              maxLength={4096}
              value={fields.apiKey}
              onChange={event => update('apiKey', event.target.value)}
            />
          </label>
          <label>
            模型 ID
            <input name="modelId" required maxLength={200} value={fields.modelId} onChange={event => update('modelId', event.target.value)} />
          </label>
          <label className={styles.check}>
            <input name="localEndpoint" type="checkbox" checked={fields.localEndpoint} onChange={event => update('localEndpoint', event.target.checked)} />
            本机回环服务（允许 HTTP）
          </label>
          <p className={styles.note}>保存仅写入本机设置。点击测试才发送合成 V1 JSON 请求，可能产生少量费用。</p>
        </div>
        <FeedbackLine feedback={feedback} />
        <footer className={`${styles.actions} ${styles.dialogActions}`}>
          <button type="button" disabled={busy} onClick={() => void submit(true)}>
            {busy ? '处理中…' : '保存并测试'}
          </button>
          <button type="submit" disabled={busy}>
            {busy ? '处理中…' : '保存'}
          </button>
        </footer>
      </form>
    </dialog>
  );
}
