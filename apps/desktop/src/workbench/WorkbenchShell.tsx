import { useCallback, useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import type {
  GpioStateMap,
  PeripheralVisualState,
  ProjectManifest,
  SimulatorSessionState,
  ToolchainInfo,
} from "../lib/contracts";
import { EventTypes, SCHEMA_VERSION } from "../lib/contracts";
import { useAppEvent } from "../hooks/useAppEvent";
import {
  applyNetlistDefaults,
  buildProject,
  cleanProject,
  detectToolchain,
  discoverProject,
  getSampleProjectRoot,
  handleBoardInteraction,
  labBoardConfig,
  parseBoardConfig,
  resolveSimElf,
  runSimulator,
  stopSimulator,
} from "../lib/tauriApi";
import { EmptyState } from "../components/EmptyState";
import { IconCode, IconCube, IconFolder } from "../components/Icons";
import { EditorHost } from "../editor/EditorHost";
import { BuildOutputPanel } from "../panels/BuildOutputPanel";
import { SerialTerminalPanel } from "../panels/SerialTerminalPanel";
import { SceneView } from "../sim3d/SceneView";
import { ExplorerPanel } from "./ExplorerPanel";
import { WorkbenchToolbar } from "./WorkbenchToolbar";
import "./WorkbenchShell.css";

type CenterView = "editor" | "simulator3d";
type BottomTab = "build" | "serial" | "terminal";
type BuildBadge = "idle" | "building" | "success" | "failed";

const BUILD_LABEL: Record<BuildBadge, string> = {
  idle: "Ready",
  building: "Building…",
  success: "Succeeded",
  failed: "Failed",
};

export function WorkbenchShell() {
  const [manifest, setManifest] = useState<ProjectManifest | null>(null);
  const [toolchain, setToolchain] = useState<ToolchainInfo | null>(null);
  const [centerView, setCenterView] = useState<CenterView>("editor");
  const [bottomTab, setBottomTab] = useState<BottomTab>("build");
  const [buildBadge, setBuildBadge] = useState<BuildBadge>("idle");
  const [simSession, setSimSession] = useState<SimulatorSessionState | null>(null);
  const [simError, setSimError] = useState<string | null>(null);
  const [discoveryError, setDiscoveryError] = useState<string | null>(null);
  const [gpio, setGpio] = useState<Record<string, number>>({});
  const [ledVisual, setLedVisual] = useState<Record<string, boolean>>({});
  const [editorOpenPath, setEditorOpenPath] = useState<string | null>(null);
  const [bottomHeight, setBottomHeight] = useState(220);

  const toolbarError = discoveryError ?? simError;

  useAppEvent<ProjectManifest>(EventTypes.projectOpened, (ev) => {
    setManifest(ev.payload);
    void applyNetlistFromIoc(ev.payload);
  });

  useAppEvent(EventTypes.buildStarted, () => {
    setBuildBadge("building");
    setBottomTab("build");
  });

  useAppEvent(EventTypes.buildCompleted, (ev) => {
    const payload = ev.payload as { status?: string };
    setBuildBadge(payload.status === "success" ? "success" : "failed");
  });

  useAppEvent<SimulatorSessionState>(EventTypes.simulatorStarted, (ev) => {
    setSimSession(ev.payload);
    setSimError(null);
    setBottomTab("serial");
  });

  useAppEvent<SimulatorSessionState>(EventTypes.simulatorStopped, (ev) => {
    setSimSession(ev.payload);
  });

  useAppEvent<SimulatorSessionState>(EventTypes.simulatorError, (ev) => {
    setSimSession(ev.payload);
    setSimError(ev.payload.message ?? ev.payload.errorCode ?? "simulator error");
  });

  useAppEvent<GpioStateMap>(EventTypes.gpioChanged, (ev) => {
    setGpio(ev.payload.pins);
  });

  useAppEvent<PeripheralVisualState>(EventTypes.peripheralVisualChanged, (ev) => {
    const { instanceId, state, moduleType } = ev.payload;
    if (moduleType === "led" && typeof state.on === "boolean") {
      const pin = instanceId.includes("led4") ? "PG14" : "PG13";
      setLedVisual((prev) => ({ ...prev, [pin]: Boolean(state.on) }));
    }
  });

  useEffect(() => {
    void detectToolchain().then(setToolchain).catch(() => {});
  }, []);

  const applyNetlistFromIoc = async (m: ProjectManifest) => {
    try {
      const board = await parseBoardConfig(m.iocPath);
      await applyNetlistDefaults(board);
    } catch {
      /* optional */
    }
  };

  const openFolder = async (path?: string) => {
    let root = path;
    if (!root) {
      const selected = await open({ directory: true, multiple: false });
      if (!selected || typeof selected !== "string") return;
      root = selected;
    }
    setDiscoveryError(null);
    const result = await discoverProject(root);
    if (result.manifest) {
      setManifest(result.manifest);
      void applyNetlistFromIoc(result.manifest);
    } else {
      const msg =
        result.errors.map((e) => e.message).join(" ") ||
        `Could not open project at ${root}`;
      setDiscoveryError(msg);
      setManifest(null);
    }
  };

  const openSample = async () => {
    try {
      const root = await getSampleProjectRoot();
      await openFolder(root);
    } catch (e) {
      setDiscoveryError(String(e));
    }
  };

  const runBuild = async () => {
    if (!manifest) return;
    setBottomTab("build");
    try {
      await buildProject(manifest.rootPath, "Debug");
    } catch {
      setBuildBadge("failed");
    }
  };

  const runClean = async () => {
    if (!manifest) return;
    try {
      await cleanProject(manifest.rootPath, "Debug");
    } catch {
      /* ignore */
    }
  };

  const runSim = async () => {
    if (!manifest) return;
    setSimError(null);
    try {
      const elfPath = await resolveSimElf(
        manifest.elfCandidates[0]?.path,
        manifest.rootPath,
      );
      const board = await parseBoardConfig(manifest.iocPath).catch(() =>
        labBoardConfig(),
      );
      const state = await runSimulator({
        schemaVersion: SCHEMA_VERSION,
        elfPath,
        boardConfig: board,
        sessionOptions: { schemaVersion: SCHEMA_VERSION, resetOnStart: true },
      });
      setSimSession(state);
      setCenterView("simulator3d");
      setBottomTab("serial");
    } catch (e) {
      setSimError(String(e));
    }
  };

  const stopSim = async () => {
    try {
      const state = await stopSimulator();
      setSimSession(state);
    } catch (e) {
      setSimError(String(e));
    }
  };

  const boardInteraction = useCallback(
    async (type: string, targetId: string) => {
      if (!simSession?.sessionId) return;
      await handleBoardInteraction({
        schemaVersion: SCHEMA_VERSION,
        sessionId: simSession.sessionId,
        type,
        targetId,
      });
    },
    [simSession],
  );

  const openMainC = useCallback(() => {
    if (!manifest) return;
    setEditorOpenPath(`${manifest.rootPath}/Core/Src/main.c`);
    setCenterView("editor");
  }, [manifest]);

  const simRunning = simSession?.status === "running";

  return (
    <div className="workbench theme-z6ds">
      <WorkbenchToolbar
        projectName={manifest?.projectName ?? null}
        hasProject={!!manifest}
        simRunning={simRunning}
        simError={toolbarError}
        onOpenFolder={() => void openFolder()}
        onOpenSample={() => void openSample()}
        onBuild={() => void runBuild()}
        onClean={() => void runClean()}
        onRunSim={() => void runSim()}
        onStopSim={() => void stopSim()}
        onOpenMainC={openMainC}
      />

      <div className="workbench-main">
        <nav className="activity-bar" aria-label="Views">
          <button
            type="button"
            className={`activity-item${centerView === "editor" ? " active" : ""}`}
            title="Editor"
            aria-current={centerView === "editor" ? "page" : undefined}
            onClick={() => setCenterView("editor")}
          >
            <IconCode size={20} />
          </button>
          <button
            type="button"
            className={`activity-item${centerView === "simulator3d" ? " active" : ""}`}
            title="3D Simulator"
            aria-current={centerView === "simulator3d" ? "page" : undefined}
            onClick={() => setCenterView("simulator3d")}
          >
            <IconCube size={20} />
          </button>
        </nav>

        <aside className="workbench-sidebar workbench-scroll">
          <ExplorerPanel
            rootPath={manifest?.rootPath ?? null}
            onOpenFile={(path) => {
              setEditorOpenPath(path);
              setCenterView("editor");
            }}
          />
        </aside>

        <div className="workbench-editor-stack">
          <div className="workbench-center">
            <div className="center-content">
              {centerView === "editor" ? (
                manifest ? (
                  <EditorHost
                    openPath={editorOpenPath}
                    onOpenPathConsumed={() => setEditorOpenPath(null)}
                  />
                ) : (
                  <EmptyState
                    title="No project open"
                    hint="Open an STM32CubeIDE folder to edit, build, and run the simulator."
                    actions={
                      <>
                        <button
                          type="button"
                          className="empty-btn empty-btn-primary"
                          onClick={() => void openFolder()}
                        >
                          <IconFolder size={14} />
                          Open Folder
                        </button>
                        <button
                          type="button"
                          className="empty-btn"
                          onClick={() => void openSample()}
                        >
                          Open Sample
                        </button>
                      </>
                    }
                  />
                )
              ) : (
                <SceneView
                  gpio={gpio}
                  ledVisual={ledVisual}
                  sessionId={simSession?.sessionId ?? null}
                  onUserPress={() => void boardInteraction("buttonPress", "PA0")}
                  onUserRelease={() => void boardInteraction("buttonRelease", "PA0")}
                  onReset={() => void boardInteraction("reset", "RESET")}
                />
              )}
            </div>
          </div>

          <div className="workbench-bottom" style={{ height: bottomHeight }}>
            <div
              className="bottom-resize-handle"
              role="separator"
              onMouseDown={(e) => {
                const startY = e.clientY;
                const startH = bottomHeight;
                const onMove = (ev: MouseEvent) => {
                  setBottomHeight(
                    Math.max(120, Math.min(500, startH + (startY - ev.clientY))),
                  );
                };
                const onUp = () => {
                  window.removeEventListener("mousemove", onMove);
                  window.removeEventListener("mouseup", onUp);
                };
                window.addEventListener("mousemove", onMove);
                window.addEventListener("mouseup", onUp);
              }}
            />
            <div className="bottom-tabs">
              {(["build", "serial", "terminal"] as const).map((tab) => (
                <button
                  key={tab}
                  type="button"
                  className={bottomTab === tab ? "active" : ""}
                  onClick={() => setBottomTab(tab)}
                >
                  {tab === "build" ? "Build" : tab === "serial" ? "Serial" : "Terminal"}
                </button>
              ))}
            </div>
            <div className="bottom-panel-content">
              {bottomTab === "build" && <BuildOutputPanel />}
              {bottomTab === "serial" && <SerialTerminalPanel session={simSession} />}
              {bottomTab === "terminal" && (
                <div className="panel-stub">Terminal — stub</div>
              )}
            </div>
          </div>
        </div>
      </div>

      <footer className="workbench-statusbar">
        <span>{manifest ? manifest.projectName : "No folder"}</span>
        <span className={`build-badge-${buildBadge}`}>Build: {BUILD_LABEL[buildBadge]}</span>
        <span className={simRunning ? "status-item sim-running" : "status-item"}>
          Sim: {simSession?.status ?? "stopped"}
        </span>
        <span className="status-right">
          {toolchain?.detected
            ? `toolchain ${toolchain.version || "ok"}`
            : "toolchain missing"}
        </span>
      </footer>
    </div>
  );
}
