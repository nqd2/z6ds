import { useEffect, useState } from "react";
import { preloadCommonMaterialIcons, resolveMaterialIconUrl } from "./materialIconTheme";
import "./FileIcon.css";

preloadCommonMaterialIcons();

export interface FileIconProps {
  name: string;
  isDir: boolean;
  expanded?: boolean;
  className?: string;
}

/**
 * Material Icon Theme glyph only — tree twistie is rendered by ExplorerPanel.
 */
export function FileIcon({ name, isDir, expanded = false, className = "" }: FileIconProps) {
  const [src, setSrc] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    void resolveMaterialIconUrl({ name, isDir, expanded, isRoot: false }).then((url) => {
      if (!cancelled) setSrc(url);
    });
    return () => {
      cancelled = true;
    };
  }, [name, isDir, expanded]);

  return (
    <span className={`file-icon-root ${className}`.trim()}>
      {src ? (
        <img
          className="material-icon-img"
          src={src}
          alt=""
          width={16}
          height={16}
          draggable={false}
        />
      ) : (
        <span className="material-icon-placeholder" aria-hidden />
      )}
    </span>
  );
}
