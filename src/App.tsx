import "./App.css";
import { getCurrentWindow } from "@tauri-apps/api/window";
import LauncherApp from "./LauncherApp";
import MainApp from "./MainApp";

function App() {
  return getCurrentWindow().label === "launcher" ? <LauncherApp /> : <MainApp />;
}

export default App;
