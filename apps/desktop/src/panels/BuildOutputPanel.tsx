import { useCallback, useEffect, useRef, useState } from "react";
import type {
  BuildLogChunk,
  BuildResult,
  BuildStarted,
  Diagnostic,
} from "../lib/contracts";
import { EventTypes } from "../lib/contracts";
import { useAppEvent } from "../hooks/useAppEvent";
import { navigateTo } from "../editor/editorApi";
import "./BuildOutputPanel.css";

type PanelStatus = "empty" | "running" | "success" | "failed" | "cancelled";

const STATUS_LABEL: Record<PanelStatus, string> = {
  empty: "Ready",
  running: "Building…",
  success: "Succeeded",
  failed: "Failed",
  cancelled: "Cancelled",
};

export function BuildOutputPanel() {
  const [lines, setLines] = useState<string[]>([]);
  const [status, setStatus] = useState<PanelStatus>("empty");
  const [summary, setSummary] = useState<string>("");
  const [diagnostics, setDiagnostics] = useState<Diagnostic[]>([]);
  const [scrollLock, setScrollLock] = useState(false);
  const [filter, setFilter] = useState<"all" | "error" | "warning">("all");
  const logRef = useRef<HTMLPreElement>(null);

  const appendChunk = useCallback((chunk: BuildLogChunk) => {
    const prefix = chunk.stream === "stderr" ? "[stderr] " : "";
  const text = chunk.text.endsWith("\n") ? chunk.text : `${chunk.text}\n`;
    setLines((prev) => [...prev, `${prefix}${text}`]);
  }, []);

  useAppEvent<BuildStarted>(EventTypes.buildStarted, () => {
    setLines([]);
    setDiagnostics([]);
    setSummary("");
    setStatus("running");
  });

  useAppEvent<BuildLogChunk>(EventTypes.buildLog, (ev) => {
    appendChunk(ev.payload);
  });

  useAppEvent<BuildResult>(EventTypes.buildCompleted, (ev) => {
    const result = ev.payload;
    setDiagnostics(result.diagnostics ?? []);
    const errors = result.diagnostics.filter((d) => d.severity === "error").length;
    const warnings = result.diagnostics.filter((d) => d.severity === "warning").length;

    if (result.status === "success") {
      setStatus("success");
      setSummary(
        `Build succeeded in ${result.durationMs}ms` +
          (result.elfPath ? ` — ${result.elfPath}` : ""),
      );
    } else if (result.status === "cancelled") {
      setStatus("cancelled");
      setSummary("Build cancelled");
    } else {
      setStatus("failed");
      setSummary(
        `Build failed — ${errors} error(s), ${warnings} warning(s)` +
          (result.errorCode ? ` (${result.errorCode})` : ""),
      );
    }

    if (result.logText && lines.length === 0) {
      setLines(result.logText.split("\n").map((l) => (l ? `${l}\n` : "\n")));
    }
  });

  useEffect(() => {
    if (!scrollLock && logRef.current) {
      logRef.current.scrollTop = logRef.current.scrollHeight;
    }
  }, [lines, scrollLock]);

  const visibleLog = lines.join("");
  const filteredDiagnostics = diagnostics.filter((d) => {
    if (filter === "all") return true;
    return d.severity === filter;
  });

  const copyLog = async () => {
    try {
      await navigator.clipboard.writeText(visibleLog);
    } catch {
      /* clipboard unavailable */
    }
  };

  const onDiagnosticClick = (d: Diagnostic) => {
    if (!d.path) return;
    navigateTo(d.path, d.line, d.column);
  };

  return (
    <div className="build-panel">
      <div className="build-panel-toolbar">
        <span className={`build-status build-status-${status}`}>{STATUS_LABEL[status]}</span>
        {summary && <span className="build-summary">{summary}</span>}
        <div className="build-panel-actions">
          <label className="build-scroll-lock">
            <input
              type="checkbox"
              checked={scrollLock}
              onChange={(e) => setScrollLock(e.target.checked)}
            />
            Scroll lock
          </label>
          <select
            value={filter}
            onChange={(e) =>
              setFilter(e.target.value as "all" | "error" | "warning")
            }
            aria-label="Filter diagnostics"
          >
            <option value="all">All</option>
            <option value="error">Errors</option>
            <option value="warning">Warnings</option>
          </select>
          <button type="button" onClick={() => setLines([])}>
            Clear
          </button>
          <button type="button" onClick={() => void copyLog()}>
            Copy
          </button>
        </div>
      </div>
      {filteredDiagnostics.length > 0 && (
        <ul className="build-diagnostics">
          {filteredDiagnostics.map((d, i) => (
            <li key={`${d.path}:${d.line}:${i}`}>
              <button
                type="button"
                className={`diag-${d.severity}`}
                onClick={() => onDiagnosticClick(d)}
              >
                {d.path}:{d.line}:{d.column} — {d.message}
              </button>
            </li>
          ))}
        </ul>
      )}
      <pre ref={logRef} className="build-log">
        {visibleLog || "Build output will appear here…"}
      </pre>
    </div>
  );
}
