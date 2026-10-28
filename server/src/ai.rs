// SPDX-License-Identifier: AGPL-3.0-or-later

//! An external AI the operator may connect for reading first pages (`NOTENWART_AI_*`). Off unless configured: the
//! page image and its text leave the server for the provider the operator chose. Two kinds of API:
//! Anthropic's Messages API, and the OpenAI-compatible chat completions API that many providers and local servers
//! (Ollama, LM Studio, vLLM …) offer.

use std::time::Duration;

use base64::Engine;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::recognize::{Fields, Known, Line};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provider {
    Anthropic,
    Openai,
}

#[derive(Debug, Clone)]
pub struct AiConfig {
    pub provider: Provider,
    /// Base URL; defaults to the provider's public API.
    pub url: Option<String>,
    pub key: Option<String>,
    pub model: Option<String>,
    /// Send the page image (needs a model that reads images).
    pub images: bool,
}

pub struct Ai {
    config: AiConfig,
    model: String,
    url: String,
    http: reqwest::Client,
}

pub struct AiInput<'a> {
    pub lines: &'a [Line],
    pub image: Option<&'a [u8]>,
    pub instruments: &'a [Known],
}

/// Fields the model is asked for; every one is a string, empty when the page does not say.
const FIELDS: [&str; 12] = [
    "title",
    "subtitle",
    "composer",
    "arranger",
    "lyricist",
    "publisher",
    "part",
    "instrument",
    "voice",
    "clef",
    "key",
    "timeSignature",
];

impl Ai {
    pub fn new(config: AiConfig) -> anyhow::Result<Self> {
        let model = match (&config.model, config.provider) {
            (Some(model), _) => model.clone(),
            (None, Provider::Anthropic) => "claude-opus-5-5".into(),
            (None, Provider::Openai) => anyhow::bail!("NOTENWART_AI_MODEL is required for an OpenAI-compatible API"),
        };
        let url = match (&config.url, config.provider) {
            (Some(url), _) => url.trim_end_matches('/').to_string(),
            (None, Provider::Anthropic) => "https://api.anthropic.com".into(),
            (None, Provider::Openai) => "https://api.openai.com/v1".into(),
        };
        if config.provider == Provider::Anthropic && config.key.is_none() {
            anyhow::bail!("NOTENWART_AI_KEY is required for Anthropic");
        }
        let http = reqwest::Client::builder().timeout(Duration::from_secs(120)).build()?;
        Ok(Self {
            config,
            model,
            url,
            http,
        })
    }

    pub fn describe(&self) -> String {
        format!("{:?} {} at {}", self.config.provider, self.model, self.url)
    }

    fn prompt(input: &AiInput) -> String {
        let names: Vec<&str> = input.instruments.iter().map(|i| i.name.as_str()).collect();
        let text: Vec<String> = input
            .lines
            .iter()
            .map(|l| {
                format!(
                    "[y={:.2} x={:.2}–{:.2} h={:.3}] {}",
                    l.y0,
                    l.x0,
                    l.x1,
                    l.y1 - l.y0,
                    l.text
                )
            })
            .collect();
        format!(
            "This is the first page of a sheet music PDF from a music school's library. Read what it says about \
             the piece and fill in the fields for a library catalogue.\n\n\
             - title, subtitle: as printed (the subtitle is often the genre or a second title).\n\
             - composer, arranger, lyricist: names only, without labels like \"Musik:\" or \"arr.\" and without life \
             dates.\n\
             - publisher: the publishing house from the copyright line, without year and \"©\".\n\
             - part: the part as printed, e.g. \"2. Trompete in B\" or \"Full Score\".\n\
             - instrument: exactly one of these names, or empty for a full score or choir: {names}.\n\
             - voice: the part number (\"2\" for \"2. Trompete\"), else empty.\n\
             - clef, key, timeSignature: from the very beginning of the first staff only – clef as treble, bass, \
             alto, tenor or percussion; key as written for this part, e.g. \"B♭ major\" or \"G minor\"; time signature \
             like \"4/4\" or \"6/8\".\n\
             Leave a field empty when the page does not show it; do not guess from general knowledge.\n\n\
             Text found on the page (positions as fractions of the page, h = text height):\n{text}",
            names = names.join(", "),
            text = if text.is_empty() {
                "(none – read the image)".into()
            } else {
                text.join("\n")
            },
        )
    }

    fn schema() -> Value {
        let properties: serde_json::Map<String, Value> = FIELDS
            .iter()
            .map(|f| (f.to_string(), json!({ "type": "string" })))
            .collect();
        json!({
            "type": "object",
            "properties": properties,
            "required": FIELDS,
            "additionalProperties": false
        })
    }

    /// Asks the model; the answer comes back as fields (the instrument already mapped to the catalogue).
    pub async fn read(&self, input: AiInput<'_>) -> anyhow::Result<Fields> {
        let prompt = Self::prompt(&input);
        let image = input
            .image
            .filter(|_| self.config.images)
            .map(|png| base64::engine::general_purpose::STANDARD.encode(png));
        let answer = match self.config.provider {
            Provider::Anthropic => self.anthropic(&prompt, image.as_deref()).await?,
            Provider::Openai => self.openai(&prompt, image.as_deref()).await?,
        };
        Ok(to_fields(&answer, input.instruments))
    }

