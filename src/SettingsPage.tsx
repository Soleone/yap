import { useEffect, useState } from "react";
import { LoaderCircle, Play } from "lucide-react";
import { disable, enable, isEnabled } from "@tauri-apps/plugin-autostart";
import { ChoiceField } from "@/ChoiceField";
import { hotkeyFromKeyDown, hotkeyKeys } from "@/hotkey";
import { bridge, type CatalogVoice, type EngineStatus, type Model, type Settings } from "@/bridge";
import { Button } from "@/components/ui/button";
import {
  Field,
  FieldContent,
  FieldDescription,
  FieldError,
  FieldGroup,
  FieldLabel,
  FieldSeparator,
  FieldSet,
} from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Kbd, KbdGroup } from "@/components/ui/kbd";
import { Slider } from "@/components/ui/slider";
import { Switch } from "@/components/ui/switch";

const LANGUAGES: Record<string, string> = {
  en: "English",
  fr: "French",
  de: "German",
  es: "Spanish",
  pt: "Portuguese",
};

/** Voice, speed, model, popup and startup. Every change is saved at once. */
export function SettingsPage() {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [models, setModels] = useState<Model[]>([]);
  const [voices, setVoices] = useState<CatalogVoice[]>([]);
  const [status, setStatus] = useState<EngineStatus>({ state: "ready" });
  const [autostart, setAutostart] = useState(false);

  useEffect(() => {
    void bridge.getSettings().then(setSettings);
    void bridge.models().then(setModels);
    // Offline before the catalog was ever downloaded, only the chosen voice shows.
    void bridge.voices().then(setVoices, () => {});
    void isEnabled().then(setAutostart);
  }, []);

  // Downloading a model can take a while, so its progress shows here.
  useEffect(() => {
    const timer = setInterval(() => void bridge.engineStatus().then(setStatus), 1000);
    return () => clearInterval(timer);
  }, []);

  if (!settings || models.length === 0) return null;

  const change = (next: Partial<Settings>) => {
    const updated = { ...settings, ...next };
    setSettings(updated);
    void bridge.setSettings(updated);
  };
  const model = models.find(({ id }) => id === settings.model) ?? models[0];
  const language = model.languages.includes(settings.language)
    ? settings.language
    : model.languages[0];
  const voiceOptions = voices.filter((voice) => voice.language.startsWith(language));
  if (!voiceOptions.some(({ name }) => name === settings.voice))
    voiceOptions.unshift({ name: settings.voice, language, description: "" });
  const description = voiceOptions.find(({ name }) => name === settings.voice)?.description;
  const speeds = model.speeds.map(({ speed }) => speed);
  const speedIndex = closestIndex(speeds, settings.speed);

  return (
    <div className="mx-auto w-full max-w-xl p-6">
      <FieldGroup>
        <FieldSet>
          <ChoiceField
            id="voice"
            label="Voice"
            value={settings.voice}
            options={voiceOptions.map(({ name }) => ({ value: name, label: name }))}
            onChange={(voice) => change({ voice })}
            description={description}
          />
          <Field>
            <FieldLabel htmlFor="speed">Speed {speeds[speedIndex].toFixed(1)}×</FieldLabel>
            <Slider
              id="speed"
              min={0}
              max={speeds.length - 1}
              step={1}
              value={speedIndex}
              onValueChange={(index) => change({ speed: speeds[index as number] })}
            />
          </Field>
          <div>
            <Button variant="outline" onClick={() => void bridge.preview()}>
              <Play data-icon="inline-start" />
              Preview
            </Button>
          </div>
        </FieldSet>

        <FieldSeparator />
        <FieldSet>
          <ChoiceField
            id="model"
            label="Model"
            value={model.id}
            options={models.map(({ id, name }) => ({ value: id, label: name }))}
            onChange={(id) => change({ model: id })}
            description={`${model.sizeMb} MB · ${model.languages.map((code) => LANGUAGES[code] ?? code).join(", ")} · ${model.license}`}
          />
          {model.languages.length > 1 && (
            <ChoiceField
              id="language"
              label="Language"
              value={language}
              options={model.languages.map((code) => ({
                value: code,
                label: LANGUAGES[code] ?? code,
              }))}
              onChange={(code) => change({ language: code })}
            />
          )}
          {model.gated && <TokenField model={model} />}
          <ModelStatus status={status} />
        </FieldSet>

        <FieldSeparator />
        <FieldSet>
          <ChoiceField
            id="popup"
            label="While reading"
            value={settings.popup}
            options={[
              { value: "subtitles", label: "Subtitles" },
              { value: "minimal", label: "Minimal" },
              { value: "off", label: "No popup" },
            ]}
            onChange={(popup) => change({ popup: popup as Settings["popup"] })}
          />
          <ChoiceField
            id="position"
            label="Position"
            value={settings.position}
            options={[
              { value: "bottom", label: "Bottom of the screen" },
              { value: "top", label: "Top of the screen" },
            ]}
            onChange={(position) => change({ position: position as Settings["position"] })}
          />
        </FieldSet>
        <FieldSeparator />
        <HotkeyField
          hotkey={settings.hotkey}
          onChange={(hotkey) => setSettings({ ...settings, hotkey })}
        />
        <FieldSeparator />
        <Field orientation="horizontal">
          <FieldContent>
            <FieldLabel htmlFor="autostart">Start with the system</FieldLabel>
            <FieldDescription>Yap waits in the tray, with the voice ready.</FieldDescription>
          </FieldContent>
          <Switch
            id="autostart"
            checked={autostart}
            onCheckedChange={(on) => void (on ? enable() : disable()).then(() => setAutostart(on))}
          />
        </Field>
      </FieldGroup>
    </div>
  );
}

