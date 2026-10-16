import styles from "./Ops.module.css";
export const c = (...names: string[]) =>
  names.map((name) => styles[name] || name).join(" ");
export const readonly = "切换前只读：录入仍在内容工作台";
export const peopleTabs = (names: string[]): [string, string][] => [
  ["", "全部"],
  ...names.map((name): [string, string] => [name, name]),
];
