import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Popup, PopupCard } from "@/Popup";
import type { Now } from "@/bridge";
import { ThemePicker } from "@/components/soleone/theme-picker";

export function App() {
  if (isTauri() && getCurrentWindow().label === "popup")
    return (
      <div data-popup className="flex h-svh flex-col justify-end">
        <Popup />
      </div>
    );
  return isTauri() ? <Main /> : <Prototype />;
}

/** Settings and the library come here. */
function Main() {
  return (
    <div className="flex min-h-svh flex-col bg-background text-foreground">
      <header className="flex items-center justify-between p-4">
        <span className="font-heading font-medium">Yap</span>
        <ThemePicker />
      </header>
    </div>
  );
}

const SAMPLE: Now = {
  text: "Yap turns text into speech, right on this machine, and keeps everything it read.",
  piece: 1,
  pieces: 4,
  elapsed: 2.1,
  length: 4.8,
  paused: false,
  voice: "Miles",
  app: "Firefox",
};

/** `pnpm dev:web`: the windows with sample data, for UI work. */
function Prototype() {
  return (
    <div className="flex min-h-svh flex-col bg-background text-foreground">
      <header className="flex items-center justify-between p-4">
        <span className="font-heading font-medium">Yap</span>
        <ThemePicker />
      </header>
      <main className="mx-auto flex w-full max-w-120 flex-1 flex-col justify-end gap-3 p-4">
        <PopupCard now={SAMPLE} onPause={() => {}} onStop={() => {}} />
        <PopupCard now={{ ...SAMPLE, paused: true }} onPause={() => {}} onStop={() => {}} />
      </main>
    </div>
  );
}
