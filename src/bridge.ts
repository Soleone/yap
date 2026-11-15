import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

type PopupStyle = "subtitles" | "minimal" | "off";

/** What plays right now: the current piece (one sentence) of the read and how far into it. */
export type Now = {
  style: PopupStyle;
  text: string;
  piece: number;
  pieces: number;
  /** Seconds into the piece, and its length once all of it is generated. */
  elapsed: number;
  length: number | null;
  paused: boolean;
  voice: string;
  /** The app the text came from, when the caller said. */
  app: string;
};

export type Settings = {
  model: string;
  /** For a model with several languages. */
  language: string;
  voice: string;
  /** A multiplier; reads use the model's offered speed closest to it. */
  speed: number;
  popup: PopupStyle;
  position: "top" | "bottom";
};

/** A model from the registry. */
export type Model = {
  id: string;
  name: string;
  sizeMb: number;
  /** Language codes; the first is the default. */
  languages: string[];
  license: string;
  /** Needs a Hugging Face token whose account accepted the model's terms. */
  gated: boolean;
  /** From slowest to fastest. */
  speeds: { speed: number }[];
};

export type CatalogVoice = { name: string; language: string; description: string };

export type EngineStatus =
  | { state: "loading"; model: string }
  | { state: "ready" }
  | { state: "failed"; error: string };

export const bridge = {
  /** Called every tick while a read plays, and with `null` once nothing plays. */
  onNow(handler: (now: Now | null) => void) {
    const unlisten = listen<Now | null>("yap://now", (event) => handler(event.payload));
    return () => void unlisten.then((stop) => stop());
  },
  togglePause: () => invoke<void>("toggle_pause"),
  stop: () => invoke<void>("stop"),
  /** Sizes the popup to `height` logical pixels and shows it; 0 hides it. */
  fitPopup: (height: number) => invoke<void>("fit_popup", { height }),
  getSettings: () => invoke<Settings>("get_settings"),
  /** Saves them and gets their voice ready for the next read. */
  setSettings: (settings: Settings) => invoke<void>("set_settings", { settings }),
  models: () => invoke<Model[]>("models"),
  voices: () => invoke<CatalogVoice[]>("voices"),
  /** Reads a line in the voice and speed of the settings. */
  preview: () => invoke<void>("preview"),
  engineStatus: () => invoke<EngineStatus>("engine_status"),
  hasToken: () => invoke<boolean>("has_token"),
  /** An empty token removes the saved one. */
  setToken: (token: string) => invoke<void>("set_token", { token }),
};
