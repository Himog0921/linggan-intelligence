import { ContentProjectionReadService } from "./content-projection-read-service";
import {
  getV2DatabaseClient,
  validateV2DatabaseIdentity,
} from "../security/v2-database-roles";

export async function readMaterialV2Content(input: {
  workspaceId: string;
  platformContentId: string;
}) {
  const db = getV2DatabaseClient("default_app");
  await validateV2DatabaseIdentity(db, "default_app");
  return new ContentProjectionReadService(db).read({
    workspaceId: input.workspaceId,
    platform: "xhs",
    platformContentId: input.platformContentId,
  });
}
