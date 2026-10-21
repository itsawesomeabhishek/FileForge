import React from "react";
import ReactDOM from "react-dom/client";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";

// The wheel overlay is a second window running the same bundle. Each view
// loads its own styles, so the overlay stays fully transparent.
async function mount() {
  const isWheel = getCurrentWebviewWindow().label === "wheel";
  if (isWheel) document.title = "exploit-wheel";
  const View = isWheel
    ? (await import("./WheelApp")).default
    : (await import("./App")).default;

  ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
    <React.StrictMode>
      <View />
    </React.StrictMode>,
  );
}

void mount();
