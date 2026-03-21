import { Check, Loader2 } from "lucide-react";
import { useState } from "react";
import { readNativeApi } from "~/nativeApi";

const PRESETS = [
  { label: "NVIDIA NIM", endpoint: "https://integrate.api.nvidia.com/v1", model: "meta/llama-3.1-70b-instruct" },
  { label: "OpenAI", endpoint: "https://api.openai.com/v1", model: "gpt-4o" },
  { label: "OpenRouter", endpoint: "https://openrouter.ai/api/v1", model: "anthropic/claude-sonnet-4" },
  { label: "Together", endpoint: "https://api.together.xyz/v1", model: "meta-llama/Meta-Llama-3.1-70B-Instruct-Turbo" },
];

export function ProviderSetup(props: { onDone?: () => void }) {
  const [endpoint, setEndpoint] = useState(PRESETS[0].endpoint);
  const [apiKey, setApiKey] = useState("");
  const [model, setModel] = useState(PRESETS[0].model);
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState<string | null>(null);

  function pick(idx: number) {
    setEndpoint(PRESETS[idx].endpoint);
    setModel(PRESETS[idx].model);
    setSaved(false);
    setError(null);
  }

  async function save() {
    if (!endpoint.trim() || !apiKey.trim() || !model.trim()) {
      setError("All fields are required");
      return;
    }
    setSaving(true);
    setError(null);
    try {
      const api = readNativeApi();
      await api.eagent.configureProvider({
        endpoint: endpoint.trim(),
        apiKey: apiKey.trim(),
        model: model.trim(),
      });
      setSaved(true);
      props.onDone?.();
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  }

  return (
    <div className="w-full max-w-md">
      <h3 className="text-sm font-semibold text-foreground">Connect AI Provider</h3>
      <p className="mt-1 text-[11px] text-muted-foreground/70">
        Any OpenAI-compatible API — NVIDIA, OpenAI, OpenRouter, Together, or custom.
      </p>

      {/* Presets */}
      <div className="mt-3 flex flex-wrap gap-1.5">
        {PRESETS.map((p, i) => (
          <button
            key={p.label}
            className="rounded-md border border-border px-2.5 py-1 text-[10px] font-medium text-muted-foreground transition hover:text-foreground hover:border-foreground/20"
            onClick={() => pick(i)}
            type="button"
          >
            {p.label}
          </button>
        ))}
      </div>

      {/* Fields */}
      <div className="mt-3 grid gap-2">
        <input
          className="rounded-lg border border-border bg-background/60 px-3 py-2 text-[12px] text-foreground outline-none focus:border-foreground/20 placeholder:text-muted-foreground/40"
          value={endpoint}
          onChange={(e) => { setEndpoint(e.target.value); setSaved(false); }}
          placeholder="API endpoint"
        />
        <input
          className="rounded-lg border border-border bg-background/60 px-3 py-2 text-[12px] text-foreground outline-none focus:border-foreground/20 placeholder:text-muted-foreground/40"
          type="password"
          value={apiKey}
          onChange={(e) => { setApiKey(e.target.value); setSaved(false); }}
          placeholder="API key"
        />
        <input
          className="rounded-lg border border-border bg-background/60 px-3 py-2 text-[12px] text-foreground outline-none focus:border-foreground/20 placeholder:text-muted-foreground/40"
          value={model}
          onChange={(e) => { setModel(e.target.value); setSaved(false); }}
          placeholder="Model name"
        />
      </div>

      {error ? <p className="mt-2 text-[11px] text-rose-400">{error}</p> : null}

      <button
        className="mt-3 inline-flex items-center gap-1.5 rounded-lg bg-foreground px-3.5 py-2 text-[11px] font-semibold text-background transition hover:opacity-90 disabled:opacity-40"
        disabled={saving || !apiKey.trim()}
        onClick={() => void save()}
        type="button"
      >
        {saving ? <Loader2 className="size-3 animate-spin" /> : saved ? <Check className="size-3" /> : null}
        {saved ? "Connected" : saving ? "Connecting..." : "Connect"}
      </button>
    </div>
  );
}
