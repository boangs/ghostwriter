use anyhow::Result;
use dotenv::dotenv;
use env_logger;
use log::{debug, info, error};
use rust_embed::RustEmbed;
use serde_json::{json, Value as JsonValue};
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::Duration;
use std::collections::HashMap;
use std::clone::Clone;
use clap::Parser;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use ghostwriter::constants::{REMARKABLE_WIDTH, REMARKABLE_HEIGHT};
use ghostwriter::handwriting::HandwritingInput;
use ghostwriter::touch::Touch;
use ghostwriter::{
    keyboard::Keyboard,
    llm_engine::{openai::OpenAI, LLMEngine},
    pen::Pen,
    util::{svg_to_bitmap, write_bitmap_to_file, OptionMap},
};

#[derive(RustEmbed)]
#[folder = "prompts/"]
struct Asset;

#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
pub struct Args {
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

    /// Set the log level. Try 'debug' or 'trace'
    #[arg(long, default_value = "info")]
    log_level: String,

    /// 使用手写输入模式
    #[arg(long)]
    handwriting_mode: bool,

    /// 显示坐标刻度
    #[arg(long)]
    show_coordinates: bool,

    /// Last content y coordinate
    #[arg(long)]
    last_content_y: Option<i32>,

    /// 测试橡皮擦检测
    #[arg(long)]
    test_eraser: bool,
}

fn main() -> Result<()> {
    dotenv().ok();
    let args = Args::parse();

    // 设置日志级别为 info 以确保能看到坐标信息
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp(None)  // 不显示时间戳
        .init();

    if args.test_eraser {
        info!("进入橡皮擦检测测试模式");
        info!("请用笔的橡皮擦端靠近或触碰屏幕");
        let mut pen = Pen::new(false);
        loop {
            if pen.check_real_eraser()? {
                println!("检测到橡皮擦！");
            }
            sleep(Duration::from_millis(100));
        }
    }

    if args.handwriting_mode {
        // 手写输入模式
        let mut options = HashMap::new();
        if let Some(engine) = &args.engine {
            options.insert("engine".to_string(), engine.clone());
        }
        if let Some(base_url) = &args.engine_base_url {
            options.insert("base_url".to_string(), base_url.clone());
        }
        if let Some(api_key) = &args.engine_api_key {
            options.insert("api_key".to_string(), api_key.clone());
        }
        options.insert("model".to_string(), args.model.clone());
        
        let engine = Box::new(OpenAI::new(&options));
        let mut handwriting = HandwritingInput::new(args.no_draw, engine)?;
        let mut touch = Touch::new(args.no_draw);
        
        info!("进入手写输入模式");
        info!("请在屏幕上书写内容");
        info!("触摸右下角区域并松开手指来触发识别");
        info!("触发区域：距离右边缘约 60 像素，距离底部约 60 像素的区域");
        
        // 等待用户在右下角触发
        loop {
            // 等待触摸事件
            if let Ok(()) = touch.wait_for_trigger() {
                info!("检测到触发手势，开始识别...");
                // 触发识别
                match handwriting.capture_and_recognize() {
                    Ok((prompt, last_y)) => {
                        info!("识别到的提示词: {}", prompt);
                        // 使用识别到的文本作为提示词，并传递最后一行的 y 坐标
                        let mut args = args.clone();
                        args.last_content_y = Some(last_y);
                        process_with_prompt(&args, &prompt)?;
                    }
                    Err(e) => {
                        error!("识别失败: {}", e);
                    }
                }
                handwriting.clear();
            }
            sleep(Duration::from_millis(10));
        }
    } else {
        // 原有的提示词文件模式
        process_with_prompt(&args, &args.prompt)?;
    }
    
    Ok(())
}

