import { createRoot } from "react-dom/client";
import "@fontsource/manrope/latin-400.css";
import "@fontsource/manrope/latin-700.css";
import "@fontsource/manrope/latin-800.css";
import "@fontsource/dm-sans/latin-400.css";
import "@fontsource/dm-sans/latin-500.css";
import "@fontsource/dm-sans/latin-700.css";
import App from "./App.tsx";
import { installErrorReporting } from "./lib/errorReport.ts";
import { logInterfaceError } from "./lib/aether.ts";
import "./globals.css";

// Uncaught errors go to the local log (a few, once each), so "it went blank" leaves something to read.
installErrorReporting(window, logInterfaceError);
createRoot(document.getElementById("root")!).render(<App />);
