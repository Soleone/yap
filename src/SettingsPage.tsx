import { useEffect, useState } from "react";
import { LoaderCircle, Play } from "lucide-react";
import { disable, enable, isEnabled } from "@tauri-apps/plugin-autostart";
import { ChoiceField } from "@/ChoiceField";
import { hotkeyFromKeyDown, hotkeyKeys } from "@/hotkey";
import { bridge, type EngineStatus, type Model, type Settings, type Voice } from "@/bridge";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
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

const LANGUAGES: Record<string, string> = {
  en: "English",
  fr: "French",
  de: "German",
  es: "Spanish",
  pt: "Portuguese",
  it: "Italian",
  ru: "Russian",
  zh: "Chinese",
  ja: "Japanese",
  ko: "Korean",
  hi: "Hindi",
};

const KEEP_DAYS = [7, 30, 90, 365, 0];

/** The saved-reads choice that means the reading model and voice. */
const SAME = "same";

/** Voice, speed, model, saved reads, history, popup and startup. Every change is saved at once. */
export function SettingsPage() {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [models, setModels] = useState<Model[]>([]);
  const [voices, setVoices] = useState<Record<string, Voice[]>>({});
  const [status, setStatus] = useState<EngineStatus>({ state: "ready" });
  const [autostart, setAutostart] = useState(false);

  useEffect(() => {
    void bridge.getSettings().then(setSettings);
    void bridge.models().then(setModels);
    void isEnabled().then(setAutostart);
  }, []);

  // Offline before Phonon's catalog was ever downloaded, only the chosen voice shows.
  const voicesOf = (id: string) =>
    bridge.voices(id).then(
      (list) => {
        setVoices((known) => ({ ...known, [id]: list }));
        return list;
      },
      () => [],
    );
  const readModel = settings?.model;
  const saveModel = settings?.saveModel;
  useEffect(() => {
    for (const id of [readModel, saveModel]) if (id) void voicesOf(id);
  }, [readModel, saveModel]);

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
  const languages = [...new Set(models.flatMap((each) => each.languages))];
  const languageModels = models.filter((each) => each.languages.includes(language));
  const saving = models.find(({ id }) => id === settings.saveModel);

  /** `current` when `id` has it in `code`, else the first voice that fits. */
  const fitVoice = async (id: string, code: string, current: string) => {
    const fitting = (await voicesOf(id)).filter((voice) => reads(voice, code));
    return fitting.some(({ name }) => name === current) ? current : (fitting[0]?.name ?? current);
  };
  const chooseLanguage = async (code: string) => {
    const reading = model.languages.includes(code)
      ? model
      : models.find((each) => each.languages.includes(code))!;
    const keepsSaving = saving?.languages.includes(code);
    change({
      language: code,
      model: reading.id,
      voice: await fitVoice(reading.id, code, settings.voice),
      saveModel: keepsSaving ? settings.saveModel : "",
      saveVoice: keepsSaving ? await fitVoice(saving!.id, code, settings.saveVoice) : "",
    });
  };
  const chooseModel = async (id: string) =>
    change({ model: id, voice: await fitVoice(id, language, settings.voice) });
  const chooseSaveModel = async (id: string) =>
    id === SAME
      ? change({ saveModel: "", saveVoice: "" })
      : change({ saveModel: id, saveVoice: await fitVoice(id, language, settings.saveVoice) });

  const speeds = model.speeds.map(({ speed }) => speed);
  const speedIndex = closestIndex(speeds, settings.speed);

  return (
    <div className="mx-auto w-full max-w-xl p-6">
      <FieldGroup>
        <FieldSet>
          <VoiceField
            id="voice"
            voices={voices[model.id] ?? []}
            language={language}
            value={settings.voice}
            onChange={(voice) => change({ voice })}
          />
          {speeds.length > 1 && (
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
          )}
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
            id="language"
            label="Language"
            value={language}
            options={languages.map((code) => ({ value: code, label: languageName(code) }))}
            onChange={(code) => void chooseLanguage(code)}
          />
          <ChoiceField
            id="model"
            label="Model"
            value={model.id}
            options={modelOptions(languageModels)}
            onChange={(id) => void chooseModel(id)}
            description={modelDescription(model, language)}
          />
          {model.gated && <TokenField model={model} />}
          <ModelStatus status={status} />
          <ModelsFolderField
            chosen={settings.modelsFolder}
            onChange={(modelsFolder) => change({ modelsFolder })}
          />
        </FieldSet>

        <FieldSeparator />
        <FieldSet>
          <ChoiceField
            id="save-model"
            label="Saved reads"
            value={saving?.id ?? SAME}
            options={[{ value: SAME, label: "Same as reading" }, ...modelOptions(languageModels)]}
            onChange={(id) => void chooseSaveModel(id)}
            description={
              saving ? (
                modelDescription(saving, language)
              ) : (
                <>
                  <code>yap --save "text"</code> makes the audio in the background without playing
                  it; it plays from the library once ready.
                </>
              )
            }
          />
          {saving && (
            <VoiceField
              id="save-voice"
              voices={voices[saving.id] ?? []}
              language={language}
              value={settings.saveVoice}
              onChange={(saveVoice) => change({ saveVoice })}
            />
          )}
        </FieldSet>

        <FieldSeparator />
        <ChoiceField
          id="keep-days"
          label="Keep reads"
          value={String(settings.keepDays)}
          options={KEEP_DAYS.map((days) => ({
            value: String(days),
            label: days === 0 ? "Forever" : `For ${days} days`,
          }))}
          onChange={(days) => change({ keepDays: Number(days) })}
          description="Counted from the last play. Reads kept in the library stay either way."
        />

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
          <Checkbox
            id="autostart"
            checked={autostart}
            onCheckedChange={(on) => void (on ? enable() : disable()).then(() => setAutostart(on))}
          />
          <FieldContent>
            <FieldLabel htmlFor="autostart">Start with the system</FieldLabel>
            <FieldDescription>Yap waits in the tray, with the voice ready.</FieldDescription>
          </FieldContent>
        </Field>
      </FieldGroup>
    </div>
  );
}

