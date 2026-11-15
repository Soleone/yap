import { useEffect, useState } from "react";
import { bridge, type Update } from "@/bridge";
import { Button } from "@/components/ui/button";

/** Checked once per start; yap keeps running in the tray, so the window may open much later. */
export function UpdateBanner() {
  const [update, setUpdate] = useState<Update | null>(null);
  const [installing, setInstalling] = useState(false);
  const [failure, setFailure] = useState<string | null>(null);

  useEffect(() => void bridge.availableUpdate().then(setUpdate), []);

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
