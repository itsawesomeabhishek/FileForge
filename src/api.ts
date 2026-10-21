import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open } from "@tauri-apps/plugin-dialog";

// Types mirror the Rust command payloads (camelCase via serde).

export type Category =
  | "image"
  | "audio"
  | "video"
  | "document"
  | "subtitle"
  | "archive";

export interface FileInfo {
  path: string;
  name: string;
  format: string | null;
  category: Category | null;
}

export interface TargetInfo {
  format: string;
  label: string;
  category: Category;
}

export type OutputLocation = "besideOriginal" | "downloads" | "desktop";

export interface Settings {
  outputLocation: OutputLocation;
  revealInExplorer: boolean;
  jpegQuality: number;
  ffmpegPath: string;
  wheelEnabled: boolean;
  excludedApps: string[];
  launchAtLogin: boolean;
}

export type Health =
  | { state: "missing" }
  | { state: "broken"; path: string; reason: string }
  | { state: "ready"; path: string; version: string };

export const probeFiles = (paths: string[]) =>
  invoke<FileInfo[]>("probe_files", { paths });

export const availableTargets = (formats: string[]) =>
  invoke<TargetInfo[]>("available_targets", { formats });

export const convertFiles = (paths: string[], target: string) =>
  invoke<void>("convert_files", { paths, target });

export type JobStatus = "waiting" | "running" | "done" | "cancelled" | "failed";

export interface JobInfo {
  id: string;
  title: string;
  detail: string;
  category: Category | null;
  status: JobStatus;
  progress: number;
  error: string | null;
  outputs: string[];
}

export const listJobs = () => invoke<JobInfo[]>("list_jobs");

export const onJobsUpdated = (handler: (jobs: JobInfo[]) => void): Promise<UnlistenFn> =>
  listen<JobInfo[]>("jobs-updated", (event) => handler(event.payload));

export const getSettings = () => invoke<Settings>("get_settings");
export const updateSettings = (settings: Settings) =>
  invoke<Settings>("update_settings", { settings });

export const getLaunchAtLogin = () => invoke<boolean>("get_launch_at_login");
export const setLaunchAtLogin = (enabled: boolean) =>
  invoke<boolean>("set_launch_at_login", { enabled });

export const ffmpegHealth = () => invoke<Health>("ffmpeg_health");
export const showLogs = () => invoke<void>("show_logs");
export const wheelHide = () => invoke<void>("wheel_hide");

export async function pickExecutable(): Promise<string | null> {
  const result = await open({
    multiple: false,
    directory: false,
    filters: [{ name: "Executable", extensions: ["exe"] }],
  });
  return typeof result === "string" ? result : null;
}

// ---- Wheel overlay ----


export type WheelMode = "convert" | "tools" | "actions";

export interface WheelShow {
  mode: WheelMode;
  /** Wheel centre in CSS pixels relative to the overlay window. */
  x: number;
  y: number;
  /** Opened from the menu/hotkey rather than by a drag. */
  picker: boolean;
  paths: string[];
}

export const onWheelShow = (handler: (show: WheelShow) => void): Promise<UnlistenFn> =>
  listen<WheelShow>("wheel-show", (event) => handler(event.payload));

export const onWheelMode = (handler: (mode: WheelMode) => void): Promise<UnlistenFn> =>
  listen<WheelMode>("wheel-mode", (event) => handler(event.payload));

export const onWheelHide = (handler: () => void): Promise<UnlistenFn> =>
  listen("wheel-hide", () => handler());

export const onWheelPill = (handler: () => void): Promise<UnlistenFn> =>
  listen("wheel-pill", () => handler());

export interface WheelDragHandlers {
  onEnter: (paths: string[], x: number, y: number) => void;
  onOver: (x: number, y: number) => void;
  onLeave: () => void;
  onDrop: (paths: string[], x: number, y: number) => void;
}

/** Drag events over the overlay; positions are converted to CSS pixels. */
export const onWheelDrag = (handlers: WheelDragHandlers): Promise<UnlistenFn> =>
  getCurrentWebview().onDragDropEvent((event) => {
    const payload = event.payload;
    if (payload.type === "leave") {
      handlers.onLeave();
      return;
    }
    const scale = window.devicePixelRatio || 1;
    const x = payload.position.x / scale;
    const y = payload.position.y / scale;
    if (payload.type === "enter") handlers.onEnter(payload.paths, x, y);
    else if (payload.type === "over") handlers.onOver(x, y);
    else handlers.onDrop(payload.paths, x, y);
  });