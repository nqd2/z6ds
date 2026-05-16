import { invoke } from "@tauri-apps/api/core";
import type {
  BoardConfig,
  BoardInteractionRequest,
  BuildRequest,
  BuildResult,
  CleanRequest,
  DiscoverRequest,
  DiscoveryResult,
  FsEntry,
  HostSendBytes,
  NetlistDocument,
  ProjectManifest,
  SimulatorRunRequest,
  SimulatorSessionState,
  ToolchainInfo,
  ValidationResult,
} from "./contracts";
import { SCHEMA_VERSION } from "./contracts";

export async function getSampleProjectRoot(): Promise<string> {
  return invoke<string>("get_sample_project_root");
}

export async function discoverProject(rootPath: string): Promise<DiscoveryResult> {
  const request: DiscoverRequest = {
    schemaVersion: SCHEMA_VERSION,
    rootPath,
  };
  return invoke<DiscoveryResult>("discover_project", { request });
}

export async function getProjectManifest(): Promise<ProjectManifest | null> {
  return invoke<ProjectManifest | null>("get_project_manifest");
}

export async function fsListDir(path: string): Promise<FsEntry[]> {
  return invoke<FsEntry[]>("fs_list_dir", { path });
}

export async function fsReadFile(path: string): Promise<string> {
  return invoke<string>("fs_read_file", { path });
}

export async function fsWriteFile(path: string, contents: string): Promise<void> {
  return invoke("fs_write_file", { path, contents });
}

export async function buildProject(
  projectRoot: string,
  target = "Debug",
): Promise<BuildResult> {
  const request: BuildRequest = {
    schemaVersion: SCHEMA_VERSION,
    projectRoot,
    target,
    clean: false,
  };
  return invoke<BuildResult>("build_project", { request });
}

export async function cleanProject(
  projectRoot: string,
  target = "Debug",
): Promise<BuildResult> {
  const request: CleanRequest = {
    schemaVersion: SCHEMA_VERSION,
    projectRoot,
    target,
  };
  return invoke<BuildResult>("clean_project", { request });
}

export async function detectToolchain(): Promise<ToolchainInfo> {
  return invoke<ToolchainInfo>("detect_toolchain");
}

export async function parseBoardConfig(iocPath: string): Promise<BoardConfig> {
  return invoke<BoardConfig>("parse_board_config", { iocPath });
}

export async function getNetlist(): Promise<NetlistDocument> {
  return invoke<NetlistDocument>("get_netlist");
}

export async function applyNetlistDefaults(
  boardConfig: BoardConfig,
): Promise<NetlistDocument> {
  return invoke<NetlistDocument>("apply_netlist_defaults", { boardConfig });
}

export async function validateNetlist(
  rules?: string[],
): Promise<ValidationResult> {
  return invoke<ValidationResult>("validate_netlist_cmd", { rules });
}

export async function resolveSimElf(
  manifestElf?: string,
  projectRoot?: string,
): Promise<string> {
  return invoke<string>("resolve_sim_elf", {
    manifestElf: manifestElf ?? null,
    projectRoot: projectRoot ?? null,
  });
}

export async function runSimulator(
  request: SimulatorRunRequest,
): Promise<SimulatorSessionState> {
  return invoke<SimulatorSessionState>("run_simulator", { request });
}

export async function stopSimulator(): Promise<SimulatorSessionState> {
  return invoke<SimulatorSessionState>("stop_simulator");
}

export async function resetSimulator(): Promise<void> {
  return invoke("reset_simulator");
}

export async function getSimulatorState(): Promise<SimulatorSessionState> {
  return invoke<SimulatorSessionState>("get_simulator_state");
}

export async function handleBoardInteraction(
  request: BoardInteractionRequest,
): Promise<void> {
  return invoke("handle_board_interaction", { request });
}

export async function hostSendUart(request: HostSendBytes): Promise<void> {
  return invoke("host_send_uart", { request });
}

export async function labBoardConfig(): Promise<BoardConfig> {
  return invoke<BoardConfig>("lab_board_config");
}
