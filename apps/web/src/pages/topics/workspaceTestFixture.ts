/** Synthetic Workspace input; contains no shared material. */
export function workspaceFixture(key = "workspace-proof") {
  const members = (["support", "challenge", "boundary"] as const).map(
    (role, index) => ({
      workPublicRef: `10000000-0000-4000-8000-00000000000${index + 1}`,
      role,
      rationale: `${role} 的人工裁定理由`,
      ordinal: index + 1,
    }),
  );
  return {
    topic: {
      canonicalKey: key,
      definition: {
        displayName: `合成工作区 ${key}`,
        definitionText: "合成定义，仅用于隔离测试。",
        version: 2,
        lifecycleState: "provisional",
      },
      classificationRun: {
        adjudicationNote: "人工冻结的合成材料集合。",
        runKind: "human_adjudicated",
        runState: "completed",
      },
      materialPack: {
        materialPackRef: "20000000-0000-4000-8000-000000000001",
        sourceBoundary: "合成输入，不证明真实来源覆盖或总体代表性。",
      },
      members,
    },
    materials: members.map((classification, index) => ({
      classification: { ...classification },
      detailUrl: `/api/local/work-resources/${classification.workPublicRef}`,
      workResource: {
        identity: {
          publicRef: classification.workPublicRef,
          platform: "xhs",
          contentExternalId: `synthetic-work-${index}`,
        },
        display: {
          title: index === 0 ? null : `合成材料 ${index + 1}`,
          creatorDisplayName: index === 0 ? null : "合成创作者",
          publishedAt: null,
          publishedAtSourceText: index === 0 ? null : "平台原始相对时间",
        },
        summary: { primaryLimitation: "SYNTHETIC_MINIMUM_NECESSARY" },
      },
    })),
  };
}
