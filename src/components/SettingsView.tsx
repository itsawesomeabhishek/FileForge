import { useCallback, useEffect, useState } from "react";
import {
  ffmpegHealth,
  getLaunchAtLogin,
  getSettings,
  pickExecutable,
  setLaunchAtLogin,
  showLogs,
  updateSettings,
  type Health,
  type OutputLocation,
  type Settings,
} from "../api";

const LOCATIONS: { value: OutputLocation; label: string }[] = [
  { value: "besideOriginal", label: "Next to the original" },
  { value: "downloads", label: "Downloads" },
  { value: "desktop", label: "Desktop" },
];

function describeHealth(health: Health | null): { text: string; ok: boolean } {
  if (!health) return { text: "Checking…", ok: true };
  switch (health.state) {
    case "ready":
      return { text: `FFmpeg ${health.version} ready`, ok: true };
    case "missing":
      return {
        text: "FFmpeg was not found. Audio, video, and some image conversions are unavailable.",
        ok: false,
      };
    case "broken":
      return { text: `FFmpeg problem: ${health.reason}`, ok: false };
  }
}

export default function SettingsView() {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [launchAtLogin, setLaunch] = useState(false);
  const [health, setHealth] = useState<Health | null>(null);
  const [pathDraft, setPathDraft] = useState("");
  const [excludedDraft, setExcludedDraft] = useState("");
  const [error, setError] = useState<string | null>(null);

  const refreshHealth = useCallback(() => {
    ffmpegHealth().then(setHealth).catch((e) => setError(String(e)));
  }, []);

  useEffect(() => {
    getSettings()
      .then((loaded) => {
        setSettings(loaded);
        setPathDraft(loaded.ffmpegPath);
        setExcludedDraft(loaded.excludedApps.join("\n"));
      })
      .catch((e) => setError(String(e)));
    getLaunchAtLogin().then(setLaunch).catch(() => setLaunch(false));
    refreshHealth();
  }, [refreshHealth]);

  const commit = useCallback(
    (next: Settings) => {
      setSettings(next);
      updateSettings(next)
        .then((saved) => {
          setSettings(saved);
          setError(null);
          if (saved.ffmpegPath !== settings?.ffmpegPath) refreshHealth();
        })
        .catch((e) => setError(String(e)));
    },
    [refreshHealth, settings?.ffmpegPath],
  );

  if (!settings) {
    return <p className="muted">{error ?? "Loading settings…"}</p>;
  }

  const status = describeHealth(health);

  return (
    <div className="settings">
      {error && <p className="notice notice-error">{error}</p>}

      <section className="card setting-group">
        <label className="setting">
          <span className="setting-label">Save converted files</span>
          <select
            value={settings.outputLocation}
            onChange={(e) =>
              commit({ ...settings, outputLocation: e.target.value as OutputLocation })
            }
          >
            {LOCATIONS.map((location) => (
              <option key={location.value} value={location.value}>
                {location.label}
              </option>
            ))}
          </select>
        </label>

        <label className="setting setting-inline">
          <input
            type="checkbox"
            checked={settings.revealInExplorer}
            onChange={(e) => commit({ ...settings, revealInExplorer: e.target.checked })}
          />
          <span className="setting-label">Show the file in Explorer when done</span>
        </label>

        <label className="setting setting-inline">
          <input
            type="checkbox"
            checked={launchAtLogin}
            onChange={(e) => {
              const enabled = e.target.checked;
              setLaunch(enabled);
              setSettings({ ...settings, launchAtLogin: enabled });
              setLaunchAtLogin(enabled)
                .then(setLaunch)
                .catch((err) => setError(String(err)));
            }}
          />
          <span className="setting-label">Start FileForge when I sign in</span>
        </label>
      </section>

      <section className="card setting-group">
        <label className="setting setting-inline">
          <input
            type="checkbox"
            checked={settings.wheelEnabled}
            onChange={(e) => commit({ ...settings, wheelEnabled: e.target.checked })}
          />
          <span className="setting-label">
            Open the wheel when I drag files from Explorer or the desktop and press Shift or Ctrl
          </span>
        </label>
        <p className="setting-note muted">
          Shift opens convert. Ctrl opens tools for images, video, and audio:
          compress, metadata, mute, snapshot, normalize, mono, and a waveform video.
          Press the key before or during the drag. The wheel stays open until you drop.
          You can also press Ctrl+Alt+O to pick files and open the convert wheel.
          While this is on, Shift+click and Ctrl+click in Explorer select only the clicked file.
        </p>
        <label className="setting">
          <span className="setting-label">Ignore drags from these apps (one .exe per line)</span>
          <textarea
            rows={3}
            value={excludedDraft}
            placeholder="explorer.exe"
            onChange={(e) => setExcludedDraft(e.target.value)}
            onBlur={() => {
              const apps = excludedDraft
                .split(/[\r\n,]+/)
                .map((app) => app.trim())
                .filter(Boolean);
              commit({ ...settings, excludedApps: apps });
            }}
          />
        </label>
      </section>

      <section className="card setting-group">
        <label className="setting">
          <span className="setting-label">
            JPEG quality: {Math.round(settings.jpegQuality * 100)}%
          </span>
          <input
            type="range"
            min={10}
            max={100}
            step={1}
            value={Math.round(settings.jpegQuality * 100)}
            onChange={(e) =>
              setSettings({ ...settings, jpegQuality: Number(e.target.value) / 100 })
            }
            onMouseUp={() => commit(settings)}
            onKeyUp={() => commit(settings)}
            onTouchEnd={() => commit(settings)}
          />
        </label>
      </section>

      <section className="card setting-group">
        <div className="setting">
          <span className="setting-label">FFmpeg</span>
          <p className={`setting-note${status.ok ? "" : " is-error"}`}>{status.text}</p>
          {health && health.state !== "missing" && (
            <p className="setting-note muted path">{health.path}</p>
          )}
          <div className="row">
            <input
              type="text"
              className="grow"
              placeholder="Automatic (bundled FFmpeg)"
              value={pathDraft}
              onChange={(e) => setPathDraft(e.target.value)}
              onBlur={() => {
                if (pathDraft.trim() !== settings.ffmpegPath) {
                  commit({ ...settings, ffmpegPath: pathDraft.trim() });
                }
              }}
            />
            <button
              type="button"
              className="button"
              onClick={() =>
                pickExecutable().then((picked) => {
                  if (picked) {
                    setPathDraft(picked);
                    commit({ ...settings, ffmpegPath: picked });
                  }
                })
              }
            >
              Browse…
            </button>
            {settings.ffmpegPath && (
              <button
                type="button"
                className="button"
                onClick={() => {
                  setPathDraft("");
                  commit({ ...settings, ffmpegPath: "" });
                }}
              >
                Reset
              </button>
            )}
          </div>
        </div>
      </section>

      <div className="row">
        <button type="button" className="button" onClick={() => showLogs().catch((e) => setError(String(e)))}>
          Show logs
        </button>
      </div>
    </div>
  );
}
