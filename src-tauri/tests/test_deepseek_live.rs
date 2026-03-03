//! Live test for DeepSeek API using reasoner model.
//! To run: cargo test --test test_deepseek_live -- --nocapture --ignored

use crystal_canvas::crystal_state::CrystalState;
use crystal_canvas::llm::context::build_crystal_context;
use crystal_canvas::llm::prompt::build_messages;
use crystal_canvas::llm::provider::{ProviderConfig, create_provider};

#[tokio::test]
#[ignore] // Ignore by default to avoid CI hanging if no network/key
async fn test_deepseek_live_api() {
    // DeepSeek API key provided by user (use env var for safety)
    let api_key =
        std::env::var("DEEPSEEK_API_KEY").unwrap_or_else(|_| "your_api_key_here".to_string());
    let model = "deepseek-reasoner".to_string();

    let config = ProviderConfig::DeepSeek { api_key, model };
    let provider = create_provider(&config);

    // Give it a generic default cell so reasoner understands
    let state = CrystalState {
        cell_a: 5.0,
        cell_b: 5.0,
        cell_c: 5.0,
        ..Default::default()
    };

    let context = build_crystal_context(&state, None);

    let user_input = "Please add a Silicon atom at the center of the unit cell (0.5, 0.5, 0.5).";
    let messages = build_messages(&context, user_input);

    println!("==================================================");
    println!(
        "Sending request to DeepSeek API ({})",
        config_model(&config)
    );
    println!("Prompt user input: {}", user_input);
    println!("Wait for reasoning and response...");
    println!("==================================================");

    // Record start time
    let start_time = std::time::Instant::now();

    let response = provider.chat(&messages).await;

    let duration = start_time.elapsed();

    match response {
        Ok(res) => {
            println!("\n[Success] Received response in {:.2?}:\n", duration);
            println!("--- RAW RESPONSE ---");
            println!("{}", res);
            println!("--------------------");

            // Parse test
            let json_result: Result<crystal_canvas::llm::command::CrystalCommand, _> =
                serde_json::from_str(&res);
            match json_result {
                Ok(cmd) => {
                    println!("✅ Perfectly parsed JSON matching CrystalCommand schema!");
                    println!("Parsed: {:?}", cmd);
                }
                Err(_) => {
                    // Check if it's wrapped in a markdown block
                    if let Some(json_block) = extract_markdown_json(&res) {
                        let block_result: Result<crystal_canvas::llm::command::CrystalCommand, _> =
                            serde_json::from_str(&json_block);
                        if let Ok(cmd) = block_result {
                            println!(
                                "⚠️ Response was wrapped in markdown ```json ... ``` but parsed successfully!"
                            );
                            println!("Parsed: {:?}", cmd);
                        } else {
                            println!(
                                "❌ Response contained a markdown block but failed schema validation."
                            );
                        }
                    } else {
                        println!(
                            "❌ Response was not clean JSON and no markdown code block found."
                        );
                    }
                }
            }
        }
        Err(e) => {
            println!("\n[Error] API call failed in {:.2?}:\n{}", duration, e);
            panic!("DeepSeek API test failed!");
        }
    }
}

fn config_model(c: &ProviderConfig) -> &str {
    match c {
        ProviderConfig::DeepSeek { model, .. } => model,
        _ => "Unknown",
    }
}

fn extract_markdown_json(text: &str) -> Option<String> {
    let mut in_block = false;
    let mut block = String::new();

    for line in text.lines() {
        if line.trim().starts_with("```") {
            if in_block {
                return Some(block);
            } else {
                in_block = true;
                continue;
            }
        }
        if in_block {
            block.push_str(line);
            block.push('\n');
        }
    }
    None
}
