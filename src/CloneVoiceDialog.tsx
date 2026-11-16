import { useRef, useState } from "react";
import { AudioLines, LoaderCircle, Mic, Square } from "lucide-react";
import { bridge, type Model } from "@/bridge";
import { MAX_SECS, toClip } from "@/clip";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import {
  Field,
  FieldContent,
  FieldDescription,
  FieldError,
  FieldGroup,
  FieldLabel,
} from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Textarea } from "@/components/ui/textarea";

/** What a recording reads, so what it says is known without speech recognition. */
const LINE =
  "Hello there. I will be reading to you today, so sit back and listen for a while. Some days it is the news, some days a long story, and some days just a note from a friend.";

/** The button beside the voice that clones a new one from a recording or an audio file, and names it. */
export function CloneVoiceDialog({
  model,
  onAdded,
}: {
  /** The model that tells what an uploaded clip says. */
  model: Model;
  onAdded: (name: string) => void;
}) {
  const [open, setOpen] = useState(false);
  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger render={<Button variant="outline" size="icon" aria-label="Clone a voice" />}>
        <AudioLines />
      </DialogTrigger>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Clone a voice</DialogTitle>
          <DialogDescription>
            Read a line aloud or upload 10 to {MAX_SECS} seconds of clear speech. Every model that
            clones can read in it. Only clone voices you have permission to use.
          </DialogDescription>
        </DialogHeader>
        <CloneForm
          model={model}
          onAdded={(name) => {
            setOpen(false);
            onAdded(name);
          }}
        />
      </DialogContent>
    </Dialog>
  );
}

function CloneForm({ model, onAdded }: { model: Model; onAdded: (name: string) => void }) {
  const [name, setName] = useState("");
  /** The clip to play back. */
  const [clip, setClip] = useState<string | null>(null);
  const [transcript, setTranscript] = useState("");
  const [work, setWork] = useState<"recording" | "transcribing" | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [forget, setForget] = useState(true);
  const recorder = useRef<MediaRecorder | null>(null);

  /** Stages `blob` as the clip, saying `said`, or what speech recognition hears without it. */
  const takeClip = async (blob: Blob, said?: string) => {
    setError(null);
    setClip(null);
    try {
      await bridge.stageClip(await toClip(blob));
      setClip(URL.createObjectURL(blob));
      if (said !== undefined) return setTranscript(said);
      setWork("transcribing");
      setTranscript(await bridge.transcribeClip(model.id, !forget));
    } catch (problem) {
      setError(String(problem));
    } finally {
      setWork(null);
    }
  };

  const record = async () => {
    if (recorder.current) return recorder.current.stop();
    setError(null);
    try {
      const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
      const media = new MediaRecorder(stream);
      const chunks: Blob[] = [];
      media.ondataavailable = (event) => chunks.push(event.data);
      media.onstop = () => {
        for (const track of stream.getTracks()) track.stop();
        recorder.current = null;
        void takeClip(new Blob(chunks, { type: media.mimeType }), LINE);
      };
      media.start();
      recorder.current = media;
      setWork("recording");
      setTimeout(() => media.state === "recording" && media.stop(), MAX_SECS * 1000);
    } catch (problem) {
      setError(`The microphone didn't start: ${String(problem)}`);
    }
  };

  const add = async () => {
    try {
      await bridge.addClonedVoice(name, transcript);
      onAdded(name.trim());
    } catch (problem) {
      setError(String(problem));
    }
  };

  return (
    <FieldGroup>
      <Field>
        <FieldLabel htmlFor="clone-name">Name</FieldLabel>
        <Input id="clone-name" value={name} onChange={(event) => setName(event.target.value)} />
      </Field>
      <Tabs defaultValue="record">
        <TabsList>
          <TabsTrigger value="record" disabled={work !== null}>
            Record
          </TabsTrigger>
          <TabsTrigger value="upload" disabled={work !== null}>
            Upload
          </TabsTrigger>
        </TabsList>
        <TabsContent value="record" className="mt-3">
          <Field>
            <FieldDescription>{LINE}</FieldDescription>
            <div>
              <Button
                variant="outline"
                disabled={work === "transcribing"}
                onClick={() => void record()}
              >
                {work === "recording" ? (
                  <Square data-icon="inline-start" />
                ) : (
                  <Mic data-icon="inline-start" />
                )}
                {work === "recording" ? "Stop" : "Record"}
              </Button>
            </div>
          </Field>
        </TabsContent>
        <TabsContent value="upload" className="mt-3">
          <FieldGroup>
            <Field>
              <Input
                type="file"
                accept="audio/*"
                aria-label="Audio file"
                disabled={work !== null}
                onChange={(event) => {
                  const file = event.target.files?.[0];
                  if (file) void takeClip(file);
                }}
              />
            </Field>
            <Field orientation="horizontal">
              <Checkbox
                id="clone-forget"
                checked={forget}
                onCheckedChange={(on) => setForget(on)}
              />
              <FieldContent>
                <FieldLabel htmlFor="clone-forget">Delete Whisper after</FieldLabel>
                <FieldDescription>
                  Whisper tells what the clip says. It downloads 1.6 GB the first time and only
                  helps when cloning.
                </FieldDescription>
              </FieldContent>
            </Field>
          </FieldGroup>
        </TabsContent>
      </Tabs>
      {work === "transcribing" && (
        <p className="flex items-center gap-2 text-sm text-muted-foreground">
          <LoaderCircle className="size-4 animate-spin motion-reduce:animate-none" />
          Listening to the clip in {model.name}
        </p>
      )}
      {clip && (
        <>
          <audio controls src={clip} className="w-full" />
          <Field>
            <FieldLabel htmlFor="clone-transcript">What the clip says</FieldLabel>
            <Textarea
              id="clone-transcript"
              value={transcript}
              onChange={(event) => setTranscript(event.target.value)}
            />
            <FieldDescription>
              Fix any word that's off; the clone reads better for it.
            </FieldDescription>
          </Field>
        </>
      )}
      {error && <FieldError>{error}</FieldError>}
      <DialogFooter>
        <Button
          disabled={!name.trim() || !clip || !transcript.trim() || work !== null}
          onClick={() => void add()}
        >
          Add voice
        </Button>
      </DialogFooter>
    </FieldGroup>
  );
}
