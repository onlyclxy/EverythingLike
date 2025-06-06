use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Clone, Debug)]
pub struct LanguageStrings {
    // Menu items
    pub menu_view: String,
    pub menu_columns: String,
    pub menu_thumbnail_options: String,
    pub menu_thumbnail_background: String,
    pub menu_language: String,
    pub menu_file: String,
    
    // View modes
    pub view_details: String,
    pub view_medium_icons: String,
    pub view_large_icons: String,
    pub view_extra_large_icons: String,
    
    // Column names
    pub column_name: String,
    pub column_size: String,
    pub column_type: String,
    pub column_date_modified: String,
    pub column_path: String,
    
    // Thumbnail options
    pub thumb_default: String,
    pub thumb_visible: String,
    pub thumb_visible_plus_500: String,
    
    // Thumbnail backgrounds
    pub bg_transparent: String,
    pub bg_checkerboard: String,
    pub bg_black: String,
    pub bg_white: String,
    pub bg_gray: String,
    pub bg_light_gray: String,
    pub bg_dark_gray: String,
    
    // Context menu
    pub ctx_open: String,
    pub ctx_open_location: String,
    pub ctx_copy_path: String,
    pub ctx_copy_name: String,
    
    // Status bar
    pub status_objects: String,
    pub status_selected: String,
    
    // Time formats
    pub time_today: String,
    pub time_yesterday: String,
    pub time_days_ago: String,
    pub time_weeks_ago: String,
    pub time_months_ago: String,
    
    // Dialog messages
    pub warning_title: String,
    pub warning_thumbnail_mode: String,
    pub warning_continue: String,
    
    // Languages
    pub lang_english: String,
    pub lang_chinese: String,
    
    // File operations
    pub file_open_list: String,
    pub file_save_list: String,
    pub file_export_list: String,
    pub file_close_list: String,
    
    // Sort menu
    pub menu_sort: String,
    pub sort_name: String,
    pub sort_size: String,
    pub sort_type: String,
    pub sort_date: String,
    pub sort_path: String,
    pub sort_ascending: String,
    pub sort_descending: String,
    
    // File filters
    pub file_filter_lists: String,
    pub file_filter_text: String,
    pub file_filter_all: String,
    
    // Confirm dialogs
    pub confirm_close_list: String,
    pub confirm_title: String,
    pub confirm_clear_index: String,
}

