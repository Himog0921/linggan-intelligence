/**
 * 写操作的请求身份。服务器按这个编号去重：同一编号重复提交只生效一次，模型测试也只计费一次。
 *
 * 规则：同一份内容在「结果未确认」之后重试，沿用原编号；内容变了，或服务器已经给出明确答复，
 * 就换新编号。只在内存里保存，离开页面即丢弃。
 */
export class RequestIdentities {
  private readonly pending = new Map<string, { fingerprint: string; key: string }>();
  private readonly newKey: () => string;

  constructor(newKey: () => string = () => crypto.randomUUID()) {
    this.newKey = newKey;
  }

  keyFor(action: string, fingerprint: string): string {
    const current = this.pending.get(action);
    if (current && current.fingerprint === fingerprint) return current.key;
    const key = this.newKey();
    this.pending.set(action, { fingerprint, key });
    return key;
  }

  /** 服务器已明确答复（成功或拒绝）：下一次提交换新编号。 */
  settle(action: string): void {
    this.pending.delete(action);
  }
}
