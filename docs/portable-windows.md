# Portable Windows Build

`eCode` now supports a portable storage layout.

## Runtime behavior
- On startup, the app tries to use `eCode-data\` next to `eCode.exe`.
- If that location is writable, config, logs, attachments, and the event store stay there.
- If it is not writable, the app falls back to the normal system profile directories.

## Build
Run:

```powershell
.\scripts\build-portable.ps1
```

That script:
- builds a release binary for `x86_64-pc-windows-msvc`
- enables the static CRT via `-C target-feature=+crt-static`
- writes a portable folder under `dist\eCode-portable`
- creates `dist\eCode-portable-windows-x64.zip`

## Notes
- `llama-server` and GGUF models are not bundled into the executable.
- Configure local model paths in Settings after launch if you want `llama.cpp`.
