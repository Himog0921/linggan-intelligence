import styles from "./Topics.module.css";
export function c(...names: string[]) {
  return names
    .flatMap((value) => value.split(/\s+/))
    .filter(Boolean)
    .map((name) => styles[name] || name)
    .join(" ");
}
