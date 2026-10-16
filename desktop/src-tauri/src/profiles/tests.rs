use super::*;
use crate::rewrite::ModelApi;
use serde_json::{Value, json};

const MAC_APP_FILE: &str = r#"{
  "apps" : [
{
  "appName" : "Slack",
  "bundleID" : "com.tinyspeck.slackmacgap",
  "customInstructions" : "",
  "engine" : "scribe",
  "rewriterID" : "6F1C7E52-0D4B-4B8E-9E0A-3C2D1B5A7F10",
  "style" : "chat"
},
{
  "appName" : "Ghostty",
  "bundleID" : "com.mitchellh.ghostty",
  "customInstructions" : "Answer as a shell command.",
  "style" : "custom"
}
  ],
  "defaultCustomInstructions" : "",
  "defaultRewriterID" : "6F1C7E52-0D4B-4B8E-9E0A-3C2D1B5A7F10",
  "defaultStyle" : "cleanup",
  "models" : [
{
  "api" : "anthropic",
  "baseURL" : "https:\/\/api.anthropic.com\/v1",
  "id" : "6F1C7E52-0D4B-4B8E-9E0A-3C2D1B5A7F10",
  "keyHelpURL" : "https:\/\/platform.claude.com\/settings\/keys",
  "model" : "claude-haiku-4-5",
  "name" : "Claude"
},
{
  "api" : "openAI",
  "baseURL" : "http:\/\/localhost:1234\/v1",
  "id" : "0B9D3F24-7A1E-4C55-8D2B-6E4F0A9C1D37",
  "model" : "qwen3",
  "name" : "LM Studio"
}
  ]
}"#;

fn local_model() -> RewriteModel {
    RewriteModel {
        model: "qwen3".into(),
        ..RewriteModel::new("LM Studio", ModelApi::OpenAi, "http://localhost:1234/v1")
    }
}

fn styled_app(bundle_id: &str, app_name: &str, style: RewriteStyle) -> AppProfile {
    AppProfile {
        style: Some(style),
        ..AppProfile::new(bundle_id, app_name)
    }
}

fn scratch_file() -> std::path::PathBuf {
    let folder = std::env::temp_dir().join(format!("profiles-test-{}", uuid::Uuid::new_v4()));
    folder.join("profiles.json")
}

#[test]
fn app_profile_overrides_the_defaults() {
    let model = local_model();
    let mut settings = ProfileSettings {
        default_style: RewriteStyle::Cleanup,
        models: vec![model.clone()],
        ..ProfileSettings::default()
    };
    settings.apps = vec![AppProfile {
        engine: Some(Engine::Scribe),
        rewriter_id: Some(model.id.clone()),
        ..styled_app("com.tinyspeck.slackmacgap", "Slack", RewriteStyle::Chat)
    }];

    let slack = settings.resolve(settings.apps.first(), Some("Slack"), Engine::Whisper);
    assert!(slack.has_app_profile);
    assert_eq!(slack.engine, Engine::Scribe);
    assert_eq!(slack.style, RewriteStyle::Chat);
    assert_eq!(slack.rewriter, RewriteTarget::Model(model));
    assert_eq!(slack.context_line().as_deref(), Some("Slack · Chat message"));

    let mail = settings.resolve(None, Some("Mail"), Engine::Whisper);
    assert!(!mail.has_app_profile);
    assert_eq!(mail.engine, Engine::Whisper);
    assert_eq!(mail.style, RewriteStyle::Cleanup);
    assert_eq!(mail.rewriter, RewriteTarget::Apple);
    assert_eq!(mail.context_line().as_deref(), Some("Clean up"));
}

#[test]
fn no_profile_and_no_style_shows_no_context_line() {
    let settings = ProfileSettings::default();
    let profile = settings.resolve(None, Some("Notes"), Engine::Whisper);
    assert!(!profile.rewrites());
    assert_eq!(profile.context_line(), None);

    let notes = AppProfile::new("com.apple.Notes", "Notes");
    let profile = settings.resolve(Some(&notes), Some("Notes"), Engine::Whisper);
    assert_eq!(profile.context_line().as_deref(), Some("Notes · No rewrite"));
}