impl Default for LanguageStrings {
    fn default() -> Self {
        // Default English strings
        Self {
            // Menu items
            menu_view: "View".to_string(),
            menu_columns: "Columns".to_string(),
            menu_thumbnail_options: "Thumbnail Options".to_string(),
            menu_thumbnail_background: "Thumbnail Background".to_string(),
            menu_language: "Language".to_string(),
            menu_file: "File".to_string(),
            
            // View modes
            view_details: "Details".to_string(),
            view_medium_icons: "Medium Icons".to_string(),
            view_large_icons: "Large Icons".to_string(),
            view_extra_large_icons: "Extra Large Icons".to_string(),
            
            // Column names
            column_name: "Name".to_string(),
            column_size: "Size".to_string(),
            column_type: "Type".to_string(),
            column_date_modified: "Date Modified".to_string(),
            column_path: "Path".to_string(),
            
            // Thumbnail options
            thumb_default: "Default (Top-to-Bottom)".to_string(),
            thumb_visible: "Only Load Visible Thumbnails".to_string(),
            thumb_visible_plus_500: "Load Visible + Next 500".to_string(),
            
            // Thumbnail backgrounds
            bg_transparent: "Transparent".to_string(),
            bg_checkerboard: "Checkerboard".to_string(),
            bg_black: "Black".to_string(),
            bg_white: "White".to_string(),
            bg_gray: "Gray".to_string(),
            bg_light_gray: "Light Gray".to_string(),
            bg_dark_gray: "Dark Gray".to_string(),
            
            // Context menu
            ctx_open: "Open".to_string(),
            ctx_open_location: "Open file location".to_string(),
            ctx_copy_path: "Copy path".to_string(),
            ctx_copy_name: "Copy name".to_string(),
            
            // Status bar
            status_objects: "objects".to_string(),
            status_selected: "Selected".to_string(),
            
            // Time formats
            time_today: "Today".to_string(),
            time_yesterday: "Yesterday".to_string(),
            time_days_ago: "days ago".to_string(),
            time_weeks_ago: "weeks ago".to_string(),
            time_months_ago: "months ago".to_string(),
            
            // Dialog messages
            warning_title: "Warning".to_string(),
            warning_thumbnail_mode: "Loading thumbnails from top to bottom may be very slow and block the UI.\nThis strategy is not recommended.\r\n\r\nDo you want to continue?".to_string(),
            warning_continue: "Continue".to_string(),
            
            // Languages
            lang_english: "English".to_string(),
            lang_chinese: "中文".to_string(),
            
            // File operations
            file_open_list: "Open File List".to_string(),
            file_save_list: "Save File List".to_string(),
            file_export_list: "Export Simple List".to_string(),
            file_close_list: "Close List".to_string(),
            
            // Sort menu
            menu_sort: "Sort".to_string(),
            sort_name: "Sort by Name".to_string(),
            sort_size: "Sort by Size".to_string(),
            sort_type: "Sort by Type".to_string(),
            sort_date: "Sort by Date Modified".to_string(),
            sort_path: "Sort by Path".to_string(),
            sort_ascending: "Ascending".to_string(),
            sort_descending: "Descending".to_string(),
            
            // File filters
            file_filter_lists: "File Lists (*.txt;*.csv;*.efu)".to_string(),
            file_filter_text: "Text".to_string(),
            file_filter_all: "All".to_string(),
            
            // Confirm dialogs
            confirm_close_list: "Are you sure you want to close the current file list?".to_string(),
            confirm_title: "Confirm".to_string(),
            confirm_clear_index: "Are you sure you want to clear the search index? This will remove all indexed file metadata.".to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Language {
    English,
    Chinese,
}

impl Language {
    pub fn from_code(code: &str) -> Self {
        match code {
            "zh" | "zh-CN" | "chinese" => Language::Chinese,
            _ => Language::English,
        }
    }
    
    pub fn to_code(&self) -> &'static str {
        match self {
            Language::English => "en",
            Language::Chinese => "zh",
        }
    }
    
    pub fn display_name(&self) -> &'static str {
        match self {
            Language::English => "English",
            Language::Chinese => "中文",
        }
    }
    
    pub fn file_name(&self) -> &'static str {
        match self {
            Language::English => "en.lang",
            Language::Chinese => "zh.lang",
        }
    }
}

pub struct LanguageManager {
    current_language: Language,
    default_strings: LanguageStrings,
    loaded_strings: HashMap<String, String>,
    lang_dir: String,
}

impl LanguageManager {
    pub fn new(lang_dir: &str) -> Self {
        let manager = Self {
            current_language: Language::English,
            default_strings: LanguageStrings::default(),
            loaded_strings: HashMap::new(),
            lang_dir: lang_dir.to_string(),
        };
        
        // Create language directory if it doesn't exist
        if let Err(e) = fs::create_dir_all(lang_dir) {
            println!("Failed to create language directory: {}", e);
        } else {
            manager.generate_default_files();
        }
        
        manager
    }
    
    pub fn set_language(&mut self, language: Language) -> Result<(), String> {
        // Always update the current language, even if loading fails
        self.current_language = language;
        
        // Try to load the language file
        match self.load_language_file(language) {
            Ok(loaded_strings) => {
                self.loaded_strings = loaded_strings;
                println!("Language switched to: {:?}", language);
                Ok(())
            }
            Err(e) => {
                println!("Failed to load language {:?}: {}. Using default language.", language, e);
                // Clear loaded strings to fall back to defaults
                self.loaded_strings.clear();
                // Return Ok because we can still function with defaults
                Ok(())
            }
        }
    }
    
    pub fn get_current_language(&self) -> Language {
        self.current_language
    }
    
