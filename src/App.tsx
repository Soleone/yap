import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Popup, PopupCard } from "@/Popup";
import { LibraryPage } from "@/LibraryPage";
import { SettingsPage } from "@/SettingsPage";
import type { Now } from "@/bridge";
import { ThemePicker } from "@/components/soleone/theme-picker";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";

export function App() {
  if (isTauri() && getCurrentWindow().label === "popup")
    return (
      <div data-popup className="flex h-svh flex-col justify-end">
        <Popup />
      </div>
    );
  return isTauri() ? <Main /> : <Prototype />;
}

/** The library and the settings. */
function Main() {
  return (
    <div className="min-h-svh bg-background text-foreground">
      <Tabs defaultValue="library">
        <header className="grid grid-cols-3 items-center p-4">
          <span className="font-heading font-medium">Yap</span>
          <TabsList className="justify-self-center">
            <TabsTrigger value="library">Library</TabsTrigger>
            <TabsTrigger value="settings">Settings</TabsTrigger>
          </TabsList>
          <div className="justify-self-end">
            <ThemePicker />
          </div>
        </header>
        <TabsContent value="library">
          <LibraryPage />
        </TabsContent>
        <TabsContent value="settings">
          <SettingsPage />
        </TabsContent>
      </Tabs>
    </div>
  );
}

const SAMPLE: Now = {
  style: "subtitles",
  libraryId: 1,
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
        <PopupCard now={{ ...SAMPLE, style: "minimal" }} onPause={() => {}} onStop={() => {}} />
      </main>
    </div>
  );
}
