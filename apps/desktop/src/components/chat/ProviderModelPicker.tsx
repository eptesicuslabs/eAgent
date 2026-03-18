import { useMemo } from "react";
import { useStore } from "~/store";
import { readNativeApi } from "~/nativeApi";
import type { ProviderKind, ThreadState } from "~/types";

function unique(values: Array<string | null | undefined>) {
  return [...new Set(values.map((value) => value?.trim()).filter((value): value is string => Boolean(value)))];
}

export function ProviderModelPicker(props: { thread: ThreadState }) {
  const bootstrap = useStore((state) => state.bootstrap);

  const codexModels = useMemo(
    () => unique([props.thread.settings.model, ...(bootstrap?.codexModels ?? [])]),
    [bootstrap?.codexModels, props.thread.settings.model],
  );
  const llamaModels = useMemo(
    () =>
      unique([
        props.thread.settings.model,
        bootstrap?.config.llama_cpp.default_model,
        bootstrap?.config.llama_cpp.model_path,
      ]),
    [
      bootstrap?.config.llama_cpp.default_model,
      bootstrap?.config.llama_cpp.model_path,
      props.thread.settings.model,
    ],
  );

  async function updateSettings(next: Partial<ThreadState["settings"]>) {
    const api = readNativeApi();
    if (!api) return;
    await api.orchestration.dispatch({
      type: "updateCurrentThreadSettings",
      settings: {
        ...props.thread.settings,
        ...next,
      },
    });
  }

  async function handleProviderChange(provider: ProviderKind) {
    const nextModel =
      provider === "codex"
        ? codexModels[0] ?? bootstrap?.config.codex.default_model ?? props.thread.settings.model
        : llamaModels[0] ??
          bootstrap?.config.llama_cpp.default_model ??
          props.thread.settings.model;

    await updateSettings({ provider, model: nextModel });
  }

  const modelOptions = props.thread.settings.provider === "codex" ? codexModels : llamaModels;

  return (
    <div className="flex flex-wrap items-center gap-2 rounded-[1.1rem] border border-border/70 bg-background/70 px-3 py-2">
      <select
        className="rounded-full border border-border/70 bg-card px-3 py-1.5 text-xs font-medium text-foreground outline-none"
        onChange={(event) => void handleProviderChange(event.target.value as ProviderKind)}
        value={props.thread.settings.provider}
      >
        <option value="codex">Codex</option>
        <option value="llama-cpp">llama.cpp</option>
      </select>

      <select
        className="max-w-44 rounded-full border border-border/70 bg-card px-3 py-1.5 text-xs font-medium text-foreground outline-none"
        onChange={(event) => void updateSettings({ model: event.target.value })}
        value={props.thread.settings.model}
      >
        {modelOptions.map((model) => (
          <option key={model} value={model}>
            {model}
          </option>
        ))}
      </select>
    </div>
  );
}
