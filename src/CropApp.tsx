import { useEffect, useRef, useState, type CSSProperties, type PointerEvent as ReactPointerEvent } from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { applyCrop, cropPreview, currentCrop, type CropPreview } from "./api";
import "./App.css";
import "./crop.css";

interface Rect {
  x: number;
  y: number;
  width: number;
  height: number;
}

type Handle = "move" | "n" | "s" | "e" | "w" | "nw" | "ne" | "sw" | "se";

const HANDLES: Handle[] = ["nw", "n", "ne", "e", "se", "s", "sw", "w"];

function clampRect(rect: Rect, width: number, height: number): Rect {
  const nextWidth = Math.max(1, Math.min(Math.round(rect.width), width));
  const nextHeight = Math.max(1, Math.min(Math.round(rect.height), height));
  return {
    x: Math.max(0, Math.min(Math.round(rect.x), width - nextWidth)),
    y: Math.max(0, Math.min(Math.round(rect.y), height - nextHeight)),
    width: nextWidth,
    height: nextHeight,
  };
}

function startingRect(width: number, height: number): Rect {
  const inset = Math.round(Math.min(width, height) * 0.08);
  if (width - inset * 2 < 8 || height - inset * 2 < 8) {
    return { x: 0, y: 0, width, height };
  }
  return { x: inset, y: inset, width: width - inset * 2, height: height - inset * 2 };
}

function handleStyle(handle: Handle): CSSProperties {
  const edge = (side: string) => (side === "n" || side === "w" ? "0%" : side === "s" || side === "e" ? "100%" : "50%");
  const vertical = handle.includes("n") ? "n" : handle.includes("s") ? "s" : "mid";
  const horizontal = handle.includes("w") ? "w" : handle.includes("e") ? "e" : "mid";
  const cursors: Record<Handle, string> = {
    move: "move",
    n: "ns-resize",
    s: "ns-resize",
    e: "ew-resize",
    w: "ew-resize",
    nw: "nwse-resize",
    se: "nwse-resize",
    ne: "nesw-resize",
    sw: "nesw-resize",
  };
  return {
    left: edge(horizontal),
    top: edge(vertical),
    cursor: cursors[handle],
  };
}

