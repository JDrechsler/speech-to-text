# Speech to Text, cross-platform (Tauri)

A port of the Mac app to Tauri 2 (Rust) with a SvelteKit front end, so the same code runs on macOS, Windows and Linux. It is meant to replace the Swift app once it holds up in daily use.

## What it shares with the Mac app

Both apps read and write the same folder (`~/Library/Application Support/SpeechToText` on macOS), so nothing has to be set up twice:

- Whisper models, the personal dictionary and the 24-hour recording history.
- `api-keys.enc`, the encrypted key file. It is sealed with a key derived from this computer's hardware ID, so no password prompt appears.
- `profiles.json`: per-app profiles and rewrite models.
- On first launch, settings are imported from the Mac app's preferences (engine, model, languages, Azure endpoint, pause media).

It has its own bundle ID (`com.jdrechsler.SpeechToText.desktop`), so it can sit next to the Mac app. **Quit the Mac app before testing.** Both apps use ⌥Space.

## Build and run

```bash
bun install
bun tauri dev
```

A signed release build, so macOS keeps the microphone and Accessibility permissions across rebuilds:

```bash
APPLE_SIGNING_IDENTITY="Apple Development: …" bun tauri build --bundles app
open "src-tauri/target/release/bundle/macos/SpeechToText Tauri.app"
```

Tests:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib
SPEECH_TO_TEXT_TEST_WAV=clip.wav cargo test --manifest-path src-tauri/Cargo.toml --lib -- --include-ignored
bun run check
```

## Differences from the Mac app

- **Memory.** The Whisper model loads when you press the hotkey and unloads after 5 idle minutes. The Mac app keeps it loaded, which is about 2 GB with Large v3 Turbo.
- **No Apple Intelligence.** Rewriting uses a model you add (Claude, OpenAI, Gemini, Ollama, LM Studio, any OpenAI-compatible server).
- **Pause media** uses the same MediaRemote adapter as the Mac app and is macOS only for now.

## Platform notes

- **Windows**: hotkey, paste (Ctrl+V) and per-app profiles by executable name. Untested so far.
- **Linux**: the hotkey works on X11. On Wayland, bind a desktop shortcut to `speech-to-text --toggle` and `speech-to-text --cancel`. Per-app profiles are not available on Linux.