/** An optional hotkey that reads the selected text of any app. Click, then press the keys. */
function HotkeyField({ hotkey, onChange }: { hotkey: string; onChange: (hotkey: string) => void }) {
  const [platform, setPlatform] = useState<string | null>(null);
  const [recording, setRecording] = useState(false);
  const [problem, setProblem] = useState<string | null>(null);

  useEffect(() => void bridge.platform().then(setPlatform), []);

  const start = () => {
    setProblem(null);
    setRecording(true);
    void bridge.pauseHotkey();
  };

  /** Registers `next`, or the current hotkey again when recording is abandoned. */
  const finish = (next: string) => {
    setRecording(false);
    bridge.setHotkey(next).then(
      () => onChange(next),
      (error: unknown) => setProblem(String(error)),
    );
  };

  const record = (event: React.KeyboardEvent) => {
    if (!recording || event.key === "Tab") return;
    event.preventDefault();
    if (event.key === "Escape") return finish(hotkey);
    const next = hotkeyFromKeyDown(event.nativeEvent);
    if (next) finish(next);
  };

  if (!platform) return null;
  if (platform === "linux")
    return (
      <Field>
        <FieldLabel>Hotkey</FieldLabel>
        <FieldDescription>
          Wayland doesn't let apps grab keys, so bind one in your window manager. In Hyprland:{" "}
          <code>bind = SUPER, R, exec, wl-paste --primary | yap</code>
        </FieldDescription>
      </Field>
    );

  const modifiers = platform === "macos" ? "Ctrl, Option or Cmd" : "Ctrl, Alt or Win";
  return (
    <Field>
      <FieldLabel id="hotkey-label">Hotkey</FieldLabel>
      <div className="flex gap-2">
        <Button
          variant="outline"
          aria-labelledby="hotkey-label"
          aria-pressed={recording}
          onClick={() => (recording ? finish(hotkey) : start())}
          onBlur={() => recording && finish(hotkey)}
          onKeyDown={record}
        >
          {recording ? (
            <span className="text-muted-foreground">Press keys…</span>
          ) : hotkey ? (
            <KbdGroup>
              {hotkeyKeys(hotkey, platform).map((key) => (
                <Kbd key={key}>{key}</Kbd>
              ))}
            </KbdGroup>
          ) : (
            "Set a hotkey"
          )}
        </Button>
        {hotkey && !recording && (
          <Button variant="ghost" onClick={() => finish("")}>
            Turn off
          </Button>
        )}
      </div>
      <FieldDescription>
        Reads the selected text of any app; pressed with nothing selected, it stops reading. Use{" "}
        {modifiers} with a letter, digit, F-key or Space. Escape keeps the current one.
      </FieldDescription>
      {problem && <FieldError>{problem}</FieldError>}
    </Field>
  );
}

/** Gated models download only with a token from an account that accepted their terms. */
function TokenField({ model }: { model: Model }) {
  const [saved, setSaved] = useState(false);
  const [token, setToken] = useState("");

  useEffect(() => void bridge.hasToken().then(setSaved), []);

  const save = async () => {
    await bridge.setToken(token);
    setSaved(token !== "");
    setToken("");
  };

  return (
    <Field>
      <FieldLabel htmlFor="token">Hugging Face token</FieldLabel>
      <div className="flex gap-2">
        <Input
          id="token"
          type="password"
          placeholder={saved ? "Saved" : "hf_…"}
          value={token}
          onChange={(event) => setToken(event.target.value)}
        />
        <Button variant="outline" onClick={() => void save()} disabled={token === "" && !saved}>
          {token === "" && saved ? "Remove" : "Save"}
        </Button>
      </div>
      <FieldDescription>
        Accept the terms at huggingface.co/gradium/{model.id} while signed in, then paste a read
        token from huggingface.co/settings/tokens.
      </FieldDescription>
    </Field>
  );
}

function ModelStatus({ status }: { status: EngineStatus }) {
  if (status.state === "loading")
    return (
      <p className="flex items-center gap-2 text-sm text-muted-foreground">
        <LoaderCircle className="size-4 animate-spin motion-reduce:animate-none" />
        Getting {status.model} ready, which downloads it the first time
      </p>
    );
  if (status.state === "failed") return <FieldError>{status.error}</FieldError>;
  return null;
}

function closestIndex(values: number[], target: number) {
  return values.reduce(
    (best, value, index) =>
      Math.abs(value - target) < Math.abs(values[best] - target) ? index : best,
    0,
  );
}