    pub fn get_strings(&self) -> LanguageStrings {
        // Create a new LanguageStrings with translations or fallbacks
        LanguageStrings {
            menu_view: self.get_string("menu_view", &self.default_strings.menu_view),
            menu_columns: self.get_string("menu_columns", &self.default_strings.menu_columns),
            menu_thumbnail_options: self.get_string("menu_thumbnail_options", &self.default_strings.menu_thumbnail_options),
            menu_thumbnail_background: self.get_string("menu_thumbnail_background", &self.default_strings.menu_thumbnail_background),
            menu_language: self.get_string("menu_language", &self.default_strings.menu_language),
            menu_file: self.get_string("menu_file", &self.default_strings.menu_file),
            
            view_details: self.get_string("view_details", &self.default_strings.view_details),
            view_medium_icons: self.get_string("view_medium_icons", &self.default_strings.view_medium_icons),
            view_large_icons: self.get_string("view_large_icons", &self.default_strings.view_large_icons),
            view_extra_large_icons: self.get_string("view_extra_large_icons", &self.default_strings.view_extra_large_icons),
            
            column_name: self.get_string("column_name", &self.default_strings.column_name),
            column_size: self.get_string("column_size", &self.default_strings.column_size),
            column_type: self.get_string("column_type", &self.default_strings.column_type),
            column_date_modified: self.get_string("column_date_modified", &self.default_strings.column_date_modified),
            column_path: self.get_string("column_path", &self.default_strings.column_path),
            
            thumb_default: self.get_string("thumb_default", &self.default_strings.thumb_default),
            thumb_visible: self.get_string("thumb_visible", &self.default_strings.thumb_visible),
            thumb_visible_plus_500: self.get_string("thumb_visible_plus_500", &self.default_strings.thumb_visible_plus_500),
            
            bg_transparent: self.get_string("bg_transparent", &self.default_strings.bg_transparent),
            bg_checkerboard: self.get_string("bg_checkerboard", &self.default_strings.bg_checkerboard),
            bg_black: self.get_string("bg_black", &self.default_strings.bg_black),
            bg_white: self.get_string("bg_white", &self.default_strings.bg_white),
            bg_gray: self.get_string("bg_gray", &self.default_strings.bg_gray),
            bg_light_gray: self.get_string("bg_light_gray", &self.default_strings.bg_light_gray),
            bg_dark_gray: self.get_string("bg_dark_gray", &self.default_strings.bg_dark_gray),
            
            ctx_open: self.get_string("ctx_open", &self.default_strings.ctx_open),
            ctx_open_location: self.get_string("ctx_open_location", &self.default_strings.ctx_open_location),
            ctx_copy_path: self.get_string("ctx_copy_path", &self.default_strings.ctx_copy_path),
            ctx_copy_name: self.get_string("ctx_copy_name", &self.default_strings.ctx_copy_name),
            
            status_objects: self.get_string("status_objects", &self.default_strings.status_objects),
            status_selected: self.get_string("status_selected", &self.default_strings.status_selected),
            
            time_today: self.get_string("time_today", &self.default_strings.time_today),
            time_yesterday: self.get_string("time_yesterday", &self.default_strings.time_yesterday),
            time_days_ago: self.get_string("time_days_ago", &self.default_strings.time_days_ago),
            time_weeks_ago: self.get_string("time_weeks_ago", &self.default_strings.time_weeks_ago),
            time_months_ago: self.get_string("time_months_ago", &self.default_strings.time_months_ago),
            
            warning_title: self.get_string("warning_title", &self.default_strings.warning_title),
            warning_thumbnail_mode: self.get_string("warning_thumbnail_mode", &self.default_strings.warning_thumbnail_mode),
            warning_continue: self.get_string("warning_continue", &self.default_strings.warning_continue),
            
            lang_english: self.get_string("lang_english", &self.default_strings.lang_english),
            lang_chinese: self.get_string("lang_chinese", &self.default_strings.lang_chinese),
            
            file_open_list: self.get_string("file_open_list", &self.default_strings.file_open_list),
            file_save_list: self.get_string("file_save_list", &self.default_strings.file_save_list),
            file_export_list: self.get_string("file_export_list", &self.default_strings.file_export_list),
            file_close_list: self.get_string("file_close_list", &self.default_strings.file_close_list),
            
            menu_sort: self.get_string("menu_sort", &self.default_strings.menu_sort),
            sort_name: self.get_string("sort_name", &self.default_strings.sort_name),
            sort_size: self.get_string("sort_size", &self.default_strings.sort_size),
            sort_type: self.get_string("sort_type", &self.default_strings.sort_type),
            sort_date: self.get_string("sort_date", &self.default_strings.sort_date),
            sort_path: self.get_string("sort_path", &self.default_strings.sort_path),
            sort_ascending: self.get_string("sort_ascending", &self.default_strings.sort_ascending),
            sort_descending: self.get_string("sort_descending", &self.default_strings.sort_descending),
            
            file_filter_lists: self.get_string("file_filter_lists", &self.default_strings.file_filter_lists),
            file_filter_text: self.get_string("file_filter_text", &self.default_strings.file_filter_text),
            file_filter_all: self.get_string("file_filter_all", &self.default_strings.file_filter_all),
            
            confirm_close_list: self.get_string("confirm_close_list", &self.default_strings.confirm_close_list),
            confirm_title: self.get_string("confirm_title", &self.default_strings.confirm_title),
            confirm_clear_index: self.get_string("confirm_clear_index", &self.default_strings.confirm_clear_index),
        }
    }
    
