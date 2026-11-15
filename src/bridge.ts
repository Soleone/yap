import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

/** What plays right now: the current piece (one sentence) of the read and how far into it. */
export type Now = {
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
};
