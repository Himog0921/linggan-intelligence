import { requestJson } from "../../shared/api";
import * as d from "../../shared/decode";

export type WorkspaceRole = "support" | "challenge" | "boundary";
const uuidPattern =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

function reference(record: d.Json, name: string) {
  const value = d.str(record, name);
  if (!uuidPattern.test(value)) throw new d.ShapeError(name);
  return value;
}

function positiveInteger(record: d.Json, name: string) {
  const value = d.num(record, name);
  if (!Number.isSafeInteger(value) || value < 1) throw new d.ShapeError(name);
  return value;
}

function nullableString(record: d.Json, name: string) {
  const value = d.field(record, name);
  if (value === null) return null;
  return d.str(record, name);
}

function member(record: d.Json): {
  ref: string;
  role: WorkspaceRole;
  rationale: string;
  ordinal: number;
} {
  const role = d.str(record, "role");
  if (role !== "support" && role !== "challenge" && role !== "boundary")
    throw new d.ShapeError("classification.role");
  return {
    ref: reference(record, "workPublicRef"),
    role,
    rationale: d.str(record, "rationale"),
    ordinal: positiveInteger(record, "ordinal"),
  };
}

/** The API composes each frozen member with its current shared Work Resource.
 * A partial or differently identified pack must not render as the requested workspace. */
export function workspace(value: unknown, expectedKey?: string) {
  return d.decode(value, (root) => {
    const topic = d.obj(root, "topic");
    const definition = d.obj(topic, "definition");
    const run = d.obj(topic, "classificationRun");
    const pack = d.obj(topic, "materialPack");
    const canonicalKey = d.str(topic, "canonicalKey");
    if (
      !/^[a-z][a-z0-9_-]{1,95}$/.test(canonicalKey) ||
      (expectedKey !== undefined && canonicalKey !== expectedKey)
    ) {
      throw new d.ShapeError("canonicalKey");
    }
    // Imports persist human adjudications; the structure store also persists machine candidates.
    // Both are existing completed packs, not authority to accept arbitrary run kinds.
    if (
      !["provisional", "candidate"].includes(
        d.str(definition, "lifecycleState"),
      ) ||
      !["human_adjudicated", "machine_proposed"].includes(d.str(run, "runKind")) ||
      d.str(run, "runState") !== "completed"
    ) {
      throw new d.ShapeError("classificationRun");
    }
    const members = d.list(topic, "members", member);
    const refs = new Set(members.map((item) => item.ref));
    const ordinals = new Set(members.map((item) => item.ordinal));
    if (refs.size !== members.length || ordinals.size !== members.length)
      throw new d.ShapeError("members");
    const materials = d.list(root, "materials", (item, index) => {
      const classification = member(d.obj(item, "classification"));
      const frozen = members[index];
      if (
        !frozen ||
        frozen.ref !== classification.ref ||
        frozen.role !== classification.role ||
        frozen.rationale !== classification.rationale ||
        frozen.ordinal !== classification.ordinal
      ) {
        throw new d.ShapeError("materials.classification");
      }
      const resource = d.obj(item, "workResource");
      const display = d.obj(resource, "display");
      const identity = d.obj(resource, "identity");
      const summary = d.obj(resource, "summary");
      if (reference(identity, "publicRef") !== classification.ref)
        throw new d.ShapeError("workResource.identity.publicRef");
      const detailUrl = d.str(item, "detailUrl");
      if (detailUrl !== `/api/local/work-resources/${classification.ref}`)
        throw new d.ShapeError("detailUrl");
      return {
        ...classification,
        detailUrl,
        title: nullableString(display, "title"),
        creator: nullableString(display, "creatorDisplayName"),
        publishedAt: nullableString(display, "publishedAt"),
        publishedAtSourceText: nullableString(display, "publishedAtSourceText"),
        platform: d.str(identity, "platform"),
        externalId: d.str(identity, "contentExternalId"),
        primaryLimitation: d.str(summary, "primaryLimitation"),
      };
    });
    if (materials.length !== members.length)
      throw new d.ShapeError("materials.length");
    return {
      topic: {
        canonicalKey,
        definition: {
          displayName: d.str(definition, "displayName"),
          definitionText: d.str(definition, "definitionText"),
          version: positiveInteger(definition, "version"),
        },
        adjudicationNote: d.str(run, "adjudicationNote"),
        materialPackRef: reference(pack, "materialPackRef"),
        sourceBoundary: d.str(pack, "sourceBoundary"),
      },
      materials,
    };
  });
}
export type Workspace = ReturnType<typeof workspace>;
export type WorkspaceMaterial = Workspace["materials"][number];
export async function readWorkspace(key: string, signal: AbortSignal) {
  return workspace(
    await requestJson(
      `/api/local/topic-workspaces/${encodeURIComponent(key)}`,
      { signal },
    ),
    key,
  );
}
