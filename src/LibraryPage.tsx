import { useCallback, useEffect, useState } from "react";
import { Play, Square, Trash2 } from "lucide-react";
import { bridge, type Now, type Read } from "@/bridge";
import { Button } from "@/components/ui/button";
import { Kbd } from "@/components/ui/kbd";
import { Slider } from "@/components/ui/slider";

/** Everything yap read, newest first, to play again from any sentence or delete. */
export function LibraryPage() {
  const [reads, setReads] = useState<Read[] | null>(null);
  const [now, setNow] = useState<Now | null>(null);
  const refresh = useCallback(() => void bridge.libraryList().then(setReads), []);

  useEffect(() => {
    refresh();
    window.addEventListener("focus", refresh);
    return () => window.removeEventListener("focus", refresh);
  }, [refresh]);

  useEffect(() => bridge.onNow(setNow), []);

  // A read started from elsewhere, like the yap command, shows up once it plays.
  const playing = now?.libraryId;
  const known = reads?.some(({ id }) => id === playing) ?? true;
  useEffect(() => {
    if (playing != null && !known) refresh();
  }, [playing, known, refresh]);

  if (!reads) return null;
  if (reads.length === 0)
    return (
      <p className="p-6 text-center text-muted-foreground">
        Nothing read yet. Run <Kbd>yap "Hello"</Kbd> in a terminal.
      </p>
    );
  return (
    <ul className="mx-auto flex w-full max-w-xl flex-col divide-y p-6">
      {reads.map((read) => (
        <ReadRow
          key={read.id}
          read={read}
          now={now?.libraryId === read.id ? now : null}
          onDelete={() => void bridge.libraryDelete(read.id).then(refresh)}
        />
      ))}
    </ul>
  );
}

function ReadRow({ read, now, onDelete }: { read: Read; now: Now | null; onDelete: () => void }) {
  const [seeking, setSeeking] = useState<number | null>(null);
  const when = new Date(read.created * 1000).toLocaleString(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  });

  return (
    <li className="group flex flex-col gap-2 py-3">
      <div className="flex items-center gap-3">
        <Button
          variant="ghost"
          size="icon-sm"
          aria-label={now ? "Stop" : "Play"}
          onClick={() => void (now ? bridge.stop() : bridge.libraryPlay(read.id, 0))}
        >
          {now ? <Square /> : <Play />}
        </Button>
        <div className="min-w-0 flex-1">
          <p className="truncate">{read.title}</p>
          <p className="truncate text-xs text-muted-foreground">
            {[read.source.app, read.voice, when].filter(Boolean).join(" · ")}
          </p>
        </div>
        <div className="opacity-0 transition-opacity group-hover:opacity-100 focus-within:opacity-100">
          <Button variant="ghost" size="icon-sm" aria-label="Delete" onClick={onDelete}>
            <Trash2 />
          </Button>
        </div>
      </div>
      {now && read.pieces > 1 && (
        <Slider
          aria-label="Sentence"
          min={0}
          max={read.pieces - 1}
          step={1}
          value={seeking ?? now.piece}
          onValueChange={(piece) => setSeeking(piece as number)}
          onValueCommitted={(piece) => {
            setSeeking(null);
            void bridge.libraryPlay(read.id, piece as number);
          }}
        />
      )}
    </li>
  );
}
