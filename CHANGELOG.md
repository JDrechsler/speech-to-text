# Changelog

## 2.1.0

- New cloud engine: GPT Transcribe by OpenAI. It handles several languages in one recording and uses your dictionary. Add your OpenAI API key in Settings under Cloud.
- Fixed Whisper sometimes leaving out whole passages of longer dictations when you have words in your dictionary.
- Every dictation now ends with a period when the text stops right after a word. Scribe in particular leaves it off the last sentence. Endings with a question mark, exclamation mark or other punctuation stay as they are.

## 2.0.2

- Fixed the microphone staying on after you finished dictating, which made macOS show it in use even while you were not recording. It now turns off as soon as you stop or cancel a recording.

## 2.0.1

- You can now drag the recording window anywhere while you talk, so it never covers what you are looking at. Your words are still pasted into the app you were typing in.

## 2.0.0

- Speech to Text now runs on Windows and Linux as well as macOS.
- Rewrite before pasting: a language model can clean up what you said, or turn it into an email, a chat message or a prompt for an AI. Use a model on your own computer, or your own key for a cloud model.
- Per-app profiles: the app you are typing in decides the engine, the rewrite style and the model. A casual message in Slack, a clear prompt in your terminal, a proper email in Mail. Available on macOS and Windows.
- Much lighter on memory. Whisper loads when you press the hotkey and unloads after 5 minutes without dictation, instead of keeping about 2 GB in memory all the time.
- No more Keychain prompts. API keys are kept in an encrypted file that only opens on your computer.
- The Engine menu marks engines on your computer in green and cloud engines in orange.
- Upgrading from version 1 keeps your settings and API keys.

## 1.1.1

- Settings shows which version of the app you are running.
- If you build the app yourself with an Apple Development certificate, macOS now keeps the Microphone and Accessibility permissions after each rebuild.

## 1.1.0

- New cloud engine: Step-Audio 3 ASR from StepFun. It understands English, Spanish, French, Chinese, Japanese and Korean, but not German, and you pick the spoken language yourself in Settings.

## 1.0.1

- First open-source release.
- Settings starts with one simple choice: On this Mac or Cloud. Cloud providers only appear once you pick Cloud, and each one is clearly marked as uploading your audio.
