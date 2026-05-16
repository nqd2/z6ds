import { useCallback, useEffect, useRef, useState } from "react";
import Editor, { type OnMount } from "@monaco-editor/react";
import type { editor } from "monaco-editor";
import type { BuildResult, Diagnostic } from "../lib/contracts";
import { EventTypes } from "../lib/contracts";
import { fsReadFile, fsWriteFile } from "../lib/tauriApi";
import { useAppEvent } from "../hooks/useAppEvent";
import { registerNavigateTo } from "./editorApi";
import { languageForPath } from "./languageForPath";
import { MONACO_Z6DS_THEME, registerMonacoZ6dsTheme } from "../theme/monaco-theme";
import "./EditorHost.css";

export interface EditorTab {
  path: string;
  label: string;
  content: string;
  savedContent: string;
  dirty: boolean;
  version: number;
}

interface EditorHostProps {
  openPath: string | null;
  onOpenPathConsumed?: () => void;
}

function severityToMarker(sev: string): number {
  switch (sev.toLowerCase()) {
    case "error":
      return 8;
    case "warning":
      return 4;
    case "info":
      return 2;
    default:
      return 8;
  }
}

export function EditorHost({ openPath, onOpenPathConsumed }: EditorHostProps) {
  const [tabs, setTabs] = useState<EditorTab[]>([]);
  const [activePath, setActivePath] = useState<string | null>(null);
  const editorRef = useRef<editor.IStandaloneCodeEditor | null>(null);
  const monacoRef = useRef<typeof import("monaco-editor") | null>(null);
  const diagnosticsRef = useRef<Map<string, Diagnostic[]>>(new Map());

  const activeTab = tabs.find((t) => t.path === activePath) ?? null;

  const openFile = useCallback(async (path: string) => {
    const exists = tabs.some((t) => t.path === path);
    if (exists) {
      setActivePath(path);
      return;
    }

    const content = await fsReadFile(path);
    const label = path.split(/[/\\]/).pop() ?? path;
    setTabs((prev) => {
      if (prev.some((t) => t.path === path)) return prev;
      return [
        ...prev,
        {
          path,
          label,
          content,
          savedContent: content,
          dirty: false,
          version: 1,
        },
      ];
    });
    setActivePath(path);
  }, [tabs]);

  const applyDiagnostics = useCallback(
    (path: string, diags: Diagnostic[]) => {
      diagnosticsRef.current.set(path, diags);
      const ed = editorRef.current;
      const monaco = monacoRef.current;
      if (!ed || !monaco || activePath !== path) return;
      const model = ed.getModel();
      if (!model) return;
      monaco.editor.setModelMarkers(
        model,
        "z6ds-build",
        diags.map((d) => ({
          startLineNumber: d.line,
          startColumn: d.column,
          endLineNumber: d.line,
          endColumn: d.column + 1,
          message: d.message,
          severity: severityToMarker(d.severity),
        })),
      );
    },
    [activePath],
  );

  useEffect(() => {
    registerNavigateTo((path, line, column) => {
      void openFile(path).then(() => {
        const ed = editorRef.current;
        if (ed) {
          ed.setPosition({ lineNumber: line, column });
          ed.revealLineInCenter(line);
          ed.focus();
        }
      });
    });
    return () => registerNavigateTo(() => {});
  }, [openFile]);

  useEffect(() => {
    if (openPath) {
      void openFile(openPath).finally(() => onOpenPathConsumed?.());
    }
  }, [openPath, openFile, onOpenPathConsumed]);

  useEffect(() => {
    if (activePath) {
      applyDiagnostics(activePath, diagnosticsRef.current.get(activePath) ?? []);
    }
  }, [activePath, applyDiagnostics]);

  useAppEvent<BuildResult>(EventTypes.buildCompleted, (ev) => {
    const byPath = new Map<string, Diagnostic[]>();
    for (const d of ev.payload.diagnostics ?? []) {
      const list = byPath.get(d.path) ?? [];
      list.push(d);
      byPath.set(d.path, list);
    }
    for (const [path, diags] of byPath) {
      applyDiagnostics(path, diags);
    }
  });

  const saveActive = useCallback(async () => {
    if (!activeTab?.dirty) return;
    await fsWriteFile(activeTab.path, activeTab.content);
    setTabs((prev) =>
      prev.map((t) =>
        t.path === activeTab.path
          ? {
              ...t,
              savedContent: t.content,
              dirty: false,
              version: t.version + 1,
            }
          : t,
      ),
    );
  }, [activeTab]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key === "s") {
        e.preventDefault();
        void saveActive();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [saveActive]);

  const handleMount: OnMount = (ed, monaco) => {
    editorRef.current = ed;
    monacoRef.current = monaco;
    registerMonacoZ6dsTheme(monaco);
    monaco.editor.setTheme(MONACO_Z6DS_THEME);
    if (activePath) {
      applyDiagnostics(activePath, diagnosticsRef.current.get(activePath) ?? []);
    }
  };

  const updateContent = (value: string) => {
    if (!activeTab) return;
    setTabs((prev) =>
      prev.map((t) =>
        t.path === activeTab.path
          ? { ...t, content: value, dirty: value !== t.savedContent }
          : t,
      ),
    );
  };

  return (
    <div className="editor-host">
      <div className="editor-tabs">
        {tabs.map((tab) => (
          <button
            key={tab.path}
            type="button"
            className={`editor-tab${tab.path === activePath ? " active" : ""}`}
            onClick={() => setActivePath(tab.path)}
          >
            {tab.label}
            {tab.dirty ? " •" : ""}
          </button>
        ))}
      </div>
      <div className="editor-body">
        {activeTab ? (
          <Editor
            path={activeTab.path}
            language={languageForPath(activeTab.path)}
            value={activeTab.content}
            theme={MONACO_Z6DS_THEME}
            onChange={(v) => updateContent(v ?? "")}
            onMount={handleMount}
            options={{
              fontSize: 13,
              fontFamily: "IBM Plex Mono, JetBrains Mono, Fira Code, Consolas, monospace",
              lineHeight: 22,
              minimap: { enabled: false },
              scrollBeyondLastLine: false,
              automaticLayout: true,
              padding: { top: 8 },
            }}
          />
        ) : (
          <div className="editor-empty">Open a file from Explorer</div>
        )}
      </div>
    </div>
  );
}
