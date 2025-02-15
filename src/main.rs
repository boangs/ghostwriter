use anyhow::Result;
use std::sync::{Arc, Mutex};

use serde_json::Value as json;

use clap::Parser;

use base64::prelude::*;

use dotenv::dotenv;

use rust_embed::Embed;

use ghostwriter::{
    keyboard::Keyboard,
    llm_engine::{anthropic::Anthropic, openai::OpenAI, google::Google, LLMEngine},
    pen::Pen,
    screenshot::Screenshot,
    segmenter::analyze_image,
    touch::Touch,
    util::{svg_to_bitmap, write_bitmap_to_file, OptionMap},
};

const REMARKABLE_WIDTH: u32 = 768;
const REMARKABLE_HEIGHT: u32 = 1024;

#[derive(Embed)]
#[folder = "prompts/"]
struct Asset;

#[derive(Parser)]
#[command(author, version)]
#[command(about = "Vision-LLM Agent for the reMarkable2")]
#[command(
    long_about = "Ghostwriter is an exploration of how to interact with vision-LLM through the handwritten medium of the reMarkable2. It is a pluggable system; you can provide a custom prompt and custom 'tools' that the agent can use."
)]
#[command(after_help = "See https://github.com/awwaiid/ghostwriter for updates!")]
struct Args {
    /// Sets the engine to use (openai, anthropic);
    /// Sometimes we can guess the engine from the model name
    #[arg(long)]
    engine: Option<String>,

    /// Sets the base URL for the engine API;
    /// Or use environment variable OPENAI_BASE_URL or ANTHROPIC_BASE_URL
    #[arg(long)]
    engine_base_url: Option<String>,

    /// Sets the API key for the engine;
    /// Or use environment variable OPENAI_API_KEY or ANTHROPIC_API_KEY
    #[arg(long)]
    engine_api_key: Option<String>,

    /// Sets the model to use
    #[arg(long, short, default_value = "claude-3-5-sonnet-latest")]
    model: String,

    /// Sets the prompt to use
    #[arg(long, default_value = "general.json")]
    prompt: String,

    /// Do not actually submit to the model, for testing
    #[arg(short, long)]
    no_submit: bool,

    /// Skip running draw_text or draw_svg
    #[arg(long)]
    no_draw: bool,

    /// Disable keyboard progress
    #[arg(long)]
    no_draw_progress: bool,

    /// Input PNG file for testing
    #[arg(long)]
    input_png: Option<String>,

    /// Output file for testing
    #[arg(long)]
    output_file: Option<String>,

    /// Output file for model parameters
    #[arg(long)]
    model_output_file: Option<String>,

    /// Save screenshot filename
    #[arg(long)]
    save_screenshot: Option<String>,

    /// Save bitmap filename
    #[arg(long)]
    save_bitmap: Option<String>,

    /// Disable looping
    #[arg(long)]
    no_loop: bool,

    /// Disable waiting for trigger
    #[arg(long)]
    no_trigger: bool,

    /// Apply segmentation
    #[arg(long)]
    apply_segmentation: bool,
}

fn main() -> Result<()> {
    dotenv().ok();
    let args = Args::parse();

    ghostwriter(&args)
}

macro_rules! shared {
   ($x:expr) => { Arc::new(Mutex::new($x)) }
}

macro_rules! lock {
   ($x:expr) => { $x.lock().unwrap() }
}

fn draw_text(text: &str, keyboard: &mut Keyboard, pen: &mut Pen) -> Result<()> {
    keyboard.progress()?;
    // 使用 pen 来绘制文本
    pen.draw_text(text, (100, 100), 24.0)?;
    keyboard.progress_end()?;
    Ok(())
}

fn draw_svg(
    svg_data: &str,
    keyboard: &mut Keyboard,
    pen: &mut Pen,
    save_bitmap: Option<&String>,
    no_draw: bool,
) -> Result<()> {
    keyboard.progress()?;
    let bitmap = svg_to_bitmap(svg_data, REMARKABLE_WIDTH, REMARKABLE_HEIGHT)?;
    if let Some(save_bitmap) = save_bitmap {
        write_bitmap_to_file(&bitmap, save_bitmap)?;
    }
    if !no_draw {
        pen.draw_bitmap(&bitmap)?;
    }
    keyboard.progress_end()?;
    Ok(())
}

fn load_config(filename: &str) -> String {
    // println!("Loading config from {}", filename);

    if std::path::Path::new(filename).exists() {
        std::fs::read_to_string(filename).unwrap()
    } else {
        std::str::from_utf8(Asset::get(filename).unwrap().data.as_ref())
            .unwrap()
            .to_string()
    }
}

fn ghostwriter(args: &Args) -> Result<()> {
    let keyboard = shared!(Keyboard::new(
        args.no_draw,
        args.no_draw_progress,
    ));
    let pen = shared!(Pen::new(args.no_draw));
    let touch = shared!(Touch::new(args.no_draw));

    let mut engine_options = OptionMap::new();

    let model = args.model.clone();
    engine_options.insert("model".to_string(), model.clone());

    let engine_name = if let Some(engine) = args.engine.clone() {
        engine.to_string()
    } else {
        if model.starts_with("gpt") {
            "openai".to_string()
        } else if model.starts_with("claude") {
            "anthropic".to_string()
        } else if model.starts_with("gemini") {
            "google".to_string()
        } else {
            panic!("Unable to guess engine from model name {}", model)
        }
    };

    if args.engine_base_url.is_some() {
        engine_options.insert(
            "base_url".to_string(),
            args.engine_base_url.clone().unwrap(),
        );
    }
    if args.engine_api_key.is_some() {
        engine_options.insert("api_key".to_string(), args.engine_api_key.clone().unwrap());
    }

    let mut engine: Box<dyn LLMEngine> = match engine_name.as_str() {
        "openai" => Box::new(OpenAI::new(&engine_options)),
        "anthropic" => Box::new(Anthropic::new(&engine_options)),
        "google" => Box::new(Google::new(&engine_options)),
        _ => panic!("Unknown engine {}", engine_name),
    };

    // 添加测试文本
    engine.add_text_content("你好");
    
    // 执行 API 调用
    if let Err(e) = engine.execute() {
        println!("API 调用失败: {}", e);
        return Err(e);
    }

    // 获取响应并绘制到屏幕上
    if let Some(response) = engine.get_response() {
        println!("AI 回复: {}", response);
        
        // 使用 keyboard 绘制文本
        let mut keyboard = keyboard.lock().unwrap();
        keyboard.draw_text(&response, 100, 100)?;  // 从坐标 (100,100) 开始绘制
    }

    Ok(())
}