export default function CropApp() {
  const [path, setPath] = useState<string | null>(null);
  const [picture, setPicture] = useState<CropPreview | null>(null);
  const [rect, setRect] = useState<Rect | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const imageRef = useRef<HTMLImageElement>(null);
  const drag = useRef<{ handle: Handle; startX: number; startY: number; origin: Rect } | null>(null);

  useEffect(() => {
    let alive = true;
    const load = (file: string) => {
      setPath(file);
      setError(null);
      setPicture(null);
      cropPreview(file)
        .then((preview) => {
          if (!alive) return;
          setPicture(preview);
          setRect(startingRect(preview.width, preview.height));
        })
        .catch((reason: unknown) => {
          if (alive) setError(reason instanceof Error ? reason.message : String(reason));
        });
    };
    void currentCrop().then((file) => {
      if (file) load(file);
    });
    const unlisten = listen<string>("crop-open", (event) => load(event.payload));
    return () => {
      alive = false;
      void unlisten.then((stop) => stop());
    };
  }, []);

  const sourcePoint = (event: ReactPointerEvent) => {
    const image = imageRef.current;
    if (!image || !picture) return null;
    const box = image.getBoundingClientRect();
    if (box.width < 1 || box.height < 1) return null;
    return {
      x: ((event.clientX - box.left) / box.width) * picture.width,
      y: ((event.clientY - box.top) / box.height) * picture.height,
      slop: (14 / box.width) * picture.width,
    };
  };

  const hitHandle = (x: number, y: number, current: Rect, slop: number): Handle => {
    const points: [Handle, number, number][] = [
      ["nw", current.x, current.y],
      ["n", current.x + current.width / 2, current.y],
      ["ne", current.x + current.width, current.y],
      ["e", current.x + current.width, current.y + current.height / 2],
      ["se", current.x + current.width, current.y + current.height],
      ["s", current.x + current.width / 2, current.y + current.height],
      ["sw", current.x, current.y + current.height],
      ["w", current.x, current.y + current.height / 2],
    ];
    const found = points.find(([, px, py]) => Math.hypot(x - px, y - py) <= slop);
    return found ? found[0] : "move";
  };

  const onPointerDown = (event: ReactPointerEvent) => {
    if (!rect || !picture) return;
    const point = sourcePoint(event);
    if (!point) return;
    drag.current = {
      handle: hitHandle(point.x, point.y, rect, point.slop),
      startX: point.x,
      startY: point.y,
      origin: rect,
    };
    event.currentTarget.setPointerCapture(event.pointerId);
  };

  const onPointerMove = (event: ReactPointerEvent) => {
    const active = drag.current;
    if (!active || !picture) return;
    const point = sourcePoint(event);
    if (!point) return;
    const dx = point.x - active.startX;
    const dy = point.y - active.startY;
    const next = { ...active.origin };
    const handle = active.handle;
    if (handle === "move") {
      next.x += dx;
      next.y += dy;
    } else {
      if (handle.includes("w")) {
        next.x += dx;
        next.width -= dx;
      }
      if (handle.includes("e")) next.width += dx;
      if (handle.includes("n")) {
        next.y += dy;
        next.height -= dy;
      }
      if (handle.includes("s")) next.height += dy;
    }
    setRect(clampRect(next, picture.width, picture.height));
  };

  const onPointerUp = () => {
    drag.current = null;
  };

  const close = () => {
    void getCurrentWebviewWindow().close();
  };

  const save = () => {
    if (!path || !rect || saving) return;
    setSaving(true);
    applyCrop(path, rect.x, rect.y, rect.width, rect.height)
      .then(close)
      .catch((reason: unknown) => {
        setSaving(false);
        setError(reason instanceof Error ? reason.message : String(reason));
      });
  };

  const setField = (field: keyof Rect, value: string) => {
    if (!picture || !rect) return;
    const number = Number(value);
    if (!Number.isFinite(number)) return;
    setRect(clampRect({ ...rect, [field]: number }, picture.width, picture.height));
  };

  return (
    <div className="crop-app">
      <header className="crop-bar">
        <div>
          <h1>Crop</h1>
          <p>
            {picture?.video
              ? "Drag the box. The same area is kept in every frame."
              : "Drag the box or type the pixels you want to keep."}
          </p>
        </div>
        <div className="crop-actions">
          <button className="button" type="button" onClick={close}>
            Cancel
          </button>
          <button className="button button-primary" type="button" onClick={save} disabled={!rect || saving}>
            Save crop
          </button>
        </div>
      </header>
      <div className="crop-stage">
        {picture && rect && (
          <div className="crop-frame" onPointerDown={onPointerDown} onPointerMove={onPointerMove} onPointerUp={onPointerUp}>
            <img ref={imageRef} src={`data:image/png;base64,${picture.pngBase64}`} alt="" draggable={false} />
            <div
              className="crop-box"
              style={{
                left: `${(rect.x / picture.width) * 100}%`,
                top: `${(rect.y / picture.height) * 100}%`,
                width: `${(rect.width / picture.width) * 100}%`,
                height: `${(rect.height / picture.height) * 100}%`,
              }}
            >
              {HANDLES.map((handle) => (
                <span key={handle} className="crop-handle" style={handleStyle(handle)} />
              ))}
            </div>
          </div>
        )}
      </div>
      {rect && (
        <div className="crop-fields">
          {(["x", "y", "width", "height"] as const).map((field) => (
            <label key={field}>
              {field.toUpperCase()}
              <input
                type="number"
                min={field === "width" || field === "height" ? 1 : 0}
                value={rect[field]}
                onChange={(event) => setField(field, event.target.value)}
              />
            </label>
          ))}
        </div>
      )}
      {error && <p className="crop-error">{error}</p>}
    </div>
  );
}
