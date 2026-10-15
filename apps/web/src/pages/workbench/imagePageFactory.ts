import type { ImagePage } from "./workbenchModel";

export function newImagePage(kind: ImagePage["kind"]): ImagePage {
  return {
    id: crypto.randomUUID(),
    kind,
    templateVersion: "creation-v3-type",
    title: "",
    subtitle: "",
    blocks: [{ id: crypto.randomUUID(), text: "" }],
    assetId: null,
    crop: { x: 0, y: 0, width: 1, height: 1 },
    style: {
      palette: "cream",
      fontFamily: "system-cjk",
      width: 1080,
      height: 1440,
      alignment: "start",
    },
  };
}
