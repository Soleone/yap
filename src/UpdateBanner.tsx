import { useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { bridge, type Update } from "@/bridge";
import { Button } from "@/components/ui/button";

/**
 * Checked whenever the window comes to the front, not only at start: closing the window only
 * hides it, so yap can run for days and a release would otherwise never show.
 */
export function UpdateBanner() {
  const [update, setUpdate] = useState<Update | null>(null);
  const [installing, setInstalling] = useState(false);
  const [failure, setFailure] = useState<string | null>(null);

  useEffect(() => {
    const look = () =>
      void bridge.availableUpdate().then((found) => {
        if (found) setUpdate(found);
      });
    look();
    const unlisten = getCurrentWindow().onFocusChanged(({ payload: focused }) => {
      if (focused) look();
    });
    return () => void unlisten.then((stop) => stop());
  }, []);

  if (!update) return null;

  const install = () => {
    setInstalling(true);
    setFailure(null);
    update.install().catch((error: unknown) => {
      setInstalling(false);
      setFailure(`Couldn't update: ${String(error)}`);
    });
  };

  return (
    <div className="flex items-center justify-between gap-4 border-b bg-muted px-4 py-2">
      <span>
        {installing
          ? `Updating to Yap ${update.version}…`
          : (failure ?? `Yap ${update.version} is available`)}
      </span>
      <Button size="sm" disabled={installing} onClick={install}>
        {failure ? "Retry" : "Update"}
      </Button>
    </div>
  );
}