    fn get_string(&self, key: &str, default: &str) -> String {
        self.loaded_strings.get(key).cloned().unwrap_or_else(|| default.to_string())
    }
    
    fn load_language_file(&self, language: Language) -> Result<HashMap<String, String>, String> {
        let file_path = Path::new(&self.lang_dir).join(language.file_name());
        
        if !file_path.exists() {
            return Err(format!("Language file not found: {:?}", file_path));
        }
        
        let content = fs::read_to_string(&file_path)
            .map_err(|e| format!("Failed to read language file: {}", e))?;
        
        let mut strings = HashMap::new();
        
        // Parse simple key=value format
        for (line_num, line) in content.lines().enumerate() {
            let line = line.trim();
            
            // Skip empty lines and comments
            if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
                continue;
            }
            
            // Split on first = sign
            if let Some(eq_pos) = line.find('=') {
                let key = line[..eq_pos].trim().to_string();
                let value = line[eq_pos + 1..].trim();
                
                // Handle quoted strings and escape sequences
                let value = if value.starts_with('"') && value.ends_with('"') && value.len() >= 2 {
                    // Remove quotes and handle escape sequences
                    let unquoted = &value[1..value.len()-1];
                    unquoted.replace("\\n", "\n").replace("\\r", "\r").replace("\\t", "\t").replace("\\\"", "\"")
                } else {
                    value.to_string()
                };
                
                if !key.is_empty() {
                    strings.insert(key, value);
                }
            } else {
                println!("Warning: Invalid line {} in language file {:?}: {}", line_num + 1, file_path, line);
            }
        }
        
