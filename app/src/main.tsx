import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
// first, so component styles can override the base ones
import "./styles/global.css";
import App from "./App";
import { connectStore } from "./store";

connectStore().catch((e) => console.error("could not reach the Rust side", e));

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);

