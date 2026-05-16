import { IconHammer, IconPlay, IconStop } from "../components/Icons";
import "./WorkbenchToolbar.css";

export interface WorkbenchToolbarProps {
  projectName: string | null;
  hasProject: boolean;
  simRunning: boolean;
  simError?: string | null;
  onOpenFolder: () => void;
  onOpenSample: () => void;
  onBuild: () => void;
  onClean: () => void;
  onRunSim: () => void;
  onStopSim: () => void;
  onOpenMainC: () => void;
}

export function WorkbenchToolbar({
  projectName,
  hasProject,
  simRunning,
  simError,
  onOpenFolder,
  onOpenSample,
  onBuild,
  onClean,
  onRunSim,
  onStopSim,
  onOpenMainC,
}: WorkbenchToolbarProps) {
  return (
    <header className="workbench-toolbar">
      <div className="toolbar-brand">
        <img
          className="toolbar-logo"
          src="/logo.png"
          srcSet="/logo.png 1x, /logo@2x.png 2x"
          width={26}
          height={26}
          alt=""
        />
        <span className="toolbar-product">z6ds</span>
        {projectName && (
          <>
            <span className="toolbar-sep" aria-hidden>
              /
            </span>
            <span className="toolbar-project" title={projectName}>
              {projectName}
            </span>
          </>
        )}
      </div>

      <div className="toolbar-groups">
        <div className="toolbar-group" role="group" aria-label="Project">
          <button type="button" className="toolbar-btn toolbar-btn-ghost" onClick={onOpenFolder}>
            Open Folder
          </button>
          <button type="button" className="toolbar-btn toolbar-btn-ghost" onClick={onOpenSample}>
            Sample
          </button>
        </div>

        <span className="toolbar-divider" aria-hidden />

        <div className="toolbar-group" role="group" aria-label="Build">
          <button
            type="button"
            className="toolbar-btn toolbar-btn-ghost"
            disabled={!hasProject}
            onClick={onBuild}
          >
            <IconHammer size={14} />
            Build
          </button>
          <button
            type="button"
            className="toolbar-btn toolbar-btn-ghost"
            disabled={!hasProject}
            onClick={onClean}
          >
            Clean
          </button>
        </div>

        <span className="toolbar-divider" aria-hidden />

        <div className="toolbar-group" role="group" aria-label="Simulator">
          <button
            type="button"
            className="toolbar-btn toolbar-btn-primary"
            disabled={!hasProject || simRunning}
            onClick={onRunSim}
          >
            <IconPlay size={14} />
            Run Sim
          </button>
          <button
            type="button"
            className="toolbar-btn toolbar-btn-ghost"
            disabled={!simRunning}
            onClick={onStopSim}
          >
            <IconStop size={14} />
            Stop
          </button>
        </div>
      </div>

      <div className="toolbar-trailing">
        {simError && (
          <span className="toolbar-error" title={simError}>
            {simError}
          </span>
        )}
        {hasProject && (
          <button
            type="button"
            className="toolbar-btn toolbar-btn-ghost toolbar-btn-main"
            onClick={onOpenMainC}
          >
            main.c
          </button>
        )}
      </div>
    </header>
  );
}
