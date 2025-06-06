use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use windows::Win32::UI::Shell::{SHGetFolderPathW, CSIDL_APPDATA};
use windows::Win32::Foundation::{MAX_PATH, HWND};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ThumbnailStrategy {
    #[serde(rename = "ModeA")]
    DefaultTopToBottom,
    #[serde(rename = "ModeB")]
    OnlyLoadVisible,
    #[serde(rename = "ModeC")]
    LoadVisiblePlus500,
}

impl Default for ThumbnailStrategy {
    fn default() -> Self {
        ThumbnailStrategy::OnlyLoadVisible // Default to Mode B
    }
}

impl ThumbnailStrategy {
    pub fn display_name(self) -> &'static str {
        match self {
            ThumbnailStrategy::DefaultTopToBottom => "Default (Top-to-Bottom)",
            ThumbnailStrategy::OnlyLoadVisible => "Only Load Visible Thumbnails",
            ThumbnailStrategy::LoadVisiblePlus500 => "Load Visible + Next 500",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ThumbnailBackground {
    #[serde(rename = "Transparent")]
    Transparent,
    #[serde(rename = "Checkerboard")]
    Checkerboard,
    #[serde(rename = "Black")]
    Black,
    #[serde(rename = "White")]
    White,
    #[serde(rename = "Gray")]
    Gray,
    #[serde(rename = "LightGray")]
    LightGray,
    #[serde(rename = "DarkGray")]
    DarkGray,
}

impl Default for ThumbnailBackground {
    fn default() -> Self {
        ThumbnailBackground::Transparent
    }
}

impl ThumbnailBackground {
    pub fn to_color_ref(self) -> u32 {
        match self {
            ThumbnailBackground::Transparent => 0x00000000, // Special case for transparent
            ThumbnailBackground::Checkerboard => 0x00000000, // Special case for checkerboard
            ThumbnailBackground::Black => 0x00000000,
            ThumbnailBackground::White => 0x00FFFFFF,
            ThumbnailBackground::Gray => 0x00808080,
            ThumbnailBackground::LightGray => 0x00C0C0C0,
            ThumbnailBackground::DarkGray => 0x00404040,
        }
    }
    
    pub fn display_name(self) -> &'static str {
        match self {
            ThumbnailBackground::Transparent => "Transparent",
            ThumbnailBackground::Checkerboard => "Checkerboard",
            ThumbnailBackground::Black => "Black",
            ThumbnailBackground::White => "White", 
            ThumbnailBackground::Gray => "Gray",
            ThumbnailBackground::LightGray => "Light Gray",
            ThumbnailBackground::DarkGray => "Dark Gray",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum LanguageCode {
    English,
    Chinese,
}

impl Default for LanguageCode {
    fn default() -> Self {
        LanguageCode::English
    }
}

impl LanguageCode {
    pub fn to_string(&self) -> String {
        match self {
            LanguageCode::English => "en".to_string(),
            LanguageCode::Chinese => "zh".to_string(),
        }
    }
    
    pub fn from_string(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "zh" | "zh-cn" | "chinese" => LanguageCode::Chinese,
            _ => LanguageCode::English,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AppConfig {
    pub thumbnail_strategy: ThumbnailStrategy,
    pub thumbnail_background: ThumbnailBackground,
    pub language: LanguageCode,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            thumbnail_strategy: ThumbnailStrategy::default(),
            thumbnail_background: ThumbnailBackground::default(),
            language: LanguageCode::default(),
        }
    }
}

pub fn get_config_dir() -> Result<PathBuf, Box<dyn std::error::Error>> {
    unsafe {
        let mut path: [u16; MAX_PATH as usize] = [0; MAX_PATH as usize];
        let result = SHGetFolderPathW(
            HWND(0),
            CSIDL_APPDATA as i32,
            None,
            0,
            &mut path,
        );
        
        if result.is_ok() {
            let len = path.iter().position(|&x| x == 0).unwrap_or(path.len());
            let appdata_path = String::from_utf16(&path[..len])?;
            let mut config_dir = PathBuf::from(appdata_path);
            config_dir.push("EverythingLikeBrowser");
            
            // Create directory if it doesn't exist
            if !config_dir.exists() {
                fs::create_dir_all(&config_dir)?;
            }
            
            Ok(config_dir)
        } else {
            Err("Failed to get AppData folder".into())
        }
    }
}

pub fn get_config_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let mut config_dir = get_config_dir()?;
    config_dir.push("config.json");
    Ok(config_dir)
}

pub fn load_config() -> AppConfig {
    match get_config_path() {
        Ok(config_path) => {
            if config_path.exists() {
                match fs::read_to_string(&config_path) {
                    Ok(content) => {
                        match serde_json::from_str::<AppConfig>(&content) {
                            Ok(config) => {
                                println!("Loaded config: {:?}", config);
                                return config;
                            }
                            Err(e) => {
                                println!("Failed to parse config file: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        println!("Failed to read config file: {}", e);
                    }
                }
            } else {
                println!("Config file not found, using defaults");
            }
        }
        Err(e) => {
            println!("Failed to get config path: {}", e);
        }
    }
    
    AppConfig::default()
}

pub fn save_config(config: &AppConfig) -> Result<(), Box<dyn std::error::Error>> {
    let config_path = get_config_path()?;
    let content = serde_json::to_string_pretty(config)?;
    fs::write(&config_path, content)?;
    println!("Saved config: {:?}", config);
    Ok(())
} 