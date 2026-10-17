import { cleanup, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it } from "vitest";
import { materialLikeBins } from "./materialLikeBins";
import { OverviewDistribution } from "./OverviewDistribution";
import { OverviewReadouts } from "./OverviewReadouts";
import { topicSnapshot, topicStats, topicWork } from "./topicTestFixture";

afterEach(cleanup);
describe("material-first overview preserves statistical boundaries", () => {
  it("partitions every returned like boundary while retaining unknown separately and respecting the actual threshold", () => {
    const likes = [0, 20, 21, 99, 100, 499, 500, 999, 1000, 4999, 5000, null];
    const works = likes.map((likes, i) => ({ ...topicWork(i), likes }));
    const result = materialLikeBins(works, 750);
    expect(result.bins.map((b) => b.refs.length)).toEqual([2, 2, 2, 2, 2, 1]);
    expect(result.unknown).toEqual([works[11]!.workRef]);
    expect(result.bins.map((b) => b.high)).toEqual([false, false, false, false, true, true]);
    expect(materialLikeBins(works, null).bins.every((b) => !b.high)).toBe(true);
    expect(works.map((w) => w.likes)).toEqual(likes);
  });
  it("shows server totals and every real platform without conflating zero, unknown and an unset rule", () => {
    const stats = topicStats(9000);
    stats.platforms.push({ platform: "douyin", workCount: 8, knownLikeCount: 0, unknownLikeCount: 8, highPerformanceCount: null, highPerformanceThreshold: 750, medianLikes: null, p90Likes: null });
    render(<OverviewReadouts statistics={stats} scope={topicSnapshot().scope} />);
    const region = screen.getByLabelText("基础概览指标");
    expect(within(region).getByRole("button", { name: "观察到的内容 9,000 篇，查看材料" })).toBeInTheDocument();
    expect(region).toHaveTextContent("小红书 · 样本爆款率");
    expect(region).toHaveTextContent("抖音 · 样本爆款率");
    expect(region).toHaveTextContent("中位数0");
    expect(region).toHaveTextContent("未设置");
    expect(region).toHaveTextContent("点赞未知 8 篇");
    expect(region).toHaveTextContent("点赞 ≥ 750");
    expect(region).toHaveTextContent("可读 2");
  });
  it("keeps the loaded histogram distinct from full server scope and preserves exact drill-down references in chart and table", async () => {
    const snapshot = topicSnapshot();
    const stats = topicStats(9000);
    render(<OverviewDistribution works={snapshot.works} platform={stats.platforms[0]} statistics={stats} scope={snapshot.scope} />);
    const region = screen.getByLabelText("已观察内容点赞分布");
    expect(region).toHaveTextContent("当前返回 2 篇 · 点赞可判定 1 篇");
    expect(region).toHaveTextContent("服务端该平台统计 9,000 篇");
    expect(region).toHaveTextContent("点赞未知 1 篇，单独保留");
    expect(within(region).getByRole("button", { name: "0–20赞，1篇，查看作品" })).toHaveAttribute("data-id", snapshot.works[0]!.workRef);
    expect(within(region).getByRole("button", { name: "查看未知指标材料" })).toHaveAttribute("data-id", snapshot.works[1]!.workRef);
    await userEvent.click(within(region).getByRole("button", { name: "表格" }));
    const table = within(region).getByRole("table");
    expect(within(table).getAllByRole("button")[0]).toHaveAttribute("data-id", snapshot.works[0]!.workRef);
    expect(within(table).getAllByRole("button")[1]).toBeDisabled();
    expect(screen.getByLabelText("当前资料边界")).toHaveTextContent("账号范围未明确");
  });
});
