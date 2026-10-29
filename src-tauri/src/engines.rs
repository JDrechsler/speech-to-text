use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Engine {
    Whisper,
    Nemotron,
    Mai,
    Scribe,
    Assembly,
    Stepfun,
    Openai,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialField {
    pub id: &'static str,
    pub label: &'static str,
    pub placeholder: &'static str,
    pub is_secret: bool,
}

pub const THIS_DEVICE: &str = if cfg!(target_os = "macos") { "this Mac" } else { "this computer" };

pub const AZURE_SPEECH_KEY: CredentialField = CredentialField {
    id: "AZURE_SPEECH_KEY",
    label: "API key",
    placeholder: "Key 1 from your Azure Speech resource",
    is_secret: true,
};
pub const AZURE_SPEECH_ENDPOINT: CredentialField = CredentialField {
    id: "AZURE_SPEECH_ENDPOINT",
    label: "Endpoint",
    placeholder: "https://<resource-name>.cognitiveservices.azure.com",
    is_secret: false,
};
pub const ELEVENLABS_KEY: CredentialField = CredentialField {
    id: "ELEVENLABS_API_KEY",
    label: "API key",
    placeholder: "ElevenLabs API key",
    is_secret: true,
};
pub const ASSEMBLYAI_KEY: CredentialField = CredentialField {
    id: "ASSEMBLYAI_API_KEY",
    label: "API key",
    placeholder: "AssemblyAI API key",
    is_secret: true,
};
pub const OPENAI_KEY: CredentialField = CredentialField {
    id: "OPENAI_API_KEY",
    label: "API key",
    placeholder: "OpenAI API key",
    is_secret: true,
};
pub const STEPFUN_KEY: CredentialField = CredentialField {
    id: "STEPFUN_API_KEY",
    label: "API key",
    placeholder: "StepFun API key",
    is_secret: true,
};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineInfo {
    pub id: Engine,
    pub is_local: bool,
    pub display_name: &'static str,
    pub provider: &'static str,
    pub short_name: &'static str,
    pub summary: &'static str,
    pub credential_fields: &'static [CredentialField],
    pub key_help_url: Option<&'static str>,
    pub privacy_policy_url: Option<&'static str>,
}

impl Engine {
    pub const ALL: [Engine; 7] = [
        Engine::Whisper,
        Engine::Nemotron,
        Engine::Mai,
        Engine::Scribe,
        Engine::Assembly,
        Engine::Openai,
        Engine::Stepfun,
    ];

    pub fn is_local(self) -> bool {
        matches!(self, Engine::Whisper | Engine::Nemotron)
    }

    pub fn is_available(self) -> bool {
        self != Engine::Nemotron || cfg!(all(target_os = "macos", target_arch = "aarch64"))
    }

    pub fn available() -> impl Iterator<Item = Engine> {
        Engine::ALL.into_iter().filter(|engine| engine.is_available())
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Engine::Whisper => "Whisper",
            Engine::Nemotron => "Nemotron 3.5 Transcribe",
            Engine::Mai => "MAI-Transcribe-2",
            Engine::Scribe => "Scribe v2",
            Engine::Assembly => "Universal-3.5 Pro",
            Engine::Stepfun => "Step-Audio 3 ASR",
            Engine::Openai => "GPT Transcribe",
        }
    }

    pub fn provider(self) -> &'static str {
        match self {
            Engine::Whisper => "whisper.cpp",
            Engine::Nemotron => "NVIDIA",
            Engine::Mai => "Microsoft Azure",
            Engine::Scribe => "ElevenLabs",
            Engine::Assembly => "AssemblyAI",
            Engine::Stepfun => "StepFun",
            Engine::Openai => "OpenAI",
        }
    }

    pub fn short_name(self) -> &'static str {
        match self {
            Engine::Whisper => "Whisper",
            Engine::Nemotron => "Nemotron",
            Engine::Mai => "MAI",
            Engine::Scribe => "Scribe",
            Engine::Assembly => "AssemblyAI",
            Engine::Stepfun => "StepFun",
            Engine::Openai => "GPT Transcribe",
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Engine::Whisper => "whisper",
            Engine::Nemotron => "nemotron",
            Engine::Mai => "mai",
            Engine::Scribe => "scribe",
            Engine::Assembly => "assembly",
            Engine::Stepfun => "stepfun",
            Engine::Openai => "openai",
        }
    }

    pub fn from_id(id: &str) -> Option<Engine> {
        Engine::ALL.into_iter().find(|engine| engine.id() == id)
    }

    pub fn menu_title(self) -> String {
        if self.is_local() {
            format!("{} · on {THIS_DEVICE}", self.display_name())
        } else {
            format!("{} · {}", self.display_name(), self.provider())
        }
    }

    pub fn summary(self) -> &'static str {
        match self {
            Engine::Whisper => "OpenAI's open Whisper model running on your own computer. Works without internet.",
            Engine::Nemotron => "NVIDIA's open speech model running on your own computer. Works without internet. Needs a Mac with 32 GB of memory or more.",
            Engine::Mai => "Microsoft's MAI model on Azure Speech. Clean punctuation, detects the language by itself.",
            Engine::Scribe => "The fastest cloud engine in this app. Mixed-language recordings can lose the minority language.",
            Engine::Assembly => "Verbatim and strong with several languages in one recording. A few seconds slower.",
            Engine::Stepfun => "StepFun's newest speech model. English, Spanish, French, Chinese, Japanese and Korean, no German. StepFun may use recordings to improve its models.",
            Engine::Openai => "OpenAI's newest speech model. Handles several languages in one recording and uses your dictionary.",
        }
    }

    pub fn credential_fields(self) -> &'static [CredentialField] {
        match self {
            Engine::Whisper | Engine::Nemotron => &[],
            Engine::Mai => &[AZURE_SPEECH_KEY, AZURE_SPEECH_ENDPOINT],
            Engine::Scribe => &[ELEVENLABS_KEY],
            Engine::Assembly => &[ASSEMBLYAI_KEY],
            Engine::Stepfun => &[STEPFUN_KEY],
            Engine::Openai => &[OPENAI_KEY],
        }
    }

    pub fn key_help_url(self) -> Option<&'static str> {
        match self {
            Engine::Whisper | Engine::Nemotron => None,
            Engine::Mai => Some("https://learn.microsoft.com/azure/ai-services/speech-service/mai-transcribe"),
            Engine::Scribe => Some("https://elevenlabs.io/app/developers/api-keys"),
            Engine::Assembly => Some("https://www.assemblyai.com/dashboard/api-keys"),
            Engine::Stepfun => Some("https://platform.stepfun.ai/interface-key"),
            Engine::Openai => Some("https://platform.openai.com/api-keys"),
        }
    }

    pub fn privacy_policy_url(self) -> Option<&'static str> {
        match self {
            Engine::Whisper | Engine::Nemotron => None,
            Engine::Mai => Some("https://learn.microsoft.com/azure/foundry/responsible-ai/speech-service/speech-to-text/data-privacy-security"),
            Engine::Scribe => Some("https://elevenlabs.io/privacy-policy"),
            Engine::Assembly => Some("https://www.assemblyai.com/legal/privacy-policy"),
            Engine::Stepfun => Some("https://platform.stepfun.ai/docs/en/agreement/userprivacy"),
            Engine::Openai => Some("https://developers.openai.com/api/docs/guides/your-data"),
        }
    }

    pub fn history_label(self, model_id: &str) -> String {
        match self {
            Engine::Whisper => format!("whisper-{model_id}"),
            Engine::Nemotron => model_id.to_string(),
            Engine::Mai => "mai-transcribe-2".into(),
            Engine::Scribe => "scribe-v2".into(),
            Engine::Assembly => "assemblyai-universal-3.5-pro".into(),
            Engine::Stepfun => "stepfun-stepaudio-3-asr-max".into(),
            Engine::Openai => "openai-gpt-transcribe".into(),
        }
    }

    pub fn info(self) -> EngineInfo {
        EngineInfo {
            id: self,
            is_local: self.is_local(),
            display_name: self.display_name(),
            provider: self.provider(),
            short_name: self.short_name(),
            summary: self.summary(),
            credential_fields: self.credential_fields(),
            key_help_url: self.key_help_url(),
            privacy_policy_url: self.privacy_policy_url(),
        }
    }
}

pub fn all_credential_fields() -> impl Iterator<Item = CredentialField> {
    Engine::ALL
        .into_iter()
        .flat_map(|engine| engine.credential_fields().iter().copied())
}