/** A model's voices that read `language`, with the chosen one's description. */
function VoiceField({
  id,
  voices,
  language,
  value,
  onChange,
}: {
  id: string;
  voices: Voice[];
  language: string;
  value: string;
  onChange: (voice: string) => void;
}) {
  const options = voices.filter((voice) => reads(voice, language));
  if (!options.some(({ name }) => name === value))
    options.unshift({ name: value, language, description: "" });
  return (
    <ChoiceField
      id={id}
      label="Voice"
      value={value}
      options={options.map(({ name }) => ({ value: name, label: name }))}
      onChange={onChange}
      description={options.find(({ name }) => name === value)?.description}
    />
  );
}

function reads(voice: Voice, language: string) {
  return voice.language === "" || voice.language.startsWith(language);
}

function languageName(code: string) {
  return LANGUAGES[code] ?? code;
}

/** Each model with what it downloads, so the disk it takes shows while choosing. */
function modelOptions(models: Model[]) {
  return models.map(({ id, name, sizeMb }) => ({
    value: id,
    label: `${name} · ${sizeMb < 1000 ? `${sizeMb} MB` : `${Math.round(sizeMb / 1000)} GB`}`,
  }));
}

function modelDescription(model: Model, language: string) {
  const others = model.languages.filter((code) => code !== language);
  return [
    others.length > 0 && `Also reads ${others.map(languageName).join(", ")}`,
    model.gpu && "Needs an NVIDIA GPU and reads slower than real time, so it suits saved reads",
    model.license,
  ]
    .filter(Boolean)
    .join(" · ");
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

/** Where models download. Yap switches to a new folder when it starts, so until then a restart is offered. */
function ModelsFolderField({
  chosen,
  onChange,
}: {
  chosen: string;
  onChange: (folder: string) => void;
}) {
  const [inUse, setInUse] = useState<string | null>(null);

  useEffect(() => void bridge.modelsFolder().then(setInUse), []);

  if (inUse === null) return null;
  const folder = chosen || inUse;
  const choose = async () => {
    const picked = await bridge.chooseFolder(folder);
    if (picked) onChange(picked);
  };

  return (
    <Field>
      <FieldLabel htmlFor="models-folder">Models folder</FieldLabel>
      <div className="flex gap-2">
        <Input id="models-folder" readOnly value={folder} />
        <Button variant="outline" onClick={() => void choose()}>
          Choose…
        </Button>
      </div>
      {folder === inUse ? (
        <FieldDescription>
          Models and their Python runtimes download here. Ones already downloaded stay where they
          are.
        </FieldDescription>
      ) : (
        <div className="flex items-center justify-between gap-2">
          <FieldDescription>Yap switches to it when it restarts.</FieldDescription>
          <Button variant="outline" size="sm" onClick={() => void bridge.restart()}>
            Restart now
          </Button>
        </div>
      )}
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
