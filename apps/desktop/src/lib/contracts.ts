/** Hand-written TS mirrors of z6ds-core / M06 JSON contracts (camelCase). */

export const SCHEMA_VERSION = 1;

export interface AppEvent<T = unknown> {
  schemaVersion: number;
  type: string;
  source: string;
  payload: T;
  correlationId?: string;
}

export interface ProjectManifest {
  schemaVersion: number;
  rootPath: string;
  projectName: string;
  mcuId: string;
  boardId: string;
  iocPath: string;
  buildTargets: BuildTarget[];
  elfCandidates: ElfCandidate[];
}

export interface BuildTarget {
  schemaVersion: number;
  name: string;
  makefilePath: string;
  workingDirectory: string;
  artifactGlob: string;
}

export interface ElfCandidate {
  schemaVersion: number;
  path: string;
  target: string;
  mtime: number;
  sizeBytes: number;
}

export interface DiscoverRequest {
  schemaVersion: number;
  rootPath: string;
  correlationId?: string;
}

export interface DiscoveryResult {
  schemaVersion: number;
  status: "success" | "partial" | "failed";
  manifest?: ProjectManifest;
  errors: { code: string; message: string }[];
  warnings: { code: string; message: string }[];
}

export interface BuildRequest {
  schemaVersion: number;
  projectRoot: string;
  target: string;
  clean: boolean;
  environment?: Record<string, string>;
}

export interface CleanRequest {
  schemaVersion: number;
  projectRoot: string;
  target: string;
}

export interface BuildLogChunk {
  schemaVersion: number;
  buildId: string;
  stream: string;
  text: string;
}

export interface Diagnostic {
  schemaVersion: number;
  path: string;
  line: number;
  column: number;
  severity: string;
  message: string;
  source: string;
}

export interface BuildResult {
  schemaVersion: number;
  buildId: string;
  status: string;
  elfPath?: string;
  durationMs: number;
  logText: string;
  diagnostics: Diagnostic[];
  errorCode?: string;
}

export interface BuildStarted {
  schemaVersion: number;
  buildId: string;
  projectRoot: string;
  target: string;
  timestamp: string;
}

export interface ToolchainInfo {
  schemaVersion: number;
  makePath: string;
  gccPath: string;
  version: string;
  detected: boolean;
}

export interface FsEntry {
  name: string;
  path: string;
  isDir: boolean;
}

export interface BoardConfig {
  schemaVersion: number;
  mcuId: string;
  boardId: string;
  pins: unknown[];
  clock: { sysclkHz: number; apb1Hz: number; apb2Hz: number };
  uartProfiles: unknown[];
}

export interface NetlistDocument {
  schemaVersion: number;
  board: string;
  modules: unknown[];
  wires: unknown[];
  metadata?: unknown;
}

export interface ValidationResult {
  schemaVersion: number;
  valid: boolean;
  issues: { code: string; message: string }[];
}

export interface SessionOptions {
  schemaVersion: number;
  resetOnStart?: boolean;
  enableGdb?: boolean;
}

export interface SimulatorRunRequest {
  schemaVersion: number;
  elfPath: string;
  netlistRef?: string;
  boardConfig?: BoardConfig;
  sessionOptions?: SessionOptions;
}

export interface SimulatorSessionState {
  schemaVersion: number;
  sessionId: string;
  status: string;
  elfPath?: string;
  startedAt?: string;
  message?: string;
  errorCode?: string;
}

export interface BoardInteractionRequest {
  schemaVersion: number;
  sessionId: string;
  type: string;
  targetId: string;
}

export interface GpioStateMap {
  schemaVersion: number;
  sessionId: string;
  pins: Record<string, number>;
  virtualTimeNs: number;
}

export interface UartStreamChunk {
  schemaVersion: number;
  sessionId: string;
  portId: string;
  direction: string;
  bytesBase64: string;
  timestamp: string;
  virtualTimeNs: number;
}

export interface HostSendBytes {
  schemaVersion: number;
  sessionId: string;
  portId: string;
  bytesBase64: string;
}

export interface PeripheralVisualState {
  schemaVersion: number;
  instanceId: string;
  moduleType: string;
  state: { on?: boolean; distanceCm?: number };
  virtualTimeNs: number;
}

export const EventTypes = {
  projectOpened: "project.opened",
  projectRefreshed: "project.refreshed",
  buildStarted: "build.started",
  buildLog: "build.log",
  buildCompleted: "build.completed",
  fileSaved: "file.saved",
  netlistChanged: "netlist.changed",
  toolchainDetected: "toolchain.detected",
  simulatorStarting: "simulator.starting",
  simulatorStarted: "simulator.started",
  simulatorStopped: "simulator.stopped",
  simulatorError: "simulator.error",
  simulatorReset: "simulator.reset",
  gpioChanged: "gpio.changed",
  peripheralVisualChanged: "peripheral.visual.changed",
  uartRx: "uart.rx",
  uartTx: "uart.tx",
  uartBridgeConnected: "uart.bridge.connected",
  uartBridgeDisconnected: "uart.bridge.disconnected",
} as const;
