import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

type PopupStyle = "subtitles" | "minimal" | "off";

/** What plays right now: the current piece (one sentence) of the read and how far into it. */
export type Now = {
  style: PopupStyle;
  /** The read in the library; a preview has none. */
  libraryId: number | null;
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
  /** Models list by language; a model that doesn't read it reads its own first language. */
  language: string;
  voice: string;
  /** What `yap --save` makes reads in; an empty model means the reading model and voice. */
  saveModel: string;
  saveVoice: string;
  /** A multiplier; reads use the model's offered speed closest to it. */
  speed: number;
  popup: PopupStyle;
  position: "top" | "bottom";
  /** Reads the selected text of any app, like `Ctrl+Alt+R`. Empty is off. */
  hotkey: string;
  /** Reads not played or kept for this many days are deleted; 0 keeps every read. */
  keepDays: number;
  /** Holds the models and their Python runtimes; empty is the data folder. Read when the app starts. */
  modelsFolder: string;
};

/** A model from the registry. */
export type Model = {
  id: string;
  name: string;
  /** Everything it downloads, its Python runtime included. */
  sizeMb: number;
  /** Language codes; the first is the default. */
  languages: string[];
  license: string;
  /** Needs a Hugging Face token whose account accepted the model's terms. */
  gated: boolean;
  /** Needs an NVIDIA GPU and reads slower than real time, so it suits saved reads. */
  gpu: boolean;
  /** From slowest to fastest. */
  speeds: { speed: number }[];
};

/** A read in the library. */
export type Read = {
  id: number;
  /** Seconds since 1970. */
  created: number;
  title: string;
  source: { app: string; title: string; url: string };
  voice: string;
  pieces: number;
  /** Kept reads are never pruned. */
  kept: boolean;
  /** Saved for later and still being made, with how many pieces are stored so far, or why making it failed. */
  saving: boolean;
  ready: number;
  error: string;
};

/** An empty language means it reads every language of its model. */
export type Voice = { name: string; language: string; description: string };

export type EngineStatus =
  | { state: "loading"; model: string }
  | { state: "ready" }
  | { state: "failed"; error: string };

export const bridge = {
  /** Called every tick while a read plays, and with `null` once nothing plays, in every window. */
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
  voices: (model: string) => invoke<Voice[]>("voices", { model }),
  /** Reads a line in the voice and speed of the settings. */
  preview: () => invoke<void>("preview"),
  engineStatus: () => invoke<EngineStatus>("engine_status"),
  hasToken: () => invoke<boolean>("has_token"),
  /** An empty token removes the saved one. */
  setToken: (token: string) => invoke<void>("set_token", { token }),
  /** The models folder in use, which a new one in the settings replaces at the next start. */
  modelsFolder: () => invoke<string>("models_folder"),
  /** The system's folder dialog, opened at `start`; `null` when cancelled. */
  chooseFolder: (start: string) => invoke<string | null>("choose_folder", { start }),
  restart: () => invoke<void>("restart"),
  /** Newest first. */
  libraryList: () => invoke<Read[]>("library_list"),
  /** Plays a read again from piece number `from`. */
  libraryPlay: (id: number, from: number) => invoke<void>("library_play", { id, from }),
  libraryDelete: (id: number) => invoke<void>("library_delete", { id }),
  /** Keeps a read from pruning, or lets it be pruned again. */
  libraryKeep: (id: number, kept: boolean) => invoke<void>("library_keep", { id, kept }),
  /** Registers and saves the hotkey; empty turns it off. Fails when the keys are taken. */
  setHotkey: (hotkey: string) => invoke<void>("set_hotkey", { hotkey }),
  /** Turns the hotkey off while a new one is recorded, since registered keys never reach the page. */
  pauseHotkey: () => invoke<void>("pause_hotkey"),
  platform: () => invoke<"windows" | "macos" | "linux">("platform"),
};
