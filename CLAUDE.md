# Speech to Text: guide for coding agents

A native macOS menu-bar dictation app (Swift, AppKit + SwiftUI, Swift Package Manager). ⌥Space records, the chosen engine transcribes, the text is pasted into the focused app and copied to the clipboard.

## Setting the app up for a user

Do these in order and report what you did. Prefer the in-app Settings window whenever the user is at the keyboard; the commands below are for doing it on their behalf.

1. **Build and install** (needs the Xcode command line tools: `xcode-select --install`):
   ```bash
   ./build.sh --run
   ```
   This builds `dist/SpeechToText.app`, installs it to `/Applications` and launches it. On first launch the Settings window opens by itself.

2. **Download an offline Whisper model.** Easiest: tell the user to click **Download** next to *Large v3 Turbo (compact)* in Settings. To do it yourself, download into the models folder with the exact file name the app expects, then select it:
   ```bash
   MODELS="$HOME/Library/Application Support/SpeechToText/models"
   mkdir -p "$MODELS"
   curl -L --fail -o "$MODELS/ggml-large-v3-turbo-q5_0.bin" \
     https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo-q5_0.bin
   defaults write com.jdrechsler.SpeechToText whisperModel large-v3-turbo-q5_0
   defaults write com.jdrechsler.SpeechToText enginePreference whisper
   ```
   Model ids, file names and byte sizes are listed in `WhisperModel.catalog` in `Sources/SpeechToText/WhisperModels.swift`. A file whose size does not match the catalog is a broken download.

3. **Cloud engines (only if the user wants one).** Never ask the user to paste an API key into the chat. Either let them paste it into Settings, or run a command that prompts for it in their own terminal, for example:
   ```bash
   security add-generic-password -U -s SpeechToText -a ELEVENLABS_API_KEY -l "Speech to Text: ELEVENLABS_API_KEY" -T /Applications/SpeechToText.app -w
   ```
   Keychain accounts: `AZURE_SPEECH_KEY`, `ELEVENLABS_API_KEY`, `ASSEMBLYAI_API_KEY` (service `SpeechToText`). The Azure endpoint is not secret and lives in the preferences: `defaults write com.jdrechsler.SpeechToText AZURE_SPEECH_ENDPOINT https://<resource>.cognitiveservices.azure.com`. Select the engine with `defaults write com.jdrechsler.SpeechToText enginePreference mai|scribe|assembly|whisper`. Restart the app after changing preferences from the command line.

4. **Permissions** cannot be granted from the terminal. Tell the user to allow Microphone on the first recording and Accessibility when asked (needed for auto-paste).

Always tell the user which engine is active and whether it uploads audio. Whisper never does; the three cloud engines upload each recording to their provider.

## Layout

| File | Responsibility |
|---|---|
| `Engines.swift` | The engine list: names, provider, local vs cloud, privacy wording, required credentials, readiness |
| `WhisperModels.swift` | Whisper model catalog, download/delete/select (`WhisperModelStore`), language option |
| `LocalWhisper.swift` | In-process whisper.cpp: loads the model, transcribes a 16 kHz mono WAV |
| `MAIClient.swift`, `ScribeClient.swift`, `AssemblyClient.swift` | One cloud provider each |
| `CloudRetry.swift`, `AppLog.swift` | Retry policy for cloud calls and the error log |
| `CloudCredentials.swift`, `Keychain.swift` | Where keys live (Keychain) and the non-secret endpoint (UserDefaults) |
| `AppState.swift` | Recording state machine: recorder → overlay → engine → paste and history |
| `SettingsWindow.swift`, `SettingsRows.swift` | The Settings window |
| `AppDelegate.swift` | Menu bar menu, hotkeys, window wiring |
| `AudioRecorder.swift`, `AudioDevices.swift` | Microphone capture straight to a WAV on disk |
| `HistoryStore.swift`, `HistoryWindow.swift` | 24-hour recording history with re-transcribe |
| `AppPaths.swift` | Every on-disk location |

whisper.cpp comes in as a prebuilt `whisper.xcframework` binary target pinned by checksum in `Package.swift`. `build.sh` copies `whisper.framework` into `Contents/Frameworks`.

## Verify a change

```bash
swift build                 # must finish with no warnings
./build.sh --dist           # full signed bundle in dist/, installs nothing
```

`./build.sh` (no flag) replaces the installed app in `/Applications` and kills the running one. Ask before doing that on someone's machine; they may be dictating with it.

## Rules that are easy to break

- **Privacy labelling is the product.** Every engine must declare `isLocal` and its `provider` truthfully. A new cloud engine shows up under Cloud with an "uploads audio to <provider>" badge automatically; do not bypass `EnginePreference` to add one.
- **The chosen engine is the only engine.** No silent fallback from one engine to another. A failure is reported and the audio stays in History.
- **Audio is never lost.** The recorder writes to disk while recording; never move transcription before the file is closed.
- **The Whisper prompt is fake preceding transcript, not an instruction.** Whisper continues whatever text the prompt looks like. A glossary-style prompt ("Glossary: Anna, Ben, …") reads like a meeting transcript's cast list and makes Whisper invent speaker labels. Keep the first-person dictation framing in `firstPersonVocabularyPrompt`.
- **MAI-Transcribe-2 rejects more than 50 phrases** with HTTP 400, so `MAIClient` caps the dictionary at 50.
- **Free the whisper context before exit.** ggml's Metal backend asserts at process exit if a model is still loaded, which shows up as a crash on Quit. `applicationWillTerminate` calls `LocalWhisper.shared.releaseBeforeExit()`.
- **Nothing personal in the repo.** The dictionary, recordings, keys and models live in the user's Library folders, never in the repository.
