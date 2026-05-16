import {
  generateManifest,
  type Manifest,
  type ManifestConfig,
} from "material-icon-theme";

const manifest: Manifest = generateManifest({
  activeIconPack: "none",
  hidesExplorerArrows: false,
} as unknown as ManifestConfig);

const iconLoaders = import.meta.glob<string>(
  "../../node_modules/material-icon-theme/icons/*.svg",
  { query: "?url", import: "default" },
);

const loaderByBase = new Map<string, () => Promise<string>>();
for (const [path, loader] of Object.entries(iconLoaders)) {
  const base = path.split("/").pop();
  if (base) loaderByBase.set(base, loader as () => Promise<string>);
}

const urlCache = new Map<string, string>();

async function loadSvgBase(baseName: string, fallback: string): Promise<string> {
  const cached = urlCache.get(baseName);
  if (cached) return cached;

  const loader = loaderByBase.get(baseName);
  if (!loader) {
    if (baseName !== fallback) return loadSvgBase(fallback, fallback);
    return "";
  }

  const url = await loader();
  urlCache.set(baseName, url);
  return url;
}

function baseNameForDefinitionId(defId: string | undefined): string {
  if (!defId) return "file.svg";
  const defs = manifest.iconDefinitions;
  const def = defs?.[defId];
  if (!def?.iconPath) return "file.svg";
  return def.iconPath.split("/").pop() ?? "file.svg";
}

function lookupName(map: Record<string, string> | undefined, name: string): string | undefined {
  if (!map) return undefined;
  return map[name] ?? map[name.toLowerCase()];
}

function extensionIconId(fileName: string): string | undefined {
  const dot = fileName.indexOf(".");
  if (dot < 0) return undefined;
  const extMaps = manifest.fileExtensions;
  if (!extMaps) return undefined;

  const tail = fileName.slice(dot + 1).toLowerCase();
  const parts = tail.split(".");
  for (let i = 0; i < parts.length; i++) {
    const ext = parts.slice(i).join(".");
    const hit = extMaps[ext];
    if (hit) return hit;
  }
  return undefined;
}

function definitionIdForRequest({
  name,
  isDir,
  expanded,
  isRoot,
}: MaterialIconRequest): string | undefined {
  if (isDir) {
    if (isRoot) {
      return expanded
        ? (lookupName(manifest.rootFolderNamesExpanded, name) ?? manifest.rootFolderExpanded)
        : (lookupName(manifest.rootFolderNames, name) ?? manifest.rootFolder);
    }
    return expanded
      ? (lookupName(manifest.folderNamesExpanded, name) ?? manifest.folderExpanded)
      : (lookupName(manifest.folderNames, name) ?? manifest.folder);
  }
  return lookupName(manifest.fileNames, name) ?? extensionIconId(name) ?? manifest.file;
}

export interface MaterialIconRequest {
  name: string;
  isDir: boolean;
  expanded?: boolean;
  isRoot?: boolean;
}

/** Resolve Material Icon Theme SVG URL (lazy-loaded per icon). */
export async function resolveMaterialIconUrl(request: MaterialIconRequest): Promise<string> {
  const defId = definitionIdForRequest(request);
  const base = baseNameForDefinitionId(defId);
  if (request.isDir) {
    const fallback = request.expanded ? "folder-open.svg" : "folder.svg";
    return loadSvgBase(base, fallback);
  }
  return loadSvgBase(base, "file.svg");
}

/** Warm common explorer icons. */
export function preloadCommonMaterialIcons(): void {
  void loadSvgBase("folder.svg", "folder.svg");
  void loadSvgBase("folder-open.svg", "folder-open.svg");
  void loadSvgBase("file.svg", "file.svg");
  void loadSvgBase("c.svg", "file.svg");
}
