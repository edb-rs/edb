//! Show proxy status command

use eyre::Result;
use serde_json::json;
use std::time::Duration;

/// Show the status of RPC proxy providers
pub async fn show_proxy_status(cli: &crate::Cli) -> Result<()> {
    tracing::info!("Checking proxy status...");

    // Query provider status
    let client = reqwest::Client::new();
    let request = json!({
        "jsonrpc": "2.0",
        "method": "edb_providers",
        "params": [],
        "id": 1
    });

    let response = client
        .post(&format!("http://127.0.0.1:{}", cli.proxy_port))
        .json(&request)
        .timeout(Duration::from_secs(5))
        .send()
        .await?;

    let response_json: serde_json::Value = response.json().await?;

    if let Some(error) = response_json.get("error") {
        println!("❌ Error getting proxy status: {}", error);
        return Ok(());
    }

    if let Some(result) = response_json.get("result") {
        let healthy_count = result["healthy_count"].as_u64().unwrap_or(0);
        let total_count = result["total_count"].as_u64().unwrap_or(0);
        let empty_providers = vec![];
        let providers = result["providers"].as_array().unwrap_or(&empty_providers);

        println!("🌐 EDB RPC Proxy Status");
        println!("=====================");
        println!("📊 Provider Summary: {}/{} healthy", healthy_count, total_count);
        println!();

        for (i, provider) in providers.iter().enumerate() {
            let url = provider["url"].as_str().unwrap_or("unknown");
            let is_healthy = provider["is_healthy"].as_bool().unwrap_or(false);
            let response_time = provider["response_time_ms"].as_u64();
            let failures = provider["consecutive_failures"].as_u64().unwrap_or(0);
            let last_check = provider["last_health_check_seconds_ago"].as_u64();

            let status_emoji = if is_healthy { "✅" } else { "❌" };
            let status_text = if is_healthy { "Healthy" } else { "Unhealthy" };

            println!("{}. {} {}", i + 1, status_emoji, status_text);
            println!("   URL: {}", url);

            if let Some(rt) = response_time {
                println!("   Response Time: {}ms", rt);
            }

            if failures > 0 {
                println!("   Failures: {}", failures);
            }

            if let Some(last) = last_check {
                if last < 60 {
                    println!("   Last Check: {}s ago", last);
                } else if last < 3600 {
                    println!("   Last Check: {}m ago", last / 60);
                } else {
                    println!("   Last Check: {}h ago", last / 3600);
                }
            }
            println!();
        }

        if healthy_count == 0 {
            println!("⚠️  Warning: No healthy providers available!");
            println!("   The proxy will attempt to health-check providers automatically.");
        } else if healthy_count < total_count {
            println!("⚠️  Some providers are unhealthy but {} are still working.", healthy_count);
        } else {
            println!("✨ All providers are healthy!");
        }
    } else {
        println!("❌ Unexpected response format from proxy");
    }

    Ok(())
}
