import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  applyFileTool,
  availableTargets,
  convertFiles,
  listJobs,
  onJobsUpdated,
  onWheelDrag,
  onWheelHide,
  onWheelMode,
  onWheelPill,
  onWheelShow,
  probeFiles,
  wheelHide,
  type FileInfo,
  type FileTool,
  type JobInfo,
  type TargetInfo,
  type WheelMode,
  type WheelShow,
} from "./api";
import { CategoryIcon } from "./components/icons";
import { playHighlight } from "./highlightSound";
import { DISC_RADIUS, RadialGeometry, clampCenter, type Point } from "./radial";
import "./wheel.css";

interface Session {
  mode: WheelMode;
  center: Point;
  picker: boolean;
}

/** Subscribes to an async Tauri listener and cleans up even under StrictMode. */
function useListener(subscribe: () => Promise<() => void>) {
  const ref = useRef(subscribe);
  ref.current = subscribe;
  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | null = null;
    ref.current().then((fn) => {
      if (disposed) fn();
      else unlisten = fn;
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);
}

/** Dev-only ring buffer of overlay events, readable over CDP as `window.__wheelTrace`. */
function trace(event: string, detail?: unknown) {
  if (!import.meta.env.DEV) return;
  const w = window as unknown as { __wheelTrace?: unknown[] };
  const log = (w.__wheelTrace ??= []);
  log.push({ t: Date.now(), event, detail });
  if (log.length > 100) log.shift();
}

function toolsFor(files: FileInfo[] | null, pathCount: number) {
  if (files === null || files.length === 0 || files.length !== pathCount) return [];
  const category = files[0].category;
  if (!category || files.some((file) => file.category !== category || file.format === null)) return [];
  if (category === "image") {
    return [
      { id: "compress", label: "COMPRESS", category },
      { id: "metadata", label: "METADATA", category },
      { id: "background", label: "ADD BG", category },
    ];
  }
  if (category === "video") {
    return [
      { id: "compress", label: "COMPRESS", category },
      { id: "mute", label: "MUTE", category },
      { id: "snapshot", label: "SNAPSHOT", category },
      { id: "metadata", label: "METADATA", category },
    ];
  }
  if (category === "audio") {
    return [
      { id: "compress", label: "COMPRESS", category },
      { id: "normalize", label: "NORMALIZE", category },
      { id: "mono", label: "MONO", category },
      { id: "metadata", label: "METADATA", category },
      { id: "visualize", label: "VISUALIZE", category },
    ];
  }
  return [];
}

function emptyMessage(
  mode: WheelMode,
  count: number,
  unsupported: boolean,
  loading: boolean,
): { message: string; detail: string } {
  if (loading) {
    return { message: "Reading this file", detail: "The choices appear in a moment" };
  }
  if (count === 0) {
    return {
      message: mode === "tools" ? "Drop here for tools" : "Drop here to convert",
      detail: "Move the files over the wheel",
    };
  }
  if (mode === "actions") return { message: "No file actions", detail: "" };
  if (unsupported) return { message: "Selection includes unsupported files", detail: "" };
  if (mode === "tools") return { message: "No tools for this selection", detail: "" };
  return {
    message: count > 1 ? "No shared conversion for these files" : "No conversions available",
    detail: "",
  };
}

function centerFor(show: WheelShow): Point {
  const scale = window.devicePixelRatio || 1;
  return clampCenter(show.x / scale, show.y / scale, show.width / scale, show.height / scale);
}

function ProgressCard({ jobs }: { jobs: JobInfo[] }) {
  const current =
    jobs.find((job) => job.status === "running") ??
    jobs.find((job) => job.status === "waiting") ??
    jobs[jobs.length - 1];
  const finished = jobs.filter((job) => job.status === "done" || job.status === "failed").length;
  const label =
    current.status === "done"
      ? "Done"
      : current.status === "failed"
        ? "Could not finish"
        : current.status === "cancelled"
          ? "Cancelled"
          : current.status === "waiting"
            ? "Waiting to convert"
            : current.title;
  const indeterminate = current.status === "running" && current.progress < 0.02;
  const width = `${Math.max(4, Math.round(current.progress * 100))}%`;

  return (
    <div className="progress-card" role="status">
      <div className="progress-copy">
        <span className="progress-title">{label}</span>
        <span className="progress-detail">
          {current.detail}
          {jobs.length > 1 ? ` · ${finished} of ${jobs.length}` : ""}
        </span>
        {current.error && <span className="progress-error">{current.error}</span>}
      </div>
      <div className={`progress-track${indeterminate ? " is-indeterminate" : ""}`}>
        <span style={indeterminate ? undefined : { width }} />
      </div>
    </div>
  );
}

export default function WheelApp() {
  const [session, setSession] = useState<Session | null>(null);
  const [paths, setPaths] = useState<string[]>([]);
  const [files, setFiles] = useState<FileInfo[] | null>(null);
  const [targets, setTargets] = useState<TargetInfo[]>([]);
  const [targetsReady, setTargetsReady] = useState(false);
  const [hover, setHover] = useState<number | null>(null);
  const [epoch, setEpoch] = useState(0);
  const [jobs, setJobs] = useState<JobInfo[]>([]);
  const [pill, setPill] = useState(false);
  const pointer = useRef<Point | null>(null);
  const hoverRef = useRef<number | null>(null);
  const hideGen = useRef(0);
  const pendingDrop = useRef<{ paths: string[]; x: number; y: number } | null>(null);
  hoverRef.current = hover;

  const reset = useCallback(() => {
    setSession(null);
    setPaths([]);
    setFiles(null);
    setTargets([]);
    setTargetsReady(false);
    setHover(null);
    pointer.current = null;
    pendingDrop.current = null;
  }, []);

  const toolItems = toolsFor(files, paths.length);
  const items =
    session?.mode === "tools"
      ? toolItems
      : targets.map((target) => ({ id: target.format, label: target.label, category: target.category }));
  const geometry = useMemo(() => new RadialGeometry(items.length), [items.length]);
  const convertible = useMemo(() => (files ?? []).filter((file) => file.format !== null), [files]);
  const unsupported = files !== null && (files.length < paths.length || convertible.length < files.length);
  const visible = session !== null && (session.picker || paths.length > 0);

  // Probe whatever is being dragged and work out the shared targets.
  useEffect(() => {
    if (paths.length === 0) {
      setFiles(null);
      return;
    }
    let stale = false;
    probeFiles(paths)
      .then((probed) => !stale && setFiles(probed))
      .catch(() => !stale && setFiles([]));
    return () => {
      stale = true;
    };
  }, [paths]);

  useEffect(() => {
    if (files === null) {
      setTargets([]);
      setTargetsReady(false);
      return;
    }
    if (unsupported || convertible.length === 0) {
      setTargets([]);
      setTargetsReady(true);
      return;
    }
    let stale = false;
    setTargetsReady(false);
    availableTargets(convertible.map((file) => file.format as string))
      .then((result) => {
        if (stale) return;
        setTargets(result);
        setTargetsReady(true);
      })
      .catch(() => {
        if (stale) return;
        setTargets([]);
        setTargetsReady(true);
      });
    return () => {
      stale = true;
    };
  }, [files, unsupported, convertible]);

  const hitTest = useCallback(
    (point: Point | null) => {
      pointer.current = point;
      if (!point || !session) return setHover(null);
      setHover(geometry.index({ x: point.x - session.center.x, y: point.y - session.center.y }));
    },
    [geometry, session],
  );

  // Items can change under a stationary pointer.
  useEffect(() => hitTest(pointer.current), [hitTest]);

  // One tick each time the pointer or keyboard lands on a different wedge.
  useEffect(() => {
    if (hover !== null && !pill) playHighlight();
  }, [hover, pill]);

  const finish = useCallback(() => {
    reset();
    void wheelHide();
  }, [reset]);

  const choose = useCallback(
    (index: number | null, dropped: string[] = []) => {
      const item = index === null ? undefined : items[index];
      const source = paths.length > 0 ? paths : dropped;
      if (item && source.length > 0) {
        const run =
          session?.mode === "tools"
            ? applyFileTool(source, item.id as FileTool)
            : convertFiles(convertible.length > 0 ? convertible.map((file) => file.path) : source, item.id);
        run.catch((error) => console.error("wheel action failed", error));
      }
      finish();
    },
    [convertible, finish, items, paths, session?.mode],
  );

  // Latest-state handle for the long-lived drag subscription.
  const live = useRef({ session, geometry, choose, hitTest, files, targetsReady, paths });
  live.current = { session, geometry, choose, hitTest, files, targetsReady, paths };

  useEffect(() => {
    const drop = pendingDrop.current;
    if (!drop || !session || files === null) return;
    if (session.mode !== "tools" && !targetsReady) return;
    pendingDrop.current = null;
    const index = geometry.index({ x: drop.x - session.center.x, y: drop.y - session.center.y });
    choose(index, drop.paths);
  }, [files, targetsReady, session, geometry, choose]);

  useListener(() =>
    onWheelShow((show: WheelShow) => {
      trace("show", show);
      hideGen.current += 1;
      pendingDrop.current = null;
      setPill(false);
      setFiles(null);
      setTargets([]);
      setTargetsReady(false);
      setSession({ mode: show.mode, center: centerFor(show), picker: show.picker });
      setPaths(show.paths);
      setHover(null);
      setEpoch((value) => value + 1);
    }),
  );
  useListener(() =>
    onWheelMode((mode) => {
      setSession((current) => (current ? { ...current, mode } : current));
      setEpoch((value) => value + 1);
    }),
  );
  useListener(() =>
    onWheelHide(() => {
      trace("hide");
      setPill(false);
      const generation = ++hideGen.current;
      window.setTimeout(() => {
        if (hideGen.current !== generation || pendingDrop.current) return;
        reset();
      }, 1000);
    }),
  );
  useListener(() => onWheelPill(() => setPill(true)));
  useListener(() => onJobsUpdated(setJobs));
  useEffect(() => {
    listJobs().then(setJobs).catch(() => setJobs([]));
  }, []);
  useListener(() =>
    onWheelDrag({
      onEnter: (dropped, x, y) => {
        trace("enter", { dropped, x, y });
        setPaths(dropped);
        live.current.hitTest({ x, y });
      },
      onOver: (x, y) => live.current.hitTest({ x, y }),
      onLeave: () => {
        trace("leave");
        live.current.hitTest(null);
      },
      onDrop: (dropped, x, y) => {
        trace("drop", { dropped, x, y });
        const current = live.current;
        if (!current.session) return;
        const waiting =
          current.files === null || (current.session.mode !== "tools" && !current.targetsReady);
        if (waiting) {
          pendingDrop.current = { paths: dropped, x, y };
          if (current.paths.length === 0) setPaths(dropped);
          return;
        }
        const index = current.geometry.index({
          x: x - current.session.center.x,
          y: y - current.session.center.y,
        });
        current.choose(index, dropped);
      },
    }),
  );

  // Picker mode: arrows step, Enter selects, Esc or a click elsewhere cancels.
  useEffect(() => {
    if (!session?.picker) return;
    const onKey = (event: KeyboardEvent) => {
      const count = items.length;
      trace("key", { key: event.key, hover: hoverRef.current });
      if (event.key === "Escape") {
        event.preventDefault();
        finish();
      } else if (event.key === "Enter") {
        event.preventDefault();
        if (hoverRef.current !== null) choose(hoverRef.current);
      } else if (count > 0 && ["ArrowLeft", "ArrowUp", "ArrowRight", "ArrowDown"].includes(event.key)) {
        event.preventDefault();
        const delta = event.key === "ArrowLeft" || event.key === "ArrowUp" ? -1 : 1;
        const next = ((((hoverRef.current ?? -1) + delta) % count) + count) % count;
        hoverRef.current = next;
        setHover(next);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [session?.picker, items.length, choose, finish]);

  const loading = paths.length > 0 && (files === null || (session?.mode !== "tools" && !targetsReady));
  const empty = emptyMessage(session?.mode ?? "convert", paths.length, unsupported, loading);
  const detail =
    paths.length === 0
      ? empty.detail
      : paths.length === 1
        ? (files?.[0]?.name ?? "")
        : `${paths.length} files`;

  const title =
    hover !== null && items[hover]
      ? items[hover].label
      : items.length === 1
        ? items[0].label
        : session?.mode === "tools"
          ? "TOOLS"
          : "CONVERT";
  const dense = items.length > 8;

  return (
    <div
      className="wheel-root"
      onPointerMove={(event) =>
        session?.picker && hitTest({ x: event.clientX, y: event.clientY })
      }
      onPointerDown={(event) => {
        if (!session?.picker) return;
        const point = { x: event.clientX, y: event.clientY };
        const waiting = files === null || (session.mode !== "tools" && !targetsReady);
        if (waiting) {
          pendingDrop.current = { paths, x: point.x, y: point.y };
          return;
        }
        const index = geometry.index({
          x: point.x - session.center.x,
          y: point.y - session.center.y,
        });
        choose(index);
      }}
    >
      {!session && pill && jobs.length > 0 && <ProgressCard jobs={jobs} />}
      {session && visible && (
        <div
          key={epoch}
          className="wheel-disc"
          style={{
            left: session.center.x - DISC_RADIUS,
            top: session.center.y - DISC_RADIUS,
            width: DISC_RADIUS * 2,
            height: DISC_RADIUS * 2,
          }}
        >
          {items.length === 0 ? (
            <div className="wheel-empty">
              <span className="wheel-empty-message">{empty.message}</span>
              {(paths.length > 0 ? detail : empty.detail) && (
                <span className="wheel-empty-detail">{paths.length > 0 ? detail : empty.detail}</span>
              )}
            </div>
          ) : (
            <>
              <svg
                width={DISC_RADIUS * 2}
                height={DISC_RADIUS * 2}
                viewBox={`${-DISC_RADIUS} ${-DISC_RADIUS} ${DISC_RADIUS * 2} ${DISC_RADIUS * 2}`}
              >
                {items.map((item, index) => {
                  const selected = hover === index;
                  const mid = geometry.midpoint(index);
                  const push = selected ? 4 / geometry.labelRadius : 0;
                  return (
                    <path
                      key={item.id}
                      d={geometry.wedgePath(index)}
                      className={`wheel-wedge${selected ? " is-selected" : ""}`}
                      style={{
                        transform: selected
                          ? `translate(${mid.x * push}px, ${mid.y * push}px) scale(1.025)`
                          : undefined,
                      }}
                    />
                  );
                })}
              </svg>
              {items.map((item, index) => {
                const selected = hover === index;
                const mid = geometry.midpoint(index);
                const push = selected ? 4 / geometry.labelRadius : 0;
                return (
                  <div
                    key={item.id}
                    className={`wheel-label${selected ? " is-selected" : ""}${dense ? " is-dense" : ""}`}
                    style={{
                      left: DISC_RADIUS + mid.x * (1 + push),
                      top: DISC_RADIUS + mid.y * (1 + push),
                      width: geometry.labelWidth,
                    }}
                  >
                    <CategoryIcon category={item.category} size={dense ? 16 : 20} />
                    <span>{item.label}</span>
                  </div>
                );
              })}
              <div className="wheel-pill">{title}</div>
            </>
          )}
        </div>
      )}
    </div>
  );
}
