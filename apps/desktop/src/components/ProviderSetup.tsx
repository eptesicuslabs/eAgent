import { useState } from "react";
import { readNativeApi } from "~/nativeApi";

const PRESETS = [
  { label: "nvidia", endpoint: "https://integrate.api.nvidia.com/v1", model: "meta/llama-3.1-70b-instruct" },
  { label: "openai", endpoint: "https://api.openai.com/v1", model: "gpt-4o" },
  { label: "openrouter", endpoint: "https://openrouter.ai/api/v1", model: "anthropic/claude-sonnet-4" },
  { label: "together", endpoint: "https://api.together.xyz/v1", model: "meta-llama/Meta-Llama-3.1-70B-Instruct-Turbo" },
];

export function ProviderSetup(props: { onDone?: () => void }) {
  const [endpoint, setEndpoint] = useState(PRESETS[0].endpoint);
  const [apiKey, setApiKey] = useState("");
  const [model, setModel] = useState(PRESETS[0].model);
  const [status, setStatus] = useState<"idle" | "saving" | "done" | "error">("idle");
  const [error, setError] = useState("");

  function pick(i: number) {
    setEndpoint(PRESETS[i].endpoint);
    setModel(PRESETS[i].model);
    setStatus("idle");
  }

  async function save() {
    if (!endpoint || !apiKey || !model) { setError("all fields required"); return; }
    setStatus("saving");
    try {
      await readNativeApi().eagent.configureProvider({ endpoint, apiKey, model });
      setStatus("done");
      props.onDone?.();
    } catch (e) {
      setError(String(e));
      setStatus("error");
    }
  }

  return (
    <div className="text-xs">
      <div className="text-muted-foreground mb-2">configure provider:</div>
      <div className="flex gap-2 mb-2">
        {PRESETS.map((p, i) => (
          <button key={p.label} className="text-muted-foreground hover:text-foreground underline" onClick={() => pick(i)} type="button">{p.label}</button>
        ))}
      </div>
      <div className="grid gap-1.5 mb-2">
        <input className="bg-neutral-900 border border-border px-2 py-1 text-foreground outline-none focus:border-neutral-600" value={endpoint} onChange={(e) => setEndpoint(e.target.value)} placeholder="endpoint" />
        <input className="bg-neutral-900 border border-border px-2 py-1 text-foreground outline-none focus:border-neutral-600" type="password" value={apiKey} onChange={(e) => setApiKey(e.target.value)} placeholder="api key" />
        <input className="bg-neutral-900 border border-border px-2 py-1 text-foreground outline-none focus:border-neutral-600" value={model} onChange={(e) => setModel(e.target.value)} placeholder="model" />
      </div>
      {error ? <div className="text-red-500 mb-1">{error}</div> : null}
      <button
        className="px-3 py-1 bg-foreground text-background font-bold hover:opacity-90 disabled:opacity-30"
        disabled={status === "saving" || !apiKey}
        onClick={() => void save()}
        type="button"
      >{status === "done" ? "connected ✓" : status === "saving" ? "..." : "connect"}</button>
    </div>
  );
}