    async fn anthropic(&self, prompt: &str, image: Option<&str>) -> anyhow::Result<Value> {
        let mut content = vec![];
        if let Some(data) = image {
            content.push(json!({
                "type": "image",
                "source": { "type": "base64", "media_type": "image/png", "data": data }
            }));
        }
        content.push(json!({ "type": "text", "text": prompt }));
        let mut body = json!({
            "model": self.model,
            "max_tokens": 16000,
            "output_config": { "format": { "type": "json_schema", "schema": Self::schema() } },
            "messages": [{ "role": "user", "content": content }]
        });
        let mut request = self
            .http
            .post(format!("{}/v1/messages", self.url))
            .header("x-api-key", self.config.key.as_deref().unwrap_or_default())
            .header("anthropic-version", "2023-06-01");
        // Current models: a quick read is enough, and a declined request falls back to another model.
        if [
            "claude-opus-5-5",
            "claude-opus-5",
            "claude-fable-5-1",
            "claude-sonnet-5-5",
        ]
        .contains(&self.model.as_str())
        {
            body["output_config"]["effort"] = json!("low");
            if self.url == "https://api.anthropic.com" {
                body["fallbacks"] = json!("default");
                request = request.header("anthropic-beta", "server-side-fallback-2026-07-01");
            }
        }
        let response: Value = request.json(&body).send().await?.error_for_status()?.json().await?;
        if response["stop_reason"] == "refusal" {
            anyhow::bail!("the model declined");
        }
        let text: String = response["content"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|b| b["type"] == "text")
            .filter_map(|b| b["text"].as_str())
            .collect();
        parse_json(&text)
    }

    async fn openai(&self, prompt: &str, image: Option<&str>) -> anyhow::Result<Value> {
        let mut content = vec![json!({ "type": "text", "text": prompt })];
        if let Some(data) = image {
            content.push(json!({
                "type": "image_url",
                "image_url": { "url": format!("data:image/png;base64,{data}") }
            }));
        }
        let body = json!({
            "model": self.model,
            "messages": [
                { "role": "system", "content": format!(
                    "Answer with one JSON object with exactly these string fields: {}.", FIELDS.join(", ")) },
                { "role": "user", "content": content }
            ],
            "response_format": { "type": "json_object" }
        });
        let mut request = self.http.post(format!("{}/chat/completions", self.url));
        if let Some(key) = &self.config.key {
            request = request.bearer_auth(key);
        }
        let response: Value = request.json(&body).send().await?.error_for_status()?.json().await?;
        parse_json(
            response["choices"][0]["message"]["content"]
                .as_str()
                .unwrap_or_default(),
        )
    }
}

/// The JSON object in a model's answer (some wrap it in a code fence).
fn parse_json(text: &str) -> anyhow::Result<Value> {
    let start = text.find('{').ok_or_else(|| anyhow::anyhow!("no JSON in the answer"))?;
    let end = text
        .rfind('}')
        .ok_or_else(|| anyhow::anyhow!("no JSON in the answer"))?;
    Ok(serde_json::from_str(&text[start..=end])?)
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct Answer {
    title: String,
    subtitle: String,
    composer: String,
    arranger: String,
    lyricist: String,
    publisher: String,
    part: String,
    instrument: String,
    voice: Value,
    clef: String,
    key: String,
    time_signature: String,
}

fn to_fields(answer: &Value, instruments: &[Known]) -> Fields {
    let answer: Answer = serde_json::from_value(answer.clone()).unwrap_or_default();
    let some = |s: String| {
        let s = s.trim().to_string();
        (!s.is_empty()).then_some(s)
    };
    let instrument = answer.instrument.trim().to_lowercase();
    let instrument_id = instruments
        .iter()
        .find(|i| !instrument.is_empty() && i.name.to_lowercase() == instrument)
        .map(|i| i.id);
    let voice = match &answer.voice {
        Value::Number(n) => n.as_i64(),
        Value::String(s) => s.trim().trim_end_matches('.').parse().ok(),
        _ => None,
    }
    .and_then(|v| i16::try_from(v).ok())
    .filter(|v| (1..=99).contains(v));
    Fields {
        title: some(answer.title),
        subtitle: some(answer.subtitle),
        composer: some(answer.composer),
        arranger: some(answer.arranger),
        lyricist: some(answer.lyricist),
        publisher: some(answer.publisher),
        part: some(answer.part),
        instrument_id,
        voice,
        clef: some(answer.clef),
        key: some(answer.key),
        time_signature: some(answer.time_signature),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn answers_become_fields() {
        let trumpet = Known {
            id: Uuid::new_v4(),
            name: "Trompete".into(),
            aliases: vec![],
        };
        let answer = parse_json(
            "```json\n{\"title\": \"Stille Nacht\", \"composer\": \"Franz Xaver Gruber\", \"instrument\": \"trompete\", \
             \"voice\": \"2.\", \"key\": \"B♭ major\", \"arranger\": \"\"}\n```",
        )
        .unwrap();
        let fields = to_fields(&answer, std::slice::from_ref(&trumpet));
        assert_eq!(fields.title.as_deref(), Some("Stille Nacht"));
        assert_eq!(fields.instrument_id, Some(trumpet.id));
        assert_eq!(fields.voice, Some(2));
        assert_eq!(fields.arranger, None);
        assert_eq!(fields.key.as_deref(), Some("B♭ major"));
    }
}
