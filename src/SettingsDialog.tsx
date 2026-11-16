import { useEffect, useState } from "react";
import {
  AudioLines,
  Cpu,
  Gauge,
  LoaderCircle,
  Play,
  Plus,
  Settings2,
  Star,
  Trash2,
  X,
} from "lucide-react";
import { disable, enable, isEnabled } from "@tauri-apps/plugin-autostart";
import { ChoiceField } from "@/ChoiceField";
import { CloneVoiceDialog } from "@/CloneVoiceDialog";
import { hotkeyFromKeyDown, hotkeyKeys } from "@/hotkey";
import {
  bridge,
  type EngineStatus,
  type Model,
  type OtherLanguage,
  type Settings,
  type Voice,
} from "@/bridge";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Field,
  FieldContent,
  FieldDescription,
  FieldError,
  FieldGroup,
  FieldLabel,
  FieldLegend,
  FieldSeparator,
  FieldSet,
  FieldTitle,
} from "@/components/ui/field";
import {
  Dialog,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Kbd, KbdGroup } from "@/components/ui/kbd";
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Slider } from "@/components/ui/slider";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { SettingsDialogContent } from "@/components/variants/dialog";
import { version } from "../package.json";

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

/** The gear that opens the settings. They load each time it opens, so polling stops while it's closed. */
export function SettingsDialog() {
  return (
    <Dialog>
      <DialogTrigger render={<Button variant="ghost" size="icon" aria-label="Settings" />}>
        <Settings2 />
      </DialogTrigger>
      <SettingsDialogContent>
        <DialogHeader>
          <DialogTitle>Settings</DialogTitle>
        </DialogHeader>
        <SettingsTabs />
        <DialogFooter>
          <span className="text-xs text-muted-foreground">Yap {version}</span>
        </DialogFooter>
      </SettingsDialogContent>
    </Dialog>
  );
}

