use anyhow::Result;
use std::sync::{Arc, Mutex};
use std::fs;
use std::path::PathBuf;
use base64::prelude::*;
use crate::pen::Pen;
use crate::screenshot::Screenshot;
use crate::llm_engine::LLMEngine;
use crate::util::OptionMap;
use std::time::Duration;
use std::thread::sleep;
use log;
use tesseract::Tesseract;
use image::{DynamicImage, ImageBuffer, Luma};
use imageproc::contrast::stretch_contrast;
use imageproc::filter::gaussian_blur_f32;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct HandwritingInput {
    pen: Arc<Mutex<Pen>>,
    strokes: Vec<Vec<(i32, i32)>>,
    is_writing: bool,
    temp_dir: PathBuf,
    engine: Box<dyn LLMEngine>,
    cache: HashMap<String, (String, u64)>,  // (图像hash, (识别结果, 时间戳))
}

impl HandwritingInput {
    pub fn new(
        no_draw: bool,
        engine: Box<dyn LLMEngine>,
    ) -> Result<Self> {
        // 创建临时目录
        let temp_dir = std::env::temp_dir().join("ghostwriter");
        fs::create_dir_all(&temp_dir)?;
        
        Ok(Self {
            pen: Arc::new(Mutex::new(Pen::new(no_draw))),
            strokes: Vec::new(),
            is_writing: false,
            temp_dir,
            engine,
            cache: HashMap::new(),
        })
    }

    pub fn start_stroke(&mut self, x: i32, y: i32) -> Result<()> {
        self.is_writing = true;
        let mut current_stroke = Vec::new();
        current_stroke.push((x, y));
        self.strokes.push(current_stroke);
        
        let mut pen = self.pen.lock().unwrap();
        pen.pen_down()?;
        pen.goto_xy((x, y))?;
        Ok(())
    }

    pub fn continue_stroke(&mut self, x: i32, y: i32) -> Result<()> {
        if self.is_writing {
            if let Some(current_stroke) = self.strokes.last_mut() {
                current_stroke.push((x, y));
                let mut pen = self.pen.lock().unwrap();
                pen.goto_xy((x, y))?;
                sleep(Duration::from_millis(1));
            }
        }
        Ok(())
    }

    pub fn end_stroke(&mut self) -> Result<()> {
        self.is_writing = false;
        let mut pen = self.pen.lock().unwrap();
        pen.pen_up()?;
        Ok(())
    }

    pub fn clear(&mut self) {
        self.strokes.clear();
        self.is_writing = false;
    }

    pub fn capture_and_recognize(&mut self) -> Result<String> {
        // 1. 截取当前屏幕
        let screenshot = Screenshot::new()?;
        let img_data = screenshot.get_image_data()?;
        
        // 计算图像hash
        let img_hash = base64::encode(&img_data);
        
        // 检查缓存
        if let Some((result, timestamp)) = self.cache.get(&img_hash) {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)?
                .as_secs();
            
            // 如果缓存时间不超过5分钟,直接返回缓存结果
            if now - timestamp < 300 {
                return Ok(result.clone());
            }
        }
        
        // 2. 图像预处理
        let img = image::load_from_memory(&img_data)?;
        let gray_img = img.to_luma8();
        
        // 应用高斯模糊减少噪点
        let blurred = gaussian_blur_f32(&gray_img, 0.5);
        
        // 增强对比度
        let enhanced = stretch_contrast(&blurred);
        
        // 保存预处理后的图像
        let temp_file = self.temp_dir.join("screenshot.png");
        enhanced.save(&temp_file)?;
        
        // 3. 使用 Tesseract 进行识别
        let mut tess = Tesseract::new(None, Some("chi_sim"))?;
        tess.set_image(&temp_file)?;
        
        // 设置 PSM 模式为 6 (假设是统一的文本块)
        tess.set_variable("tessedit_pageseg_mode", "6")?;
        
        // 优化性能相关的配置
        tess.set_variable("debug_file", "/dev/null")?;  // 禁用调试输出
        tess.set_variable("tessedit_do_invert", "0")?;  // 禁用图像反转
        tess.set_variable("textord_heavy_nr", "0")?;    // 禁用重度降噪
        tess.set_variable("textord_min_linesize", "2.5")?;  // 减少最小行高要求
        tess.set_variable("tessedit_unrej_any_wd", "1")?;   // 允许所有单词
        tess.set_variable("edges_max_children_per_outline", "10")?;  // 限制轮廓处理
        
        // 设置白名单字符（可选,但可以提高速度）
        tess.set_variable("tessedit_char_whitelist", "的一是在不了有和人这中大为上个国我以要他时来用们生到作地于出就分对成会可主发年动同工也能下过子说产种面而方后多定行学法所民得经十三之进着等部度家电力里如水化高自二理起小物现实加量都两体制机当使点从业本去把性好应开它合还因由其些然前外天政四日那社义事平形相全表间样与关各重新线内数正心反你明看原又么利比或但质气第向道命此变条只没结解问意建月公无系军很情者最立代想已通并提直题党程展五果料象员革位入常文总次品式活设及管特件长求老头基资边流路级少图山统接知较将组见计别她手角期根论运农指几九区强放决西被干做必战先回则任取据处队南给色光门即保治北造百规热领七海口东导器压志世金增争济阶油思术极交受联什认六共权收证改清己美再采转更单风切打白教速花带安场身车例真务具万每目至达走积示议声报斗完类八离华名确才科张信马节话米整空元况今集温传土许步群广石记需段研界拉林律叫且究观越织装影算低持音众书布复容儿须际商非验连断深难近矿千周委素技备半办青省列习响约支般史感劳便团往酸历市克何除消构府称太准精值号率族维划选标写存候毛亲快效斯院查江型眼王按格养易置派层片始却专状育厂京识适属圆包火住调满县局照参红细引听该铁价严龙飞")?;
        
        // 进行识别
        let text = tess.get_text()?;
        
        // 更新缓存
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)?
            .as_secs();
        self.cache.insert(img_hash, (text.clone(), now));
        
        // 4. 将识别结果传给 AI 引擎
        self.engine.clear_content();
        self.engine.add_text_content(&format!(
            "识别到的手写文字内容是:\n{}\n请对这段文字进行分析和回应。",
            text.trim()
        ));
        
        // 5. 注册回调处理识别结果
        let response = Arc::new(Mutex::new(String::new()));
        let response_clone = response.clone();
        
        self.engine.register_tool(
            "write",
            serde_json::json!({
                "name": "write",
                "description": "Write the recognized text",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "text": {
                            "type": "string",
                            "description": "The recognized text"
                        }
                    },
                    "required": ["text"]
                }
            }),
            Box::new(move |args: serde_json::Value| {
                let text = args["text"].as_str().unwrap_or_default();
                *response_clone.lock().unwrap() = text.to_string();
            })
        );
        
        // 6. 执行识别
        self.engine.execute()?;
        
        // 7. 返回识别结果
        let result = response.lock().unwrap().clone();
        Ok(result)
    }
} 