#[test]
fn custom_style_without_instructions_does_not_rewrite() {
    let mut settings = ProfileSettings {
        default_style: RewriteStyle::Custom,
        ..ProfileSettings::default()
    };
    assert!(!settings.resolve(None, None, Engine::Whisper).rewrites());
    settings.default_custom_instructions = "Translate to English.".into();
    assert!(settings.resolve(None, None, Engine::Whisper).rewrites());
}

#[test]
fn custom_app_style_uses_the_apps_own_instructions() {
    let settings = ProfileSettings {
        default_custom_instructions: "Translate to English.".into(),
        ..ProfileSettings::default()
    };
    let ghostty = AppProfile {
        custom_instructions: "Answer as a shell command.".into(),
        ..styled_app("com.mitchellh.ghostty", "Ghostty", RewriteStyle::Custom)
    };
    let mail = AppProfile {
        custom_instructions: "Ignored".into(),
        ..styled_app("com.apple.mail", "Mail", RewriteStyle::Email)
    };
    let own = settings.resolve(Some(&ghostty), None, Engine::Whisper);
    assert_eq!(own.custom_instructions, "Answer as a shell command.");
    let shared = settings.resolve(Some(&mail), None, Engine::Whisper);
    assert_eq!(shared.custom_instructions, "Translate to English.");
}

#[test]
fn deleted_model_falls_back_to_the_default_rewriter() {
    let settings = ProfileSettings {
        default_style: RewriteStyle::Cleanup,
        ..ProfileSettings::default()
    };
    let orphan = AppProfile {
        rewriter_id: Some("gone".into()),
        ..AppProfile::new("com.apple.mail", "Mail")
    };
    let profile = settings.resolve(Some(&orphan), Some("Mail"), Engine::Whisper);
    assert_eq!(profile.rewriter, RewriteTarget::Apple);
}

#[test]
fn rewrite_targets_list_apple_first_then_the_models() {
    let model = local_model();
    let settings = ProfileSettings { models: vec![model.clone()], ..ProfileSettings::default() };
    let targets = settings.rewrite_targets();
    let ids: Vec<&str> = targets.iter().map(RewriteTarget::id).collect();
    assert_eq!(ids, ["apple", model.id.as_str()]);
    assert_eq!(settings.target(None), None);
}

#[test]
fn settings_survive_a_json_round_trip() {
    let settings = ProfileSettings {
        models: vec![local_model()],
        apps: vec![styled_app("com.mitchellh.ghostty", "Ghostty", RewriteStyle::Prompt)],
        ..ProfileSettings::default()
    };
    let data = serde_json::to_vec(&settings).unwrap();
    let decoded: ProfileSettings = serde_json::from_slice(&data).unwrap();
    assert_eq!(decoded, settings);
}

#[test]
fn reads_and_rewrites_the_mac_apps_file_unchanged() {
    let settings: ProfileSettings = serde_json::from_str(MAC_APP_FILE).unwrap();
    assert_eq!(settings.default_style, RewriteStyle::Cleanup);
    assert_eq!(settings.apps[0].engine, Some(Engine::Scribe));
    assert_eq!(settings.apps[1].rewriter_id, None);
    assert_eq!(settings.models[0].api, ModelApi::Anthropic);
    assert_eq!(settings.models[0].base_url, "https://api.anthropic.com/v1");
    assert_eq!(settings.models[1].key_help_url, None);
    assert_eq!(settings.default_rewriter().display_name(), "Claude");

    let written = serde_json::to_value(&settings).unwrap();
    let original: Value = serde_json::from_str(MAC_APP_FILE).unwrap();
    assert_eq!(written, original);
}

