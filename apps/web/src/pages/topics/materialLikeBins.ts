import { known } from "./topicModel";
import type { Work } from "./types";

const ranges = [
  [0, 20, "0–20"], [21, 99, "21–99"], [100, 499, "100–499"],
  [500, 999, "500–999"], [1000, 4999, "1千–<5千"], [5000, Infinity, "≥5千"],
] as const;

/** Presentation bins describe returned material, never the server's full statistical denominator. */
export function materialLikeBins(works: readonly Work[], threshold: number | null) {
  const bins = ranges.map(([min, max, label]) => ({
    min, max, label, refs: [] as string[],
    high: threshold !== null && min >= threshold,
  }));
  const unknown: string[] = [];
  for (const work of works) {
    const likes = work.likes;
    if (!known(likes)) unknown.push(work.workRef);
    else bins.find((bin) => likes >= bin.min && likes <= bin.max)?.refs.push(work.workRef);
  }
  return { bins, unknown };
}