/** Voice, languages, saved reads, history, popup and startup. Every change is saved at once. */
function SettingsTabs() {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [models, setModels] = useState<Model[]>([]);
  const [voices, setVoices] = useState<Record<string, Voice[]>>({});
  const [status, setStatus] = useState<EngineStatus>({ state: "ready" });
  const [downloaded, setDownloaded] = useState<string[]>([]);
  const [autostart, setAutostart] = useState(false);
  const [tokenSaved, setTokenSaved] = useState(false);

  useEffect(() => {
    void bridge.getSettings().then(setSettings);
    void bridge.hasToken().then(setTokenSaved);
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
  /** After a voice was cloned or deleted, which every model that clones lists. */
  const refreshVoices = () => void Promise.all(Object.keys(voices).map(voicesOf));
  const usedModels = settings
    ? [settings.model, settings.saveModel, ...settings.otherLanguages.map(({ model }) => model)]
    : [];
  const usedKey = usedModels.join(" ");
  useEffect(() => {
    for (const id of new Set(usedKey.split(" "))) if (id) void voicesOf(id);
  }, [usedKey]);

  // Downloading a model can take a while, so its progress shows here.
  useEffect(() => {
    const timer = setInterval(() => void bridge.engineStatus().then(setStatus), 1000);
    return () => clearInterval(timer);
  }, []);
  useEffect(() => void bridge.downloadedModels().then(setDownloaded), [status.state]);

  if (!settings || models.length === 0) return null;

  const change = (next: Partial<Settings>) => {
    const updated = { ...settings, ...next };
    setSettings(updated);
    void bridge.setSettings(updated);
  };
  const favorites = settings.favoriteVoices;
  const toggleFavorite = (key: string) =>
    change({
      favoriteVoices: favorites.includes(key)
        ? favorites.filter((each) => each !== key)
        : [...favorites, key],
    });
  const model = models.find(({ id }) => id === settings.model) ?? models[0];
  const language = model.languages.includes(settings.language)
    ? settings.language
    : model.languages[0];
  const languages = [...new Set(models.flatMap((each) => each.languages))];
  const languageModels = models.filter((each) => each.languages.includes(language));
  const saving = models.find(({ id }) => id === settings.saveModel);
  const others = settings.otherLanguages;
  const addable = languages.filter(
    (code) => code !== language && !others.some((other) => other.language === code),
  );
  const gated = models.filter(({ id, gated }) => gated && usedModels.includes(id));

  /** The main model when it reads `code`, else the first that does. */
  const modelFor = (code: string) =>
    model.languages.includes(code) ? model : models.find((each) => each.languages.includes(code))!;

  /** `current` when `id` has it in `code`, else the first voice that fits. */
  const fitVoice = async (id: string, code: string, current: string) => {
    const fitting = (await voicesOf(id)).filter((voice) => reads(voice, code));
    return fitting.some(({ name }) => name === current) ? current : (fitting[0]?.name ?? current);
  };
  const chooseLanguage = async (code: string) => {
    const reading = modelFor(code);
    const keepsSaving = saving?.languages.includes(code);
    change({
      language: code,
      model: reading.id,
      voice: await fitVoice(reading.id, code, settings.voice),
      otherLanguages: others.filter((other) => other.language !== code),
      saveModel: keepsSaving ? settings.saveModel : "",
      saveVoice: keepsSaving ? await fitVoice(saving!.id, code, settings.saveVoice) : "",
    });
  };
  const chooseModel = async (id: string) =>
    change({ model: id, voice: await fitVoice(id, language, settings.voice) });
  const addLanguage = async (code: string) => {
    const reading = modelFor(code);
    const voice = await fitVoice(reading.id, code, "");
    change({ otherLanguages: [...others, { language: code, model: reading.id, voice }] });
  };
  const changeOther = (code: string, next: Partial<OtherLanguage>) =>
    change({
      otherLanguages: others.map((other) =>
        other.language === code ? { ...other, ...next } : other,
      ),
    });
  const chooseOtherModel = async (other: OtherLanguage, id: string) =>
    changeOther(other.language, {
      model: id,
      voice: await fitVoice(id, other.language, other.voice),
    });
  const chooseSaveModel = async (id: string) =>
    id === SAME
      ? change({ saveModel: "", saveVoice: "" })
      : change({ saveModel: id, saveVoice: await fitVoice(id, language, settings.saveVoice) });

  const speeds = model.speeds.map(({ speed }) => speed);
  const speedIndex = closestIndex(speeds, settings.speed);

  return (
    <Tabs defaultValue="voice">
      <TabsList>
        <TabsTrigger value="voice">Voice</TabsTrigger>
        <TabsTrigger value="languages">Languages</TabsTrigger>
        <TabsTrigger value="library">Library</TabsTrigger>
        <TabsTrigger value="general">General</TabsTrigger>
      </TabsList>
      <ModelStatus status={status} onCancel={() => void bridge.cancelLoad().then(setSettings)} />
      {gated.length > 0 && !tokenSaved && (
        <TokenField models={gated} saved={false} onSaved={setTokenSaved} />
      )}
      <TabsContent value="voice" className="mt-3">
        <FieldGroup>
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
              options={modelOptions(languageModels, downloaded)}
              onChange={(id) => void chooseModel(id)}
              description={modelDescription(model, language)}
            />
            <VoiceField
              id="voice"
              model={model}
              onVoicesChanged={refreshVoices}
              favorites={favorites}
              onFavorite={toggleFavorite}
              voices={voices[model.id] ?? []}
              language={language}
              value={settings.voice}
              onChange={(voice) => change({ voice })}
            />
            {speeds.length > 1 ? (
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
                <FieldDescription>
                  Every model reads at its closest step to this, so it carries over when you switch.
                </FieldDescription>
              </Field>
            ) : (
              <Field>
                <FieldLabel>Speed</FieldLabel>
                <FieldDescription>
                  {model.name} reads at one speed. Models with a speed control keep{" "}
                  {settings.speed.toFixed(1)}×.
                </FieldDescription>
              </Field>
            )}
            <div>
              <Button variant="outline" onClick={() => void bridge.preview()}>
                <Play data-icon="inline-start" />
                Preview
              </Button>
            </div>
          </FieldSet>
        </FieldGroup>
      </TabsContent>

      <TabsContent value="languages" className="mt-3">
        <FieldSet>
          <FieldLegend>Other languages</FieldLegend>
          <FieldDescription>
            Yap tells which of these languages a read is in and reads it in that language's voice.
            Anything else, and text too short to tell, reads in {languageName(language)}.
          </FieldDescription>
          {others.length === 0 && (
            <p className="text-sm text-muted-foreground">
              None yet, so everything reads in {languageName(language)}.
            </p>
          )}
          {others.map((other) => (
            <OtherLanguageField
              key={other.language}
              other={other}
              models={models.filter((each) => each.languages.includes(other.language))}
              downloaded={downloaded}
              voices={voices[other.model] ?? []}
              favorites={favorites}
              onFavorite={toggleFavorite}
              onVoicesChanged={refreshVoices}
              onModel={(id) => void chooseOtherModel(other, id)}
              onVoice={(voice) => changeOther(other.language, { voice })}
              onRemove={() => change({ otherLanguages: others.filter((each) => each !== other) })}
            />
          ))}
          {addable.length > 0 && (
            <AddLanguage languages={addable} onAdd={(code) => void addLanguage(code)} />
          )}
        </FieldSet>
      </TabsContent>

      <TabsContent value="library" className="mt-3">
        <FieldGroup>
          <FieldSet>
            <ChoiceField
              id="save-model"
              label="Saved reads"
              value={saving?.id ?? SAME}
              options={[
                { value: SAME, label: "Same as reading" },
                ...modelOptions(languageModels, downloaded),
              ]}
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
                model={saving}
                onVoicesChanged={refreshVoices}
                favorites={favorites}
                onFavorite={toggleFavorite}
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
            label="Keep audio files"
            value={String(settings.keepDays)}
            options={KEEP_DAYS.map((days) => ({
              value: String(days),
              label: days === 0 ? "Forever" : `For ${days} days`,
            }))}
            onChange={(days) => change({ keepDays: Number(days) })}
            description="Counted from the last play. Reads kept in the library stay either way."
          />
        </FieldGroup>
      </TabsContent>

      <TabsContent value="general" className="mt-3">
        <FieldGroup>
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
              onCheckedChange={(on) =>
                void (on ? enable() : disable()).then(() => setAutostart(on))
              }
            />
            <FieldContent>
              <FieldLabel htmlFor="autostart">Start with the system</FieldLabel>
              <FieldDescription>Yap waits in the tray, with the voice ready.</FieldDescription>
            </FieldContent>
          </Field>
          <FieldSeparator />
          <ModelsFolderField />
          {tokenSaved && (
            <>
              <FieldSeparator />
              <TokenField models={gated} saved onSaved={setTokenSaved} />
            </>
          )}
        </FieldGroup>
      </TabsContent>
    </Tabs>
  );
}

/** A language read in a model and voice of its own. */
function OtherLanguageField({
  other,
  models,
  downloaded,
  voices,
  favorites,
  onFavorite,
  onVoicesChanged,
  onModel,
  onVoice,
  onRemove,
}: {
  other: OtherLanguage;
  models: Model[];
  downloaded: string[];
  voices: Voice[];
  favorites: string[];
  onFavorite: (key: string) => void;
  onVoicesChanged: () => void;
  onModel: (id: string) => void;
  onVoice: (voice: string) => void;
  onRemove: () => void;
}) {
  const model = models.find(({ id }) => id === other.model) ?? models[0];
  return (
    <div className="rounded-lg border p-4">
      <FieldGroup>
        <div className="flex items-center justify-between gap-2">
          <FieldTitle>{languageName(other.language)}</FieldTitle>
          <Button
            variant="ghost"
            size="icon-sm"
            aria-label={`Remove ${languageName(other.language)}`}
            onClick={onRemove}
          >
            <X />
          </Button>
        </div>
        <ChoiceField
          id={`model-${other.language}`}
          label="Model"
          value={model.id}
          options={modelOptions(models, downloaded)}
          onChange={onModel}
          description={modelDescription(model, other.language)}
        />
        <VoiceField
          id={`voice-${other.language}`}
          model={model}
          onVoicesChanged={onVoicesChanged}
          favorites={favorites}
          onFavorite={onFavorite}
          voices={voices}
          language={other.language}
          value={other.voice}
          onChange={onVoice}
        />
      </FieldGroup>
    </div>
  );
}

/** Picks a language to add to the list; it shows as a button, not a setting of its own. */
function AddLanguage({ languages, onAdd }: { languages: string[]; onAdd: (code: string) => void }) {
  return (
    <div>
      <Select value={null} onValueChange={(code) => code !== null && onAdd(code)}>
        <SelectTrigger aria-label="Add a language">
          <Plus />
          <SelectValue placeholder="Add a language" />
        </SelectTrigger>
        <SelectContent>
          <SelectGroup>
            {languages.map((code) => (
              <SelectItem key={code} value={code}>
                {languageName(code)}
              </SelectItem>
            ))}
          </SelectGroup>
        </SelectContent>
      </Select>
    </div>
  );
}

/** A model's voices that read `language`, favorites first, with the chosen one's description. A model that clones
 * offers to clone another and to delete a cloned one. */
function VoiceField({
  id,
  model,
  voices,
  language,
  value,
  favorites,
  onChange,
  onFavorite,
  onVoicesChanged,
}: {
  id: string;
  model: Model;
  voices: Voice[];
  language: string;
  value: string;
  favorites: string[];
  onChange: (voice: string) => void;
  onFavorite: (key: string) => void;
  onVoicesChanged: () => void;
}) {
  const [deleting, setDeleting] = useState(false);
  const options = voices.filter((voice) => reads(voice, language));
  const none = options.length === 0;
  const chosen = options.find(({ name }) => name === value);
  if (!chosen) options.unshift({ name: value, language, description: "", cloned: false });
  const isFavorite = (name: string) => favorites.includes(`${model.id}/${name}`);
  const sorted = [
    ...options.filter(({ name }) => isFavorite(name)),
    ...options.filter(({ name }) => !isFavorite(name)),
  ];
  const favorite = isFavorite(value);
  const remove = async () => {
    setDeleting(false);
    await bridge.deleteClonedVoice(value);
    onVoicesChanged();
    const left = (await bridge.voices(model.id)).filter((voice) => reads(voice, language));
    onChange(left[0]?.name ?? "");
  };
  return (
    <ChoiceField
      id={id}
      label="Voice"
      value={value}
      options={sorted.map(({ name }) => ({
        value: name,
        label: isFavorite(name) ? `★ ${name}` : name,
      }))}
      onChange={onChange}
      description={
        none && model.clones
          ? `${model.name} reads only in cloned voices; clone one with the button beside.`
          : chosen?.description
      }
      action={
        <>
          <Button
            variant="outline"
            size="icon"
            aria-label={favorite ? "Unfavorite voice" : "Favorite voice"}
            aria-pressed={favorite}
            onClick={() => onFavorite(`${model.id}/${value}`)}
          >
            <Star className={favorite ? "fill-current" : undefined} />
          </Button>
          {chosen?.cloned &&
            (deleting ? (
              <Button
                variant="destructive"
                onBlur={() => setDeleting(false)}
                onClick={() => void remove()}
              >
                Delete {value}
              </Button>
            ) : (
              <Button
                variant="outline"
                size="icon"
                aria-label={`Delete ${value}`}
                onClick={() => setDeleting(true)}
              >
                <Trash2 />
              </Button>
            ))}
          {model.clones && (
            <CloneVoiceDialog
              model={model}
              onAdded={(name) => {
                onVoicesChanged();
                onChange(name);
              }}
            />
          )}
        </>
      }
    />
  );
}

function reads(voice: Voice, language: string) {
  return voice.language === "" || voice.language.startsWith(language);
}

function languageName(code: string) {
  return LANGUAGES[code] ?? code;
}

/** Each model with what it downloads unless it's on disk, so the cost of choosing it shows. */
function modelOptions(models: Model[], downloaded: string[]) {
  return models.map((model) => ({
    value: model.id,
    label: `${model.name} · ${
      downloaded.includes(model.id)
        ? "Downloaded"
        : model.sizeMb < 1000
          ? `${model.sizeMb} MB`
          : `${Math.round(model.sizeMb / 1000)} GB`
    }`,
    extra: <ModelIcons model={model} />,
  }));
}

/** What a model can do, beside its name in the list, each in its own column so the list reads as a table. */
function ModelIcons({ model }: { model: Model }) {
  const features = [
    { Icon: AudioLines, label: "Clones voices", has: model.clones },
    { Icon: Gauge, label: "Speed control", has: model.speeds.length > 1 },
    { Icon: Cpu, label: "Needs an NVIDIA GPU", has: model.gpu },
  ];
  return (
    <span className="ml-auto flex gap-1.5 pl-3 text-muted-foreground">
      {features.map(({ Icon, label, has }) => (
        <span key={label} title={has ? label : undefined} className={has ? undefined : "invisible"}>
          <Icon aria-label={has ? label : undefined} aria-hidden={!has} />
        </span>
      ))}
    </span>
  );
}

function modelDescription(model: Model, language: string) {
  const others = model.languages.filter((code) => code !== language);
  return [
    model.clones && "Clones voices",
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

/** Gated models download only with a token from an account that accepted their terms. Asked for above the tabs
 * while one is needed, and kept in General once saved. */
function TokenField({
  models,
  saved,
  onSaved,
}: {
  models: Model[];
  saved: boolean;
  onSaved: (saved: boolean) => void;
}) {
  const [token, setToken] = useState("");

  const save = async () => {
    await bridge.setToken(token);
    onSaved(token !== "");
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
        {saved ? (
          "Downloads models that ask you to accept their terms. Paste another to replace it."
        ) : (
          <Linked
            text={`Accept the terms at ${models.map(({ id }) => `huggingface.co/gradium/${id}`).join(" and ")} while signed in, then paste a read token from huggingface.co/settings/tokens.`}
          />
        )}
      </FieldDescription>
    </Field>
  );
}

/** Where models download. Choosing another folder moves the downloaded ones there at once. */
function ModelsFolderField() {
  const [folder, setFolder] = useState<string | null>(null);
  const [moving, setMoving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => void bridge.modelsFolder().then(setFolder), []);

  if (folder === null) return null;
  const choose = async () => {
    const picked = await bridge.chooseFolder(folder);
    if (!picked || picked === folder) return;
    setMoving(true);
    setError(null);
    try {
      await bridge.moveModels(picked);
      setFolder(picked);
    } catch (failure) {
      setError(String(failure));
    } finally {
      setMoving(false);
    }
  };

  return (
    <Field>
      <FieldLabel htmlFor="models-folder">Models folder</FieldLabel>
      <div className="flex gap-2">
        <Input id="models-folder" readOnly value={folder} />
        <Button variant="outline" disabled={moving} onClick={() => void choose()}>
          Choose…
        </Button>
      </div>
      {moving ? (
        <p className="flex items-center gap-2 text-sm text-muted-foreground">
          <LoaderCircle className="size-4 animate-spin motion-reduce:animate-none" />
          Moving the models to the new folder
        </p>
      ) : error ? (
        <FieldError>
          {error} The rest stayed here; choosing the folder again finishes it.
        </FieldError>
      ) : (
        <FieldDescription>
          Models and their Python runtimes download here. Choosing another folder moves them there.
        </FieldDescription>
      )}
    </Field>
  );
}

function ModelStatus({ status, onCancel }: { status: EngineStatus; onCancel: () => void }) {
  if (status.state === "moving" || status.state === "loading")
    return (
      <div className="flex items-center justify-between gap-2">
        <p className="flex items-center gap-2 text-sm text-muted-foreground">
          <LoaderCircle className="size-4 animate-spin motion-reduce:animate-none" />
          {status.state === "moving"
            ? "Moving the models to the new folder"
            : `Getting ${status.model} ready, which downloads it the first time`}
        </p>
        {status.state === "loading" && status.cancellable && (
          <Button variant="outline" size="sm" onClick={onCancel}>
            Cancel
          </Button>
        )}
      </div>
    );
  if (status.state === "failed")
    return (
      <div className="flex items-start justify-between gap-2">
        <FieldError>
          <Linked text={status.error} />
        </FieldError>
        <Button variant="outline" size="sm" onClick={() => void bridge.prime()}>
          Try again
        </Button>
      </div>
    );
  return null;
}

/** Web addresses written without a scheme, like huggingface.co/settings/tokens. */
const WEB_ADDRESS = /\b((?:[a-z0-9-]+\.)+[a-z]{2,}\/[\w./-]*[\w/])/gi;

/** `text` with each web address in it opening in the browser. */
function Linked({ text }: { text: string }) {
  return text.split(WEB_ADDRESS).map((part, index) =>
    index % 2 === 0 ? (
      part
    ) : (
      <a
        key={index}
        href={`https://${part}`}
        className="underline underline-offset-4"
        onClick={(event) => {
          event.preventDefault();
          void bridge.openUrl(`https://${part}`);
        }}
      >
        {part}
      </a>
    ),
  );
}

function closestIndex(values: number[], target: number) {
  return values.reduce(
    (best, value, index) =>
      Math.abs(value - target) < Math.abs(values[best] - target) ? index : best,
    0,
  );
}
