import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import App from "@/App";
import { attachConsole } from "@/lib/log";
import "@/index.css";

const container = document.getElementById("root");
if (!container) {
  throw new Error("#root not found in index.html");
}

void attachConsole();

createRoot(container).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
