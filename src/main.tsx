import React from "react";
import ReactDOM from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import App from "./App";
import PetBubble from "./components/PetBubble";
import "./App.css";

// 气泡跑在第二个窗口（label = "bubble"）里，共用同一份前端资源、靠 label 分流：
// 它只画气泡，不加载编辑器/侧栏，也不碰主窗那份 zustand store（跨窗不共享状态）。
function windowLabel(): string {
  try {
    return getCurrentWindow().label;
  } catch {
    return "main"; // 非 Tauri 环境（纯浏览器预览）
  }
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>{windowLabel() === "bubble" ? <PetBubble /> : <App />}</React.StrictMode>
);
