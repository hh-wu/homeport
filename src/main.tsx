import React from "react";
import ReactDOM from "react-dom/client";

import { AppProviders } from "@/app/providers";
import App from "@/App";
import { ErrorBoundary } from "@/components/ui/error-boundary";
import { DEFAULT_LOCALE, detectLocale } from "@/i18n/config";
import { loadLocale } from "@/i18n/translate";
import "@/styles/globals.css";

function isEditableTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  if (target.isContentEditable) return true;
  return target.tagName === "INPUT" || target.tagName === "TEXTAREA";
}

function suppressNativeContextMenu() {
  document.addEventListener("contextmenu", (event) => {
    if (isEditableTarget(event.target)) return;
    event.preventDefault();
  });
}

async function start() {
  suppressNativeContextMenu();
  try {
    await loadLocale(detectLocale());
  } catch {
    await loadLocale(DEFAULT_LOCALE).catch(() => {});
  }
  ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
    <React.StrictMode>
      <ErrorBoundary>
        <AppProviders>
          <App />
        </AppProviders>
      </ErrorBoundary>
    </React.StrictMode>,
  );
}

void start();