#[test]
fn reading_tolerates_missing_unknown_and_null_keys() {
    let file = json!({
        "defaultStyle": "email",
        "futureSetting": true,
        "apps": [{"bundleID": "notepad.exe", "appName": "Notepad", "engine": null, "rewriterID": null}],
        "models": [{"id": "M1", "name": "Ollama", "api": "openAI", "baseURL": "http://localhost:11434/v1", "keyHelpURL": null}]
    });
    let settings: ProfileSettings = serde_json::from_value(file).unwrap();
    assert_eq!(settings.default_style, RewriteStyle::Email);
    assert_eq!(settings.default_rewriter_id, "apple");
    assert_eq!(settings.apps[0].engine, None);
    assert_eq!(settings.apps[0].custom_instructions, "");
    assert_eq!(settings.models[0].model, "");
    assert_eq!(settings.models[0].key_help_url, None);
}

#[test]
fn saved_settings_load_back_and_a_broken_file_gives_defaults() {
    let path = scratch_file();
    assert_eq!(load_from(&path), ProfileSettings::default());

    let settings = ProfileSettings {
        default_style: RewriteStyle::Chat,
        models: vec![local_model()],
        ..ProfileSettings::default()
    };
    save_to(&path, &settings).unwrap();
    assert_eq!(load_from(&path), settings);

    fs::write(&path, "{ not json").unwrap();
    assert_eq!(load_from(&path), ProfileSettings::default());
    fs::remove_dir_all(path.parent().unwrap()).unwrap();
}

#[test]
fn adding_an_app_twice_keeps_one_and_the_list_stays_sorted() {
    let mut settings = ProfileSettings::default();
    add_app(&mut settings, "com.tinyspeck.slackmacgap", "Slack");
    add_app(&mut settings, "com.apple.mail", "mail");
    add_app(&mut settings, "notepad.exe", "Notepad");
    add_app(&mut settings, "com.apple.mail", "Mail again");
    let names: Vec<&str> = settings.apps.iter().map(|app| app.app_name.as_str()).collect();
    assert_eq!(names, ["mail", "Notepad", "Slack"]);

    remove_app(&mut settings, "notepad.exe");
    assert!(settings.app("notepad.exe").is_none());
    assert_eq!(settings.apps.len(), 2);
}

#[test]
fn first_added_model_becomes_the_default_rewriter() {
    let mut settings = ProfileSettings::default();
    let first = add_model_from_preset(&mut settings, &ModelPreset::LOCAL[0]);
    assert_eq!(settings.default_rewriter_id, first.id);
    let second = add_model_from_preset(&mut settings, &ModelPreset::CLOUD[0]);
    assert_eq!(settings.default_rewriter_id, first.id);
    assert_ne!(first.id, second.id);
    assert_eq!(second.name, "Claude");
    assert_eq!(second.api, ModelApi::Anthropic);
    assert_eq!(second.key_help_url.as_deref(), Some("https://platform.claude.com/settings/keys"));
}

#[test]
fn removing_a_model_clears_every_reference_to_it() {
    let mut settings = ProfileSettings::default();
    let first = add_model_from_preset(&mut settings, &ModelPreset::LOCAL[0]);
    let second = add_model_from_preset(&mut settings, &ModelPreset::CLOUD[1]);
    settings.apps = vec![
        AppProfile {
            rewriter_id: Some(first.id.clone()),
            ..AppProfile::new("com.tinyspeck.slackmacgap", "Slack")
        },
        AppProfile {
            rewriter_id: Some(second.id.clone()),
            ..AppProfile::new("com.apple.mail", "Mail")
        },
    ];

    forget_model(&mut settings, &first.id);
    assert_eq!(settings.models, vec![second.clone()]);
    assert_eq!(settings.default_rewriter_id, second.id);
    assert_eq!(settings.apps[0].rewriter_id, None);
    assert_eq!(settings.apps[1].rewriter_id.as_deref(), Some(second.id.as_str()));

    forget_model(&mut settings, &second.id);
    assert_eq!(settings.default_rewriter_id, "apple");
    assert_eq!(settings.apps[1].rewriter_id, None);
}
