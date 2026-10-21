import React from "react";
import ReactDOM from "react-dom/client";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";

// The wheel overlay is a second window running the same bundle. Each view
// loads its own styles, so the overlay stays fully transparent.
async function mount() {
  const label = getCurrentWebviewWindow().label;
  if (label === "wheel") document.title = "fileforge-wheel";
  if (label === "crop") document.title = "Crop";
  const View =
    label === "wheel"
      ? (await import("./WheelApp")).default
      : label === "crop"
        ? (await import("./CropApp")).default
        : (await import("./App")).default;

  ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
    <React.StrictMode>
      <View />
    </React.StrictMode>,
  );
}

void mount();
