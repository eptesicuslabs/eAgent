import type { NativeApi } from "./types";
import { createTauriNativeApi } from "./tauriNativeApi";

let cachedApi: NativeApi | undefined;

export function readNativeApi(): NativeApi {
  if (!cachedApi) {
    cachedApi = createTauriNativeApi();
  }
  return cachedApi;
}
