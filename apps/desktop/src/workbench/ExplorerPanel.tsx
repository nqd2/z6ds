import { useCallback, useEffect, useState } from "react";
import type { FsEntry } from "../lib/contracts";
import { fsListDir } from "../lib/tauriApi";
import { IconTreeChevron } from "../components/Icons";
import { FileIcon } from "../icons/FileIcon";
import "./ExplorerPanel.css";

interface TreeNode extends FsEntry {
  children?: TreeNode[];
  expanded?: boolean;
  loading?: boolean;
}

interface ExplorerPanelProps {
  rootPath: string | null;
  onOpenFile: (path: string) => void;
}

const INDENT_PX = 8;

export function ExplorerPanel({ rootPath, onOpenFile }: ExplorerPanelProps) {
  const [tree, setTree] = useState<TreeNode[]>([]);

  const loadDir = useCallback(async (path: string): Promise<TreeNode[]> => {
    const entries = await fsListDir(path);
    return entries.map((e) => ({ ...e, expanded: false }));
  }, []);

  useEffect(() => {
    if (!rootPath) {
      setTree([]);
      return;
    }
    void loadDir(rootPath).then(setTree);
  }, [rootPath, loadDir]);

  const toggleDir = async (node: TreeNode) => {
    if (!node.isDir) {
      onOpenFile(node.path);
      return;
    }
    if (node.expanded) {
      setTree((prev) => updateNode(prev, node.path, (n) => ({ ...n, expanded: false })));
      return;
    }
    setTree((prev) =>
      updateNode(prev, node.path, (n) => ({ ...n, loading: true, expanded: true })),
    );
    const children = await loadDir(node.path);
    setTree((prev) =>
      updateNode(prev, node.path, (n) => ({
        ...n,
        loading: false,
        expanded: true,
        children,
      })),
    );
  };

  if (!rootPath) {
    return (
      <div className="explorer-panel explorer-empty">
        Open a folder to browse project files
      </div>
    );
  }

  return (
    <div className="explorer-panel workbench-scroll">
      <div className="explorer-title">EXPLORER</div>
      <ul className="explorer-tree">
        {tree.map((node) => (
          <ExplorerNode key={node.path} node={node} depth={0} onToggle={toggleDir} />
        ))}
      </ul>
    </div>
  );
}

function ExplorerNode({
  node,
  depth,
  onToggle,
}: {
  node: TreeNode;
  depth: number;
  onToggle: (node: TreeNode) => void;
}) {
  const expanded = Boolean(node.expanded);

  return (
    <li className="explorer-node">
      <button
        type="button"
        className="explorer-item"
        style={{ paddingLeft: `${8 + depth * INDENT_PX}px` }}
        onClick={() => void onToggle(node)}
      >
        {node.isDir ? (
          <span className="explorer-twistie" aria-hidden>
            <IconTreeChevron expanded={expanded} />
          </span>
        ) : (
          <span className="explorer-twistie explorer-twistie-spacer" aria-hidden />
        )}
        <FileIcon
          name={node.name}
          isDir={node.isDir}
          expanded={expanded}
          className="explorer-file-icon"
        />
        <span className="explorer-label">{node.name}</span>
        {node.loading ? <span className="explorer-loading">…</span> : null}
      </button>
      {expanded && node.children && (
        <ul className="explorer-children">
          {node.children.map((child) => (
            <ExplorerNode key={child.path} node={child} depth={depth + 1} onToggle={onToggle} />
          ))}
        </ul>
      )}
    </li>
  );
}

function updateNode(
  nodes: TreeNode[],
  path: string,
  fn: (n: TreeNode) => TreeNode,
): TreeNode[] {
  return nodes.map((n) => {
    if (n.path === path) return fn(n);
    if (n.children) {
      return { ...n, children: updateNode(n.children, path, fn) };
    }
    return n;
  });
}