        println!("Loaded {} translations from {:?}", strings.len(), file_path);
        Ok(strings)
    }
    
    fn generate_default_files(&self) {
        self.generate_language_file(Language::English, &self.get_english_translations());
        self.generate_language_file(Language::Chinese, &self.get_chinese_translations());
    }
    
    fn generate_language_file(&self, language: Language, translations: &HashMap<String, String>) {
        let file_path = Path::new(&self.lang_dir).join(language.file_name());
        
        if file_path.exists() {
            // Don't overwrite existing files
            return;
        }
        
        let mut content = format!("# {} Language File\n", language.display_name());
        content.push_str("# Format: key=value\n");
        content.push_str("# Use quotes for values with spaces or special characters\n");
        content.push_str("# Use \\n for newlines, \\r for carriage returns\n\n");
        
        // Sort keys for consistent output
        let mut keys: Vec<_> = translations.keys().collect();
        keys.sort();
        
        for key in keys {
            if let Some(value) = translations.get(key) {
                // Quote values that contain special characters
                if value.contains('\n') || value.contains('\r') || value.contains('"') || value.starts_with(' ') || value.ends_with(' ') {
                    let escaped = value.replace('"', "\\\"").replace('\n', "\\n").replace('\r', "\\r");
                    content.push_str(&format!("{}=\"{}\"\n", key, escaped));
                } else {
                    content.push_str(&format!("{}={}\n", key, value));
                }
            }
        }
        
        match fs::write(&file_path, content) {
            Ok(_) => println!("Generated language file: {:?}", file_path),
            Err(e) => println!("Failed to write language file {:?}: {}", file_path, e),
        }
    }
    
    fn get_english_translations(&self) -> HashMap<String, String> {
        let default = LanguageStrings::default();
        let mut map = HashMap::new();
        
        map.insert("menu_view".to_string(), default.menu_view);
        map.insert("menu_columns".to_string(), default.menu_columns);
        map.insert("menu_thumbnail_options".to_string(), default.menu_thumbnail_options);
        map.insert("menu_thumbnail_background".to_string(), default.menu_thumbnail_background);
        map.insert("menu_language".to_string(), default.menu_language);
        map.insert("menu_file".to_string(), default.menu_file);
        
        map.insert("view_details".to_string(), default.view_details);
        map.insert("view_medium_icons".to_string(), default.view_medium_icons);
        map.insert("view_large_icons".to_string(), default.view_large_icons);
        map.insert("view_extra_large_icons".to_string(), default.view_extra_large_icons);
        
        map.insert("column_name".to_string(), default.column_name);
        map.insert("column_size".to_string(), default.column_size);
        map.insert("column_type".to_string(), default.column_type);
        map.insert("column_date_modified".to_string(), default.column_date_modified);
        map.insert("column_path".to_string(), default.column_path);
        
        map.insert("thumb_default".to_string(), default.thumb_default);
        map.insert("thumb_visible".to_string(), default.thumb_visible);
        map.insert("thumb_visible_plus_500".to_string(), default.thumb_visible_plus_500);
        
        map.insert("bg_transparent".to_string(), default.bg_transparent);
        map.insert("bg_checkerboard".to_string(), default.bg_checkerboard);
        map.insert("bg_black".to_string(), default.bg_black);
        map.insert("bg_white".to_string(), default.bg_white);
        map.insert("bg_gray".to_string(), default.bg_gray);
        map.insert("bg_light_gray".to_string(), default.bg_light_gray);
        map.insert("bg_dark_gray".to_string(), default.bg_dark_gray);
        
        map.insert("ctx_open".to_string(), default.ctx_open);
        map.insert("ctx_open_location".to_string(), default.ctx_open_location);
        map.insert("ctx_copy_path".to_string(), default.ctx_copy_path);
        map.insert("ctx_copy_name".to_string(), default.ctx_copy_name);
        
        map.insert("status_objects".to_string(), default.status_objects);
        map.insert("status_selected".to_string(), default.status_selected);
        
        map.insert("time_today".to_string(), default.time_today);
        map.insert("time_yesterday".to_string(), default.time_yesterday);
        map.insert("time_days_ago".to_string(), default.time_days_ago);
        map.insert("time_weeks_ago".to_string(), default.time_weeks_ago);
        map.insert("time_months_ago".to_string(), default.time_months_ago);
        
        map.insert("warning_title".to_string(), default.warning_title);
        map.insert("warning_thumbnail_mode".to_string(), default.warning_thumbnail_mode);
        map.insert("warning_continue".to_string(), default.warning_continue);
        
        map.insert("lang_english".to_string(), default.lang_english);
        map.insert("lang_chinese".to_string(), default.lang_chinese);
        
        map.insert("file_open_list".to_string(), default.file_open_list);
        map.insert("file_save_list".to_string(), default.file_save_list);
        map.insert("file_export_list".to_string(), default.file_export_list);
        map.insert("file_close_list".to_string(), default.file_close_list);
        
        map.insert("menu_sort".to_string(), default.menu_sort);
        map.insert("sort_name".to_string(), default.sort_name);
        map.insert("sort_size".to_string(), default.sort_size);
        map.insert("sort_type".to_string(), default.sort_type);
        map.insert("sort_date".to_string(), default.sort_date);
        map.insert("sort_path".to_string(), default.sort_path);
        map.insert("sort_ascending".to_string(), default.sort_ascending);
        map.insert("sort_descending".to_string(), default.sort_descending);
        
        map.insert("file_filter_lists".to_string(), default.file_filter_lists);
        map.insert("file_filter_text".to_string(), default.file_filter_text);
        map.insert("file_filter_all".to_string(), default.file_filter_all);
        
        map.insert("confirm_close_list".to_string(), default.confirm_close_list);
        map.insert("confirm_title".to_string(), default.confirm_title);
        map.insert("confirm_clear_index".to_string(), default.confirm_clear_index);
        
        map
    }
    
    fn get_chinese_translations(&self) -> HashMap<String, String> {
        let mut map = HashMap::new();
        
        map.insert("menu_view".to_string(), "查看".to_string());
        map.insert("menu_columns".to_string(), "列".to_string());
        map.insert("menu_thumbnail_options".to_string(), "缩略图选项".to_string());
        map.insert("menu_thumbnail_background".to_string(), "缩略图背景".to_string());
        map.insert("menu_language".to_string(), "语言".to_string());
        map.insert("menu_file".to_string(), "文件".to_string());
        
        map.insert("view_details".to_string(), "详细信息".to_string());
        map.insert("view_medium_icons".to_string(), "中等图标".to_string());
        map.insert("view_large_icons".to_string(), "大图标".to_string());
        map.insert("view_extra_large_icons".to_string(), "超大图标".to_string());
        
        map.insert("column_name".to_string(), "名称".to_string());
        map.insert("column_size".to_string(), "大小".to_string());
        map.insert("column_type".to_string(), "类型".to_string());
        map.insert("column_date_modified".to_string(), "修改时间".to_string());
        map.insert("column_path".to_string(), "路径".to_string());
        
        map.insert("thumb_default".to_string(), "默认 (从上到下)".to_string());
        map.insert("thumb_visible".to_string(), "仅加载可见缩略图".to_string());
        map.insert("thumb_visible_plus_500".to_string(), "加载可见 + 后续500个".to_string());
        
        map.insert("bg_transparent".to_string(), "透明".to_string());
        map.insert("bg_checkerboard".to_string(), "棋盘格".to_string());
        map.insert("bg_black".to_string(), "黑色".to_string());
        map.insert("bg_white".to_string(), "白色".to_string());
        map.insert("bg_gray".to_string(), "灰色".to_string());
        map.insert("bg_light_gray".to_string(), "浅灰色".to_string());
        map.insert("bg_dark_gray".to_string(), "深灰色".to_string());
        
        map.insert("ctx_open".to_string(), "打开".to_string());
        map.insert("ctx_open_location".to_string(), "打开文件位置".to_string());
        map.insert("ctx_copy_path".to_string(), "复制路径".to_string());
        map.insert("ctx_copy_name".to_string(), "复制名称".to_string());
        
        map.insert("status_objects".to_string(), "个对象".to_string());
        map.insert("status_selected".to_string(), "已选择".to_string());
        
        map.insert("time_today".to_string(), "今天".to_string());
        map.insert("time_yesterday".to_string(), "昨天".to_string());
        map.insert("time_days_ago".to_string(), "天前".to_string());
        map.insert("time_weeks_ago".to_string(), "周前".to_string());
        map.insert("time_months_ago".to_string(), "个月前".to_string());
        
        map.insert("warning_title".to_string(), "警告".to_string());
        map.insert("warning_thumbnail_mode".to_string(), "从上到下加载缩略图可能非常缓慢并阻塞界面。\\n不推荐使用此策略。\\r\\n\\r\\n您要继续吗？".to_string());
        map.insert("warning_continue".to_string(), "继续".to_string());
        
        map.insert("lang_english".to_string(), "English".to_string());
        map.insert("lang_chinese".to_string(), "中文".to_string());
        
        map.insert("file_open_list".to_string(), "打开文件列表".to_string());
        map.insert("file_save_list".to_string(), "保存文件列表".to_string());
        map.insert("file_export_list".to_string(), "导出简单列表".to_string());
        map.insert("file_close_list".to_string(), "关闭列表".to_string());
        
        map.insert("menu_sort".to_string(), "排序".to_string());
        map.insert("sort_name".to_string(), "按名称排序".to_string());
        map.insert("sort_size".to_string(), "按大小排序".to_string());
        map.insert("sort_type".to_string(), "按类型排序".to_string());
        map.insert("sort_date".to_string(), "按修改时间排序".to_string());
        map.insert("sort_path".to_string(), "按路径排序".to_string());
        map.insert("sort_ascending".to_string(), "升序".to_string());
        map.insert("sort_descending".to_string(), "降序".to_string());
        
        map.insert("file_filter_lists".to_string(), "文件列表 (*.txt;*.csv;*.efu)".to_string());
        map.insert("file_filter_text".to_string(), "文本".to_string());
        map.insert("file_filter_all".to_string(), "全部".to_string());
        
        map.insert("confirm_close_list".to_string(), "确定要关闭当前文件列表吗？".to_string());
        map.insert("confirm_title".to_string(), "确认".to_string());
        map.insert("confirm_clear_index".to_string(), "确定要清除搜索索引吗？这将删除所有已索引的文件元数据。".to_string());
        
        map
    }
}

// Global language manager
static mut LANGUAGE_MANAGER: Option<LanguageManager> = None;

pub fn init_language_manager() {
    unsafe {
        LANGUAGE_MANAGER = Some(LanguageManager::new("languages"));
    }
}

pub fn get_language_manager() -> Option<&'static mut LanguageManager> {
    unsafe {
        LANGUAGE_MANAGER.as_mut()
    }
}

pub fn get_strings() -> LanguageStrings {
    unsafe {
        match &LANGUAGE_MANAGER {
            Some(manager) => manager.get_strings(),
            None => LanguageStrings::default(),
        }
    }
}

pub fn set_language(language: Language) -> Result<(), String> {
    unsafe {
        match &mut LANGUAGE_MANAGER {
            Some(manager) => manager.set_language(language),
            None => Err("Language manager not initialized".to_string()),
        }
    }
}

pub fn get_current_language() -> Language {
    unsafe {
        match &LANGUAGE_MANAGER {
            Some(manager) => manager.get_current_language(),
            None => Language::English,
        }
    }
} 