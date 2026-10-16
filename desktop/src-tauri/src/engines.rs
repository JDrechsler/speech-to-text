use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Engine {
    Whisper,
    Mai,
    Scribe,
    Assembly,
    Stepfun,
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
    pub const ALL: [Engine; 5] = [
        Engine::Whisper,
        Engine::Mai,
        Engine::Scribe,
        Engine::Assembly,
        Engine::Stepfun,
    ];

    pub fn is_local(self) -> bool {
        self == Engine::Whisper
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Engine::Whisper => "Whisper",
            Engine::Mai => "MAI-Transcribe-2",
            Engine::Scribe => "Scribe v2",
            Engine::Assembly => "Universal-3.5 Pro",
            Engine::Stepfun => "Step-Audio 3 ASR",
        }
    }

    pub fn provider(self) -> &'static str {
        match self {
            Engine::Whisper => "whisper.cpp",
            Engine::Mai => "Microsoft Azure",
            Engine::Scribe => "ElevenLabs",
            Engine::Assembly => "AssemblyAI",
            Engine::Stepfun => "StepFun",
        }
    }

    pub fn short_name(self) -> &'static str {
        match self {
            Engine::Whisper => "Whisper",
            Engine::Mai => "MAI",
            Engine::Scribe => "Scribe",
            Engine::Assembly => "AssemblyAI",
            Engine::Stepfun => "StepFun",
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Engine::Whisper => "whisper",
            Engine::Mai => "mai",
            Engine::Scribe => "scribe",
            Engine::Assembly => "assembly",
            Engine::Stepfun => "stepfun",
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
            Engine::Mai => "Microsoft's MAI model on Azure Speech. Clean punctuation, detects the language by itself.",
            Engine::Scribe => "The fastest cloud engine in this app. Mixed-language recordings can lose the minority language.",
            Engine::Assembly => "Verbatim and strong with several languages in one recording. A few seconds slower.",
            Engine::Stepfun => "StepFun's newest speech model. English, Spanish, French, Chinese, Japanese and Korean, no German. StepFun may use recordings to improve its models.",
        }
    }

    pub fn credential_fields(self) -> &'static [CredentialField] {
        match self {
            Engine::Whisper => &[],
            Engine::Mai => &[AZURE_SPEECH_KEY, AZURE_SPEECH_ENDPOINT],
            Engine::Scribe => &[ELEVENLABS_KEY],
            Engine::Assembly => &[ASSEMBLYAI_KEY],
            Engine::Stepfun => &[STEPFUN_KEY],
        }
    }

    pub fn key_help_url(self) -> Option<&'static str> {
        match self {
            Engine::Whisper => None,
            Engine::Mai => Some("https://learn.microsoft.com/azure/ai-services/speech-service/mai-transcribe"),
            Engine::Scribe => Some("https://elevenlabs.io/app/developers/api-keys"),
            Engine::Assembly => Some("https://www.assemblyai.com/dashboard/api-keys"),
            Engine::Stepfun => Some("https://platform.stepfun.ai/interface-key"),
        }
    }

    pub fn privacy_policy_url(self) -> Option<&'static str> {
        match self {
            Engine::Whisper => None,
            Engine::Mai => Some("https://learn.microsoft.com/azure/foundry/responsible-ai/speech-service/speech-to-text/data-privacy-security"),
            Engine::Scribe => Some("https://elevenlabs.io/privacy-policy"),
            Engine::Assembly => Some("https://www.assemblyai.com/legal/privacy-policy"),
            Engine::Stepfun => Some("https://platform.stepfun.ai/docs/en/agreement/userprivacy"),
        }
    }

    pub fn history_label(self, whisper_model_id: &str) -> String {
        match self {
            Engine::Whisper => format!("whisper-{whisper_model_id}"),
            Engine::Mai => "mai-transcribe-2".into(),
            Engine::Scribe => "scribe-v2".into(),
            Engine::Assembly => "assemblyai-universal-3.5-pro".into(),
            Engine::Stepfun => "stepfun-stepaudio-3-asr-max".into(),
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
