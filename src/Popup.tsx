import { useEffect, useRef, useState } from "react";
import { Pause, Play, X } from "lucide-react";
import { cn } from "cn";
import { bridge, type Now } from "@/bridge";
import { Button } from "@/components/ui/button";

/** About how fast the voice speaks, to move the highlight before a piece's length is known. */
const CHARS_PER_SECOND = 15;

/** The popup window: what plays, sized to its card so the screen around it stays clickable. */
export function Popup() {
  const [now, setNow] = useState<Now | null>(null);
  const card = useRef<HTMLDivElement>(null);
  const shown = now !== null;

  useEffect(() => bridge.onNow(setNow), []);

  useEffect(() => {
    const element = card.current;
    if (!element) return void bridge.fitPopup(0);
    const observer = new ResizeObserver(() => void bridge.fitPopup(element.offsetHeight));
    observer.observe(element);
    return () => observer.disconnect();
  }, [shown]);

  if (!now) return null;
  return (
    <div ref={card} className="p-2">
      <PopupCard
        now={now}
        onPause={() => void bridge.togglePause()}
        onStop={() => void bridge.stop()}
      />
    </div>
  );
}

/** The current sentence with its spoken words highlighted, or in the minimal style only where the text came from and
 * how far along. Pause and stop show on hover, and stay while paused. */
export function PopupCard({
  now,
  onPause,
  onStop,
}: {
  now: Now;
  onPause: () => void;
  onStop: () => void;
}) {
  const words = now.text.split(" ");
  const spoken = spokenWords(words, now);
  const details = [
    now.app,
    now.voice,
    now.pieces > 1 && `${now.piece + 1} of ${now.pieces}`,
  ].filter(Boolean);
  const subtitles = now.style === "subtitles";

  return (
    <div
      className={cn(
        "group flex gap-3 rounded-2xl border bg-popover pr-2 pl-4 text-popover-foreground shadow-lg",
        subtitles ? "items-start py-2.5" : "items-center py-1.5",
      )}
    >
      <div className="min-w-0 flex-1">
        <p className={cn("text-muted-foreground", subtitles ? "text-xs" : "truncate text-sm")}>
          {details.join(" · ")}
        </p>
        {subtitles && (
          <p className="mt-0.5 leading-snug">
            {words.slice(0, spoken).join(" ")}{" "}
            <span className="text-muted-foreground">{words.slice(spoken).join(" ")}</span>
          </p>
        )}
      </div>
      <div
        className={cn(
          "flex gap-0.5 transition-opacity",
          !now.paused && "opacity-0 group-hover:opacity-100",
        )}
      >
        <Button
          variant="ghost"
          size="icon-sm"
          aria-label={now.paused ? "Resume" : "Pause"}
          onClick={onPause}
        >
          {now.paused ? <Play /> : <Pause />}
        </Button>
        <Button variant="ghost" size="icon-sm" aria-label="Stop" onClick={onStop}>
          <X />
        </Button>
      </div>
    </div>
  );
}

/** How many words have started, assuming each takes time in proportion to its length. */
function spokenWords(words: string[], now: Now) {
  const length = now.length ?? now.text.length / CHARS_PER_SECOND;
  const reached = (now.elapsed / length) * now.text.length;
  let start = 0;
  let count = 0;
  for (const word of words) {
    if (start > reached) break;
    count += 1;
    start += word.length + 1;
  }
  return count;
}
