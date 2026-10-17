//! Synthesize a single message with a service from a `*.tts.toml` config.
//!
//! ```text
//! cargo run --example synth -- --tts-config-path discord-tts.tts.toml \
//!     --service-id BingSpeech --style-id ja-JP/ja-JP-KeitaNeural --text "これはテスト"
//! ```

use std::path::PathBuf;
use std::time::Instant;

use anyhow::{Context, Result};
use clap::Parser;
use tracing::{debug, info};
use tracing_subscriber::EnvFilter;
use tts_bot_core::model::TtsConfig;
use tts_bot_core::tts::TtsServices;

#[derive(Parser, Debug)]
#[command(about, version)]
struct Cli {
    /// Path of the `*.tts.toml` configuration file.
    #[arg(long, env = "TTS_CONFIG_PATH", default_value = "discord-tts.tts.toml")]
    tts_config_path: PathBuf,

    /// Service id of the config, defaults to the configured default style.
    #[arg(long, env = "TTS_SERVICE_ID")]
    service_id: Option<String>,

    /// Style id of the service, defaults to the configured default style.
    #[arg(long, env = "TTS_STYLE_ID")]
    style_id: Option<String>,

    /// Text to synthesize.
    #[arg(long, env = "TTS_TEXT", default_value = "これはテスト")]
    text: String,

    /// Where to write the resulting wav, defaults to `synth.wav`.
    #[arg(long, env = "TTS_OUTPUT", default_value = "synth.wav")]
    output: PathBuf,

    /// List the styles of every configured service instead of synthesizing.
    #[arg(long)]
    list_styles: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();
    let config = TtsConfig::new(&cli.tts_config_path)
        .with_context(|| format!("failed to read {}", cli.tts_config_path.display()))?;

    info!(
        config = ?cli.tts_config_path,
        services = config.tts_services.len(),
        "loaded tts config"
    );

    let services = TtsServices::new();
    let started = Instant::now();
    services
        .initialize(&config)
        .await
        .context("failed to initialize the tts services")?;
    info!(
        elapsed_ms = started.elapsed().as_millis(),
        "tts services initialized"
    );

    if cli.list_styles {
        for (service_id, characters) in services.styles().await {
            info!("service {service_id}");
            for character in characters {
                for style in character.styles {
                    info!("  {}/{}", character.name, style.id);
                }
            }
        }
        return Ok(());
    }

    let service_id = cli
        .service_id
        .as_deref()
        .unwrap_or(&config.default_style.service_id);
    let style_id = cli
        .style_id
        .as_deref()
        .unwrap_or(&config.default_style.style_id);
    info!(service_id, style_id, text = %cli.text, "synthesizing");

    let started = Instant::now();
    let available = services.is_available(service_id, style_id).await;
    info!(available, "service availability checked");

    let wav = services
        .tts(service_id, style_id, &cli.text)
        .await
        .with_context(|| format!("failed to synthesize with {service_id}/{style_id}"))?;
    info!(
        bytes = wav.len(),
        elapsed_ms = started.elapsed().as_millis(),
        "synthesis finished"
    );

    std::fs::write(&cli.output, &wav)
        .with_context(|| format!("failed to write {}", cli.output.display()))?;
    debug!(output = ?cli.output, "wav written");

    println!("wrote {} bytes to {}", wav.len(), cli.output.display());
    Ok(())
}
