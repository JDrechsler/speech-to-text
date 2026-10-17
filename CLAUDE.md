# Speech to Text: guide for coding agents

A menu-bar (tray) dictation app for macOS, Windows and Linux, built with Tauri 2: a Rust backend in `src-tauri/` and a SvelteKit front end in `src/` for the Settings, History and recording overlay windows. ⌥Space (Alt+Space on Windows and Linux) records, the chosen engine transcribes, the text is pasted into the focused app and copied to the clipboard.

## Setting the app up for a user

Do these in order and report what you did. Prefer the Settings window whenever the user is at the keyboard; the commands below are for doing it on their behalf.

1. **Build and install.** Needs Rust (`rustup`), [bun](https://bun.sh) and the platform tools listed in the README.
   ```bash
   bun install
   bun tauri build --bundles app
   ```
   On macOS this produces `src-tauri/target/release/bundle/macos/SpeechToText.app`; copy it to `/Applications` and open it. On Windows and Linux use `bun tauri build` and run the installer it produces under `src-tauri/target/release/bundle/`. On first launch the Settings window opens by itself.

   On macOS, sign local builds so Microphone and Accessibility survive rebuilds: `APPLE_SIGNING_IDENTITY="Apple Development: …" bun tauri build --bundles app`. Ask before replacing an installed copy; the user may be dictating with it.

2. **Download an offline Whisper model.** Easiest: tell the user to click **Download** next to *Large v3 Turbo (compact)* in Settings. To do it yourself, download into the models folder with the exact file name the app expects, then select it in Settings:
   ```bash
   MODELS="$HOME/Library/Application Support/SpeechToText/models"
   mkdir -p "$MODELS"
   curl -L --fail -o "$MODELS/ggml-large-v3-turbo-q5_0.bin" \
     https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo-q5_0.bin
   ```
   The data folder is `~/Library/Application Support/SpeechToText` on macOS, `%APPDATA%\SpeechToText` on Windows and `~/.local/share/SpeechToText` on Linux. Model ids, file names and byte sizes are listed in `CATALOG` in `src-tauri/src/models.rs`. A file whose size does not match the catalog is a broken download.

3. **Cloud engines (only if the user wants one).** Never ask the user to paste an API key into the chat. Keys are entered in Settings and saved to `api-keys.enc` in the data folder, encrypted with a key derived from this computer's hardware ID; there is no command-line way to add one. The Azure endpoint is not secret and lives in `settings.json` as `azureEndpoint`. The engine is `settings.json` → `engine`: `whisper`, `mai`, `scribe`, `assembly` or `stepfun`. Quit the app before editing `settings.json`.

4. **Permissions** cannot be granted from the terminal. On macOS, tell the user to allow Microphone on the first recording and Accessibility when asked (needed for auto-paste).

Always tell the user which engine is active and whether it uploads audio. Whisper never does; every cloud engine uploads each recording to its provider.

## Layout

| Path | Responsibility |
|---|---|
| `src-tauri/src/engines.rs` | The engine list: names, provider, local vs cloud, required credentials |
| `src-tauri/src/models.rs`, `whisper.rs` | Whisper model catalog and downloads; in-process whisper.cpp that loads on the hotkey and unloads after 5 idle minutes |
| `src-tauri/src/cloud/` | One file per cloud provider, plus the retry policy |
| `src-tauri/src/session.rs`, `state.rs` | Recording state machine: recorder → overlay → engine → rewrite → paste and history |
| `src-tauri/src/rewrite/`, `profiles/` | Optional rewrite step with a model the user adds, and per-app profiles |
| `src-tauri/src/secrets.rs`, `settings.rs` | `api-keys.enc` and `settings.json`; the one-time import of the old Mac app's preferences and Keychain keys |
| `src-tauri/src/audio.rs`, `audio_devices.rs`, `microphone.rs` | Microphone capture straight to a WAV on disk; CoreAudio device listing on macOS |
| `src-tauri/src/paste.rs`, `frontmost.rs`, `media.rs` | Auto-paste, the focused app, pausing media (macOS) |
| `src-tauri/src/tray.rs`, `windows.rs`, `login_item.rs` | Menu bar menu, windows, Launch at Login |
| `src-tauri/src/commands/` | Everything the front end can call |
| `src/routes/settings`, `history`, `overlay` | The three windows |
| `vendor/mediaremote-adapter` | Built by `src-tauri/build.rs` (through `scripts/build-media-adapter.sh`) on every macOS build, for pausing media |

## Verify a change

```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib   # must pass with no warnings
bun run check                                           # svelte-check, 0 errors and 0 warnings
bun tauri build --bundles app                           # full bundle, installs nothing
```

`SPEECH_TO_TEXT_TEST_WAV=clip.wav cargo test --manifest-path src-tauri/Cargo.toml --lib -- --include-ignored` also runs a real Whisper transcription.

## Rules that are easy to break

- **Privacy labelling is the product.** Every engine declares truthfully whether it is local and who its provider is. Cloud engines are marked "uploads audio" everywhere they appear. Rewrite models count as local only for loopback addresses.
- **The chosen engine is the only engine.** No silent fallback from one engine to another. A failure is reported and the audio stays in History.
- **Audio is never lost.** The recorder writes to disk while recording; never move transcription before the file is closed.
- **The overlay never activates the app.** Paste goes to whichever app is frontmost, so a click or drag on the overlay must not bring Speech to Text forward. On macOS `drag_on_click_without_activating` in `windows.rs` sets `_setPreventsActivation:`; a plain `NSWindow` drops the public non-activating style mask.
- **Never block the main thread.** Sync Tauri commands, tray menu building and setup run on it. No subprocesses, network calls or waits on the Whisper lock there; the hardware ID is read once and cached for that reason.
- **The Whisper prompt is fake preceding transcript, not an instruction.** Whisper continues whatever text the prompt looks like. A glossary-style prompt reads like a meeting transcript's cast list and makes Whisper invent speaker labels. Keep the first-person dictation framing in `vocabulary_prompt`.
- **MAI-Transcribe-2 rejects more than 50 phrases** with HTTP 400, so `cloud/mai.rs` caps the dictionary at 50.
- **StepFun's Step-Audio 3 ASR is not OpenAI-compatible.** It takes base64 audio in JSON on `/v1/audio/asr/sse` and answers with server-sent events; the transcript is the `text` of the `transcript.text.done` event. It has no language detection and no German. Its `hotwords` field is only documented for the China endpoint, so the dictionary is not sent.
- **The dictionary is never sent to rewrite models.** A model once pasted the word list back instead of the dictation.
- **Free the Whisper context before exit.** ggml's Metal backend asserts at process exit if a model is still loaded. `RunEvent::Exit` unloads it.
- **Nothing personal in the repo.** The dictionary, recordings, keys and models live in the user's data folder, never in the repository.
