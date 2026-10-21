import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  availableTargets,
  cancelJob,
  convertFiles,
  dismissFinished,
  ffmpegHealth,
  listJobs,
  onFileDrop,
  onJobsUpdated,
  onTrayAction,
  openOutput,
  pickFiles,
  probeFiles,
  revealOutput,
  type FileInfo,
  type Health,
  type JobInfo,
  type TargetInfo,
} from "./api";
import DropZone from "./components/DropZone";
import FileList from "./components/FileList";
import JobList from "./components/JobList";
import SettingsView from "./components/SettingsView";
import TargetPicker from "./components/TargetPicker";
import "./App.css";

type Tab = "convert" | "settings";

/** Subscribes to an async Tauri listener and cleans up even under StrictMode. */
function useListener(subscribe: () => Promise<() => void>) {
  const subscribeRef = useRef(subscribe);
  subscribeRef.current = subscribe;
  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | null = null;
    subscribeRef.current().then((fn) => {
      if (disposed) fn();
      else unlisten = fn;
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);
}

export default function App() {
  const [tab, setTab] = useState<Tab>("convert");
  const [files, setFiles] = useState<FileInfo[]>([]);
  const [jobs, setJobs] = useState<JobInfo[]>([]);
  const [targets, setTargets] = useState<TargetInfo[]>([]);
  const [loadingTargets, setLoadingTargets] = useState(false);
  const [selected, setSelected] = useState<string | null>(null);
  const [hovering, setHovering] = useState(false);
  const [health, setHealth] = useState<Health | null>(null);
  const [error, setError] = useState<string | null>(null);

  const addPaths = useCallback(async (paths: string[]) => {
    if (paths.length === 0) return;
    try {
      const probed = await probeFiles(paths);
      setFiles((current) => {
        const known = new Set(current.map((file) => file.path));
        return [...current, ...probed.filter((file) => !known.has(file.path))];
      });
      setTab("convert");
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  const browse = useCallback(async () => {
    try {
      await addPaths(await pickFiles());
    } catch (e) {
      setError(String(e));
    }
  }, [addPaths]);

  useListener(() => onJobsUpdated(setJobs));
  useListener(() =>
    onTrayAction((action) => {
      if (action === "open-settings") setTab("settings");
      else void browse();
    }),
  );
  useListener(() => onFileDrop({ onHover: setHovering, onDrop: (paths) => void addPaths(paths) }));

  useEffect(() => {
    listJobs().then(setJobs).catch((e) => setError(String(e)));
    ffmpegHealth().then(setHealth).catch(() => setHealth(null));
  }, []);

  const convertible = useMemo(() => files.filter((file) => file.format !== null), [files]);
  const formatsKey = useMemo(
    () => convertible.map((file) => file.format).join(","),
    [convertible],
  );

  // Recompute the shared targets whenever the selection changes.
  useEffect(() => {
    if (convertible.length === 0) {
      setTargets([]);
      setSelected(null);
      return;
    }
    let stale = false;
    setLoadingTargets(true);
    availableTargets(convertible.map((file) => file.format as string))
      .then((result) => {
        if (stale) return;
        setTargets(result);
        setSelected((current) =>
          current && result.some((t) => t.format === current) ? current : null,
        );
      })
      .catch((e) => !stale && setError(String(e)))
      .finally(() => !stale && setLoadingTargets(false));
    return () => {
      stale = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [formatsKey]);

  const start = useCallback(async () => {
    if (!selected || convertible.length === 0) return;
    try {
      await convertFiles(
        convertible.map((file) => file.path),
        selected,
      );
      setFiles([]);
      setSelected(null);
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  }, [convertible, selected]);

  const removeFile = useCallback(
    (path: string) => setFiles((current) => current.filter((file) => file.path !== path)),
    [],
  );

  const reportError = (e: unknown) => setError(String(e));
  const healthProblem = health !== null && health.state !== "ready";

  return (
    <div className="app">
      <header className="app-header">
        <h1>exploit</h1>
        <nav className="tabs" role="tablist">
          <button
            type="button"
            role="tab"
            aria-selected={tab === "convert"}
            className={`tab${tab === "convert" ? " is-active" : ""}`}
            onClick={() => setTab("convert")}
          >
            Convert
          </button>
          <button
            type="button"
            role="tab"
            aria-selected={tab === "settings"}
            className={`tab${tab === "settings" ? " is-active" : ""}`}
            onClick={() => setTab("settings")}
          >
            Settings
          </button>
        </nav>
      </header>

      <main className="app-main">
        {error && (
          <p className="notice notice-error" role="alert">
            {error}
          </p>
        )}

        {tab === "settings" ? (
          <SettingsView />
        ) : (
          <>
            {healthProblem && (
              <p className="notice">
                FFmpeg is not ready, so audio, video, and some image conversions
                are unavailable. Open Settings to fix it.
              </p>
            )}

            <DropZone hovering={hovering} onBrowse={() => void browse()} />

            {files.length > 0 && (
              <section className="convert-panel">
                <FileList files={files} onRemove={removeFile} />
                <div className="section-header">
                  <h2>Convert to</h2>
                </div>
                <TargetPicker
                  targets={targets}
                  selected={selected}
                  loading={loadingTargets}
                  hasConvertible={convertible.length > 0}
                  onSelect={setSelected}
                />
                <div className="row row-end">
                  <button type="button" className="button" onClick={() => setFiles([])}>
                    Clear
                  </button>
                  <button
                    type="button"
                    className="button button-primary"
                    disabled={!selected || convertible.length === 0}
                    onClick={() => void start()}
                  >
                    Convert
                    {convertible.length > 1 ? ` ${convertible.length} files` : ""}
                  </button>
                </div>
              </section>
            )}

            <JobList
              jobs={jobs}
              onCancel={(id) => void cancelJob(id).catch(reportError)}
              onOpen={(path) => void openOutput(path).catch(reportError)}
              onReveal={(path) => void revealOutput(path).catch(reportError)}
              onDismiss={() => void dismissFinished().catch(reportError)}
            />
          </>
        )}
      </main>
    </div>
  );
}
