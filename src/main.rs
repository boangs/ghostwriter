use anyhow::Result;
use std::sync::{Arc, Mutex};
use std::thread;
use std::env;

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

    // 设置环境变量
    if let Some(url) = args.engine_base_url.clone() {
        env::set_var("ENGINE_BASE_URL", url);
    }
    
    if let Some(api_key) = args.engine_api_key.clone() {
        env::set_var("OPENAI_API_KEY", api_key);
    }

    let mut engine_options = OptionMap::new();
    engine_options.insert("model".to_string(), args.model.clone());

    let engine: Arc<Mutex<Box<dyn LLMEngine>>> = match args.engine.as_deref().unwrap_or("openai") {
        "openai" => Arc::new(Mutex::new(Box::new(OpenAI::new(&engine_options)?))),
        "anthropic" => Arc::new(Mutex::new(Box::new(Anthropic::new(&engine_options)?))),
        "google" => Arc::new(Mutex::new(Box::new(Google::new(&engine_options)?))),
        _ => panic!("不支持的引擎类型"),
    };

    let pen = Arc::new(Mutex::new(Pen::new(args.no_draw)));
    let touch = Arc::new(Mutex::new(Touch::new(args.no_draw)));
    let keyboard = Arc::new(Mutex::new(Keyboard::new(args.no_draw, args.no_draw_progress)));

    println!("等待触发（触摸右上角）...");
    
    loop {
        {
            let mut pen = pen.lock().unwrap();
            pen.handle_pen_input()?;
        }

        let mut touch = touch.lock().unwrap();
        if touch.wait_for_touch()? {
            println!("检测到触摸，开始 AI 交互");
            drop(touch);
            
            let test_message = "你好";
            println!("发送给 AI 的消息: {}", test_message);
            
            let mut engine = engine.lock().unwrap();
            engine.add_text_content(test_message);
            
            if let Err(e) = engine.execute() {
                println!("API 调用失败: {}", e);
                continue;
            }

            if let Some(response) = engine.get_response() {
                println!("\nAI 回复: {}", response);
                let mut pen = pen.lock().unwrap();
                pen.draw_text(&response, (100, 100), 32.0)?;
            }
            
            engine.clear_content();
        }
        
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
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
        // 将 Vec<Vec<bool>> 转换为 Vec<u8>
        let flat_bitmap: Vec<u8> = bitmap
            .iter()
            .flat_map(|row| row.iter().map(|&pixel| if pixel { 0 } else { 255 }))
            .collect();
        pen.draw_bitmap(&flat_bitmap)?;
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
    let keyboard = shared!(Keyboard::new(args.no_draw, args.no_draw_progress));
    let pen = shared!(Pen::new(args.no_draw));
    let touch = shared!(Touch::new(args.no_draw));

    // 设置环境变量
    if let Some(url) = args.engine_base_url.clone() {
        env::set_var("ENGINE_BASE_URL", url);
    }
    
    if let Some(api_key) = args.engine_api_key.clone() {
        env::set_var("OPENAI_API_KEY", api_key);
    }

    let mut engine_options = OptionMap::new();
    engine_options.insert("model".to_string(), args.model.clone());

    let engine = match args.engine.as_deref().unwrap_or("openai") {
        "openai" => OpenAI::new(&engine_options)?,
        "anthropic" => Anthropic::new(&engine_options)?,
        "google" => Google::new(&engine_options)?,
        _ => panic!("不支持的引擎类型"),
    };

    let engine = shared!(engine);

    println!("等待触发（触摸右上角）...");
    
    loop {
        let mut touch = touch.lock().unwrap();
        if touch.wait_for_touch()? {
            println!("检测到触摸，开始 AI 交互");
            drop(touch);
            
            let test_message = "你好";
            println!("发送给 AI 的消息: {}", test_message);
            
            let mut engine = engine.lock().unwrap();
            engine.add_text_content(test_message);
            
            if let Err(e) = engine.execute() {
                println!("API 调用失败: {}", e);
                continue;
            }

            if let Some(response) = engine.get_response() {
                println!("\nAI 回复: {}", response);
                let mut pen = pen.lock().unwrap();
                pen.draw_text(&response, (100, 100), 32.0)?;
            }
            
            engine.clear_content();
        }
        
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}
