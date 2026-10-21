type CurvePoint = { x: number; y: number };

/** Smooth measured values without bridging missing data or overshooting adjacent values. */
export function smoothSeriesPath(points: Array<CurvePoint | null>, baseline?: number): string {
  const segments: CurvePoint[][] = [];
  let segment: CurvePoint[] = [];
  const flush = () => { if (segment.length) segments.push(segment); segment = []; };
  for (const point of points) {
    if (point === null) flush();
    else segment.push(point);
  }
  flush();
  const fmt = (value: number) => value.toFixed(2);
  return segments.map((line) => {
    if (!line.length) return "";
    let d = "M" + fmt(line[0]!.x) + "," + fmt(line[0]!.y);
    for (let i = 0; i < line.length - 1; i++) {
      const a = line[i]!, b = line[i + 1]!;
      const before = line[Math.max(0, i - 1)]!;
      const after = line[Math.min(line.length - 1, i + 2)]!;
      const dx = (b.x - a.x) / 3;
      const min = Math.min(a.y, b.y), max = Math.max(a.y, b.y);
      const clamp = (value: number) => Math.max(min, Math.min(max, value));
      const firstY = clamp(a.y + (b.y - before.y) / 6);
      const secondY = clamp(b.y - (after.y - a.y) / 6);
      d += " C" + fmt(a.x + dx) + "," + fmt(firstY) + " " +
        fmt(b.x - dx) + "," + fmt(secondY) + " " + fmt(b.x) + "," + fmt(b.y);
    }
    if (baseline !== undefined) {
      d += " L" + fmt(line.at(-1)!.x) + "," + fmt(baseline) +
        " L" + fmt(line[0]!.x) + "," + fmt(baseline) + " Z";
    }
    return d;
  }).filter(Boolean).join(" ");
}
