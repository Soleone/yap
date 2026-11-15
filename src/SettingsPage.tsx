import { useEffect, useState } from "react";
import { LoaderCircle, Play } from "lucide-react";
import { disable, enable, isEnabled } from "@tauri-apps/plugin-autostart";
import { ChoiceField } from "@/ChoiceField";
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
