import "./App.css";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect } from "react";
import LauncherApp from "./LauncherApp";
import MainApp from "./MainApp";

function App() {
  useEffect(() => {
    const preventContextMenu = (event: MouseEvent) => event.preventDefault();
    window.addEventListener("contextmenu", preventContextMenu);
    return () => window.removeEventListener("contextmenu", preventContextMenu);
  }, []);

  return getCurrentWindow().label === "launcher" ? <LauncherApp /> : <MainApp />;
}

export default App;
