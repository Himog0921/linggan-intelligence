import styles from './Assets.module.css';

export function TemplateLabelFields({ name, tags, onName, onTags, disabled }: {
  name: string;
  tags: string;
  onName: (value: string) => void;
  onTags: (value: string) => void;
  disabled: boolean;
}) {
  return <div className={styles.templateLabels}>
    <label>样板名称
      <input value={name} maxLength={120} disabled={disabled} onChange={event => onName(event.target.value)} />
    </label>
    <label>样板标签
      <input value={tags} maxLength={400} disabled={disabled} onChange={event => onTags(event.target.value)} />
    </label>
    <p className={styles.hint}>可选。名称最多 60 字。最多 8 个标签，每个不超过 24 字，用逗号分隔。请用自己的话命名和标记。</p>
  </div>;
}
