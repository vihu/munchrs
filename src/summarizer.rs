use crate::parser::Symbol;

/// Extract first sentence from docstring (Tier 1).
pub fn extract_summary_from_docstring(docstring: &str) -> String {
    if docstring.is_empty() {
        return String::new();
    }
    let first_line = docstring.trim().lines().next().unwrap_or("").trim();
    let result = if let Some(dot_pos) = first_line.find('.') {
        &first_line[..dot_pos + 1]
    } else {
        first_line
    };
    if result.len() > 120 {
        result[..120].to_string()
    } else {
        result.to_string()
    }
}

/// Generate summary from signature when all else fails (Tier 3).
pub fn signature_fallback(symbol: &Symbol) -> String {
    match symbol.kind.as_str() {
        "class" => format!("Class {}", symbol.name),
        "constant" => format!("Constant {}", symbol.name),
        "type" => format!("Type definition {}", symbol.name),
        _ => {
            if !symbol.signature.is_empty() {
                let sig = &symbol.signature;
                if sig.len() > 120 {
                    sig[..120].to_string()
                } else {
                    sig.clone()
                }
            } else {
                format!("{} {}", symbol.kind, symbol.name)
            }
        }
    }
}

/// Tier 1 + Tier 3: Docstring extraction + signature fallback.
/// No AI required. Fast and deterministic.
pub fn summarize_symbols_simple(symbols: &mut [Symbol]) {
    for sym in symbols.iter_mut() {
        if !sym.summary.is_empty() {
            continue;
        }
        if !sym.docstring.is_empty() {
            sym.summary = extract_summary_from_docstring(&sym.docstring);
        }
        if sym.summary.is_empty() {
            sym.summary = signature_fallback(sym);
        }
    }
}

/// Full three-tier summarization.
///
/// Tier 1: Docstring extraction (free)
/// Tier 2: AI batch summarization (OpenRouter) — async
/// Tier 3: Signature fallback (always works)
#[allow(dead_code)]
pub async fn summarize_symbols(symbols: &mut [Symbol], use_ai: bool) {
    // Tier 1: Extract from docstrings
    for sym in symbols.iter_mut() {
        if !sym.docstring.is_empty() && sym.summary.is_empty() {
            sym.summary = extract_summary_from_docstring(&sym.docstring);
        }
    }

    // Tier 2: AI summarization for remaining symbols
    if use_ai && let Some(client) = OpenRouterClient::from_env() {
        let to_summarize: Vec<usize> = symbols
            .iter()
            .enumerate()
            .filter(|(_, s)| s.summary.is_empty() && s.docstring.is_empty())
            .map(|(i, _)| i)
            .collect();

        for chunk in to_summarize.chunks(10) {
            let batch: Vec<(usize, String, String)> = chunk
                .iter()
                .map(|&i| (i, symbols[i].kind.clone(), symbols[i].signature.clone()))
                .collect();

            if let Ok(summaries) = client.summarize_batch(&batch).await {
                for ((idx, _, _), summary) in batch.iter().zip(summaries.iter()) {
                    if !summary.is_empty() {
                        symbols[*idx].summary = summary.clone();
                    }
                }
            }
        }
    }

    // Tier 3: Signature fallback for any still missing
    for sym in symbols.iter_mut() {
        if sym.summary.is_empty() {
            sym.summary = signature_fallback(sym);
        }
    }
}

#[allow(dead_code)]
struct OpenRouterClient {
    client: reqwest::Client,
    api_key: String,
    base_url: String,
    model: String,
}

#[allow(dead_code)]
impl OpenRouterClient {
    fn from_env() -> Option<Self> {
        let api_key = std::env::var("OPENROUTER_API_KEY").ok()?;
        let base_url = std::env::var("OPENROUTER_BASE_URL")
            .unwrap_or_else(|_| "https://openrouter.ai/api/v1".to_string());
        let model = std::env::var("OPENROUTER_MODEL")
            .unwrap_or_else(|_| "anthropic/claude-haiku-4-5-20251001".to_string());

        Some(Self {
            client: reqwest::Client::new(),
            api_key,
            base_url: base_url.trim_end_matches('/').to_string(),
            model,
        })
    }

    async fn summarize_batch(
        &self,
        batch: &[(usize, String, String)],
    ) -> Result<Vec<String>, reqwest::Error> {
        let mut prompt_lines = vec![
            "Summarize each code symbol in ONE short sentence (max 15 words).".to_string(),
            "Focus on what it does, not how.".to_string(),
            String::new(),
            "Input:".to_string(),
        ];

        for (i, (_, kind, signature)) in batch.iter().enumerate() {
            prompt_lines.push(format!("{}. {}: {}", i + 1, kind, signature));
        }

        prompt_lines.extend([
            String::new(),
            "Output format: NUMBER. SUMMARY".to_string(),
            "Example: 1. Authenticates users with username and password.".to_string(),
            String::new(),
            "Summaries:".to_string(),
        ]);

        let prompt = prompt_lines.join("\n");

        let payload = serde_json::json!({
            "model": self.model,
            "messages": [{"role": "user", "content": prompt}],
            "max_tokens": 500,
            "temperature": 0.0,
        });

        let resp = self
            .client
            .post(format!("{}/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&payload)
            .send()
            .await?
            .json::<serde_json::Value>()
            .await?;

        let text = resp["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("");

        Ok(parse_numbered_response(text, batch.len()))
    }
}

#[allow(dead_code)]
fn parse_numbered_response(text: &str, expected_count: usize) -> Vec<String> {
    let mut summaries = vec![String::new(); expected_count];
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(dot_pos) = line.find('.')
            && let Ok(num) = line[..dot_pos].trim().parse::<usize>()
            && num >= 1
            && num <= expected_count
        {
            summaries[num - 1] = line[dot_pos + 1..].trim().to_string();
        }
    }
    summaries
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_summary_from_docstring() {
        assert_eq!(
            extract_summary_from_docstring("Check if a file is binary. Uses multiple heuristics."),
            "Check if a file is binary."
        );
        assert_eq!(
            extract_summary_from_docstring("Simple one-liner"),
            "Simple one-liner"
        );
        assert_eq!(extract_summary_from_docstring(""), "");
    }

    #[test]
    fn test_parse_numbered_response() {
        let text = "1. Does thing A.\n2. Does thing B.\n3. Does thing C.";
        let result = parse_numbered_response(text, 3);
        assert_eq!(
            result,
            vec!["Does thing A.", "Does thing B.", "Does thing C."]
        );
    }
}
