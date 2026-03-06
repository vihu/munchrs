use std::fs;
use std::path::PathBuf;

const SAVINGS_FILE: &str = "_savings.json";
const BYTES_PER_TOKEN: usize = 4;

/// Input token pricing ($ per token).
const PRICING_CLAUDE_OPUS: f64 = 15.00 / 1_000_000.0;
const PRICING_GPT5_LATEST: f64 = 10.00 / 1_000_000.0;

fn savings_path(base_path: Option<&str>) -> PathBuf {
    let root = match base_path {
        Some(p) => PathBuf::from(p),
        None => dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".code-index"),
    };
    let _ = fs::create_dir_all(&root);
    root.join(SAVINGS_FILE)
}

/// Add tokens_saved to the running total. Returns new cumulative total.
pub fn record_savings(tokens_saved: usize, base_path: Option<&str>) -> usize {
    let path = savings_path(base_path);
    let data: serde_json::Value = if path.exists() {
        fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or(serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    let total = data
        .get("total_tokens_saved")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as usize
        + tokens_saved;

    let updated = serde_json::json!({ "total_tokens_saved": total });
    let _ = fs::write(&path, serde_json::to_string(&updated).unwrap_or_default());
    total
}

/// Return the current cumulative total without modifying it.
pub fn get_total_saved(base_path: Option<&str>) -> usize {
    let path = savings_path(base_path);
    if !path.exists() {
        return 0;
    }
    fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .and_then(|v| v.get("total_tokens_saved").and_then(|v| v.as_u64()))
        .unwrap_or(0) as usize
}

/// Estimate tokens saved: (raw - response) / bytes_per_token.
pub fn estimate_savings(raw_bytes: usize, response_bytes: usize) -> usize {
    if raw_bytes > response_bytes {
        (raw_bytes - response_bytes) / BYTES_PER_TOKEN
    } else {
        0
    }
}

/// Return cost avoided estimates for this call and the running total.
pub fn cost_avoided(tokens_saved: usize, total_tokens_saved: usize) -> serde_json::Value {
    serde_json::json!({
        "cost_avoided": {
            "claude_opus": round4(tokens_saved as f64 * PRICING_CLAUDE_OPUS),
            "gpt5_latest": round4(tokens_saved as f64 * PRICING_GPT5_LATEST),
        },
        "total_cost_avoided": {
            "claude_opus": round4(total_tokens_saved as f64 * PRICING_CLAUDE_OPUS),
            "gpt5_latest": round4(total_tokens_saved as f64 * PRICING_GPT5_LATEST),
        },
    })
}

fn round4(v: f64) -> f64 {
    (v * 10000.0).round() / 10000.0
}
