import SettingsView from "./components/SettingsView";
import "./App.css";

/**
 * The main window is only a settings panel. Converting happens through the
 * drag wheel and the Ctrl+Alt+O picker; this window is opened from the tray.
 */
export default function App() {
  return (
    <div className="app">
      <header className="app-header">
        <h1>FileForge</h1>
        <span className="muted">Settings</span>
      </header>
      <main className="app-main">
        <SettingsView />
      </main>
    </div>
  );
}