fn process_with_prompt(args: &Args, prompt: &str) -> Result<()> {
    // 创建 AI 引擎实例
    let mut options = HashMap::new();
    if let Some(engine) = &args.engine {
        options.insert("engine".to_string(), engine.clone());
    }
    if let Some(base_url) = &args.engine_base_url {
        options.insert("base_url".to_string(), base_url.clone());
    }
    if let Some(api_key) = &args.engine_api_key {
        options.insert("api_key".to_string(), api_key.clone());
    }
    options.insert("model".to_string(), args.model.clone());
    
    let mut engine = OpenAI::new(&options);
    
    // 添加提示内容
    engine.add_text_content(prompt);
    
    // 如果有输入图片，添加图片内容
    if let Some(png_file) = &args.input_png {
        let image_data = std::fs::read(png_file)?;
        let base64_image = STANDARD.encode(&image_data);
        engine.add_image_content(&base64_image);
    }
    
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
        Box::new(move |args: JsonValue| {
            let text = args["text"].as_str().unwrap_or_default();
            *response_clone.lock().unwrap() = text.to_string();
        })
    );

    // 执行并获取回复
    if !args.no_submit {
        if let Err(e) = engine.execute() {
            error!("获取 AI 回复失败: {}", e);
            return Err(anyhow::anyhow!("AI 回复失败: {}", e));
        }
    }

    let response_text = response.lock().unwrap().clone();
    info!("收到 AI 回复: {}", response_text);

    // 如果需要保存模型输出
    if let Some(output_file) = &args.model_output_file {
        std::fs::write(output_file, &response_text)?;
    }

    // 创建键盘实例，使用最后一行的 y 坐标加上一些间距
    let last_y = if let Some(y) = args.last_content_y {
        y as u32 + 10  // 添加 10 像素的间距
    } else {
        100  // 默认值
    };

    // 如果是手写模式，使用 HandwritingInput 的 write_text 方法
    if args.handwriting_mode {
        let mut options = HashMap::new();
        if let Some(engine) = &args.engine {
            options.insert("engine".to_string(), engine.clone());
        }
        if let Some(base_url) = &args.engine_base_url {
            options.insert("base_url".to_string(), base_url.clone());
        }
        if let Some(api_key) = &args.engine_api_key {
            options.insert("api_key".to_string(), api_key.clone());
        }
        options.insert("model".to_string(), args.model.clone());
        
        let engine = Box::new(OpenAI::new(&options));
        let mut handwriting = HandwritingInput::new(args.no_draw, engine)?;
        
        // 绘制 AI 回复的文字
        if !args.no_draw {
            info!("开始绘制 AI 回复");
            handwriting.write_text(&response_text, 150, REMARKABLE_HEIGHT as i32 - 200)?;
        }
    } else {
        let keyboard = Keyboard::new(args.no_draw, args.no_draw_progress, Some(last_y))?;

        // 如果需要显示坐标刻度
        if args.show_coordinates {
            info!("显示坐标刻度");
            keyboard.write_coordinates()?;
        }

        // 绘制 AI 回复的文字
        if !args.no_draw {
            info!("开始绘制 AI 回复");
            keyboard.write_text(&response_text)?;
        }
    }

    // 创建触摸实例
    let mut touch = Touch::new(args.no_draw);

    // 在手写模式下，等待用户触摸右下角继续
    if args.handwriting_mode && !args.no_trigger {
        info!("绘制完成，请在右下角触摸以继续...");
        touch.wait_for_trigger()?;
    }
    
    Ok(())
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

#[allow(dead_code)]
fn draw_text(text: &str, keyboard: &mut Keyboard) -> Result<()> {
    info!("Drawing text to the screen.");
    keyboard.write_text(text)?;  // 直接使用 write_text，因为它已经包含了所有必要的功能
    Ok(())
}

#[allow(dead_code)]
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

#[allow(dead_code)]
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

#[allow(dead_code)]
fn ghostwriter(args: &Args) -> Result<String> {
    let keyboard = shared!(Keyboard::new(false, false, None));
    let pen = shared!(Pen::new(false));
    let touch = shared!(Touch::new(false));
    let touch_clone: Arc<Mutex<Touch>> = Arc::clone(&touch);

    let mut engine_options = OptionMap::new();

    let model = "gpt-3.5-turbo".to_string();
    engine_options.insert("model".to_string(), model.clone());

    let engine_name = "openai".to_string();

    let mut engine: Box<dyn LLMEngine> = Box::new(OpenAI::new(&engine_options));

    let output_file = args.output_file.clone();
    let no_draw = args.no_draw;
    let keyboard_clone = Arc::clone(&keyboard);
    let touch_clone = Arc::clone(&touch_clone);

    let tool_config_draw_text = load_config("tool_draw_text.json");

    engine.register_tool(
        "draw_text",
        serde_json::from_str::<JsonValue>(tool_config_draw_text.as_str())?,
        Box::new(move |arguments: JsonValue| {
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
        serde_json::from_str::<JsonValue>(tool_config_draw_svg.as_str())?,
        Box::new(move |arguments: JsonValue| {
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
    engine.add_text_content(&args.prompt);

    info!("Executing the engine (call out to {}", engine_name);
    engine.execute()?;
    
    let response_text = String::new(); // 这里需要获取实际的响应文本
    if args.no_loop {
        Ok(response_text)
    } else {
        Ok(response_text)
    }
}

#[allow(dead_code)]
fn process_response(response: Result<(), anyhow::Error>) -> Result<()> {
    match response {
        Ok(_) => Ok(()),
        Err(e) => Err(e),
    }
}
