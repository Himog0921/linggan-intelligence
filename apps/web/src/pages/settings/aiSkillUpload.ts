import type { AiSettings } from './aiSettingsModel';

/** Read only the explicit browser folder selection. The server repeats all package checks. */
export async function readSkillFolder(files: File[], limits: AiSettings['resources']) {
  if (!files.length || files.length > limits.maxSkillFiles) throw new Error(`请选择包含 SKILL.md 的目录，最多 ${limits.maxSkillFiles} 个文件。`);
  const directory = files[0]?.webkitRelativePath.split('/')[0]?.normalize('NFC');
  if (!directory) throw new Error('请使用“选择做法目录”，单个文件不能作为完整做法包导入。');
  if (!/^[a-z0-9]+(?:-[a-z0-9]+)*$/.test(directory) || directory.length > 64) throw new Error('做法目录名只能使用小写字母、数字和单个连字符，最多 64 个字符。');
  let total = 0;
  const paths = new Set<string>();
  const uploads = [];
  for (const file of files) {
    const [root, ...parts] = file.webkitRelativePath.split('/');
    const path = parts.join('/').normalize('NFC');
    const unsafe = path.includes('\\') || path.includes(':') || Array.from(path).some(char => char.codePointAt(0)! < 32 || char.codePointAt(0) === 127);
    if (root?.normalize('NFC') !== directory || !path || parts.some(p => p === '..' || p === '.' || !p) || unsafe || paths.has(path)) throw new Error(`${file.name}：路径不合格或归一化后重复。`);
    if (parts.some(part => part.toLowerCase() === 'scripts')) throw new Error(`${path}：做法包不允许 scripts 目录。`);
    total += file.size;
    if (file.size > limits.maxSkillFileBytes || total > limits.maxSkillTotalBytes) throw new Error(`${path}：超过做法文件或总容量上限。`);
    paths.add(path);
    const bytes = new Uint8Array(await file.arrayBuffer());
    let encoding: 'utf8' | 'base64' = 'utf8'; let content: string;
    try { content = new TextDecoder('utf-8', { fatal: true }).decode(bytes); }
    catch {
      encoding = 'base64'; let binary = '';
      for (let offset = 0; offset < bytes.length; offset += 8192) binary += String.fromCharCode(...bytes.subarray(offset, offset + 8192));
      content = btoa(binary);
    }
    uploads.push({ path, encoding, content });
  }
  if (!paths.has('SKILL.md')) throw new Error('所选目录缺少根目录 SKILL.md。');
  return { directory, files: uploads };
}
