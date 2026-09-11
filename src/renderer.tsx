import { RegistryProvider } from "@effect/atom-react";
import { RouterProvider } from "@tanstack/react-router";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { getRouter } from "./router";
import "./styles.css";

const router = getRouter();
const rootElement = document.getElementById("root");

if (!rootElement) {
  throw new Error("Missal could not find the root element.");
}

createRoot(rootElement).render(
  <StrictMode>
    <RegistryProvider>
      <RouterProvider router={router} />
    </RegistryProvider>
  </StrictMode>,
);
