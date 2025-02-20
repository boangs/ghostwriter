use anyhow::Result;
use base64::prelude::*;
use clap::Parser;
use dotenv::dotenv;
use env_logger;
use log::{debug, info, error};
use rust_embed::Embed;
use serde_json::Value as json;
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::Duration;
use std::collections::HashMap;
use std::env;

use ghostwriter::{
    keyboard::Keyboard,
    llm_engine::{anthropic::Anthropic, google::Google, openai::OpenAI, LLMEngine},
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

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// LLM engine to use
    #[arg(long)]
    engine: String,

    /// LLM engine base URL
    #[arg(long)]
    engine_base_url: String,

    /// LLM engine API key
    #[arg(long)]
    engine_api_key: String,

    /// LLM model to use
    #[arg(long)]
    model: String,

    /// Save screenshot filename
    #[arg(long)]
    save_screenshot: Option<String>,

    /// Save bitmap filename
    #[arg(long)]
    save_bitmap: Option<String>,

    /// Output file
    #[arg(long)]
    output_file: Option<String>,

    /// Disable drawing
    #[arg(long)]
    no_draw: bool,

    /// Disable looping
    #[arg(long)]
    no_loop: bool,

    /// Disable waiting for trigger
    #[arg(long)]
    no_trigger: bool,

    /// Apply segmentation
    #[arg(long)]
    apply_segmentation: bool,

    /// Set the log level. Try 'debug' or 'trace'
    #[arg(long, default_value = "info")]
    log_level: String,

    #[arg(long, default_value = "你好，写一篇200字的笑话")]
    initial_text: String,
}

fn main() -> Result<()> {
    dotenv().ok();
    let args = Args::parse();

    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or(args.log_level.as_str()),
    )
    .init();

    // 创建键盘实例
    let keyboard = Keyboard::new(args.no_draw, args.no_trigger)?;
    
    // 获取 AI 回复
    let mut options = HashMap::new();
    options.insert("content".to_string(), args.initial_text.clone());
    options.insert("api_key".to_string(), args.engine_api_key.clone());
    options.insert("base_url".to_string(), args.engine_base_url.clone());
    options.insert("model".to_string(), args.model.clone());
    
    let mut engine = OpenAI::new(&options);
    
    // 添加文本内容
    engine.add_text_content(&args.initial_text);
    
    // 注册回调函数来处理 AI 的回复
    let response = Arc::new(Mutex::new(String::new()));
    let response_clone = response.clone();
    
    engine.register_tool(
        "write",
        json!({
            "name": "write",
            "description": "Write the response text",
            "parameters": {
                "type": "object",
                "properties": {
                    "text": {
                        "type": "string",
                        "description": "The text to write"
                    }
                },
                "required": ["text"]
            }
        }),
        Box::new(move |args| {
            let text = args["text"].as_str().unwrap_or_default();
            *response_clone.lock().unwrap() = text.to_string();
        })
    );

    // 执行并获取回复
    if let Err(e) = engine.execute() {
        error!("获取 AI 回复失败: {}", e);
        return Err(anyhow::anyhow!("AI 回复失败: {}", e));
    }

    let response_text = response.lock().unwrap().clone();
    info!("收到 AI 回复: {}", response_text);

    // 绘制 AI 回复的文字
    info!("开始绘制 AI 回复");
    keyboard.write_text(&response_text)?;
    
    if args.no_loop {
        Ok(response_text)
    } else {
        Ok(response_text)
    }
}

macro_rules! shared {
    ($x:expr) => {
        Arc::new(Mutex::new($x))
    };
}

macro_rules! lock {
    ($x:expr) => {
        $x.lock().unwrap()
    };
}

fn draw_text(text: &str, keyboard: &mut Keyboard) -> Result<()> {
    info!("Drawing text to the screen.");
    keyboard.progress()?;
    keyboard.progress_end()?;
    keyboard.key_cmd_body()?;
    keyboard.string_to_keypresses(text)?;
    keyboard.string_to_keypresses("\n\n")?;
    Ok(())
}

fn draw_svg(
    svg_data: &str,
    keyboard: &mut Keyboard,
    pen: &mut Pen,
    save_bitmap: Option<&String>,
    no_draw: bool,
) -> Result<()> {
    info!("Drawing SVG to the screen.");
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
    debug!("Loading config from {}", filename);

    if std::path::Path::new(filename).exists() {
        std::fs::read_to_string(filename).unwrap()
    } else {
        std::str::from_utf8(Asset::get(filename).unwrap().data.as_ref())
            .unwrap()
            .to_string()
    }
}

fn ghostwriter(args: &Args) -> Result<String> {
    let keyboard = shared!(Keyboard::new(false, false));
    let pen = shared!(Pen::new(false));
    let touch = shared!(Touch::new(false));

    let mut engine_options = OptionMap::new();

    let model = "gpt-3.5-turbo".to_string();
    engine_options.insert("model".to_string(), model.clone());

    let engine_name = "openai".to_string();

    let mut engine: Box<dyn LLMEngine> = Box::new(OpenAI::new(&engine_options));

    let output_file = args.output_file.clone();
    let no_draw = args.no_draw;
    let keyboard_clone = Arc::clone(&keyboard);
    let touch_clone = Arc::clone(&touch);

    let tool_config_draw_text = load_config("tool_draw_text.json");

    engine.register_tool(
        "draw_text",
        serde_json::from_str::<serde_json::Value>(tool_config_draw_text.as_str())?,
        Box::new(move |arguments: json| {
            let text = arguments["text"].as_str().unwrap();
            if let Some(output_file) = &output_file {
                std::fs::write(output_file, text).unwrap();
            }
            if !no_draw {
                // Touch in the middle bottom to make sure we go below any new drawing
                lock!(touch_clone).touch_start((384, 1000)).unwrap(); // middle bottom
                lock!(touch_clone).touch_stop().unwrap();

                let mut keyboard = lock!(keyboard_clone);
                if let Ok(keyboard) = keyboard.as_mut() {
                    draw_text(text, keyboard).unwrap();
                }
            }
        }),
    );

    let output_file = args.output_file.clone();
    let save_bitmap = args.save_bitmap.clone();
    let no_draw = args.no_draw;
    let keyboard_clone = Arc::clone(&keyboard);
    let pen_clone = Arc::clone(&pen);

    let tool_config_draw_svg = load_config("tool_draw_svg.json");
    engine.register_tool(
        "draw_svg",
        serde_json::from_str::<serde_json::Value>(tool_config_draw_svg.as_str())?,
        Box::new(move |arguments: json| {
            let svg_data = arguments["svg"].as_str().unwrap();
            if let Some(output_file) = &output_file {
                std::fs::write(output_file, svg_data).unwrap();
            }
            let mut keyboard = lock!(keyboard_clone);
            let mut pen = lock!(pen_clone);
            if let Ok(keyboard) = keyboard.as_mut() {
                draw_svg(
                    svg_data,
                    keyboard,
                    &mut pen,
                    save_bitmap.as_ref(),
                    no_draw,
                ).unwrap();
            }
        }),
    );

    // 添加初始文本到引擎
    engine.add_text_content(&args.initial_text);

    info!("Executing the engine (call out to {}", engine_name);
    let response = engine.execute()?;
    
    if args.no_loop {
        Ok(response)
    } else {
        Ok(response)
    }
}
