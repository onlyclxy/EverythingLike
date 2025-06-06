use windows::{
    core::*,
    Win32::{
        Foundation::*,
        Graphics::Gdi::*,
        System::LibraryLoader::GetModuleHandleW,
        UI::{
            Controls::*,
            Input::KeyboardAndMouse::*,
            WindowsAndMessaging::*,
            Shell::ShellExecuteW,
        },
    },
};

mod everything_sdk;
mod thumbnail;
mod config;
mod lang;
mod file_icons;

use everything_sdk::{EverythingSDK, FileResult};
use thumbnail::{ThumbnailTaskManager, WM_THUMBNAIL_READY, WM_RECOMPUTE_THUMBS, create_placeholder_bitmap, to_wide};
use config::{ThumbnailStrategy, ThumbnailBackground, LanguageCode, AppConfig, load_config, save_config};
use lang::{Language, init_language_manager, set_language, get_strings, get_current_language};
use file_icons::{init_icon_cache, get_file_icon, get_default_file_icon, draw_icon};
use lru::LruCache;
use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::num::NonZeroUsize;
use std::sync::{Arc, atomic::{AtomicBool, AtomicU64, Ordering}, Mutex, mpsc};
use std::time::{Duration, Instant};
use rayon::prelude::*;

// Global logger for debugging
static mut LOG_FILE: Option<std::fs::File> = None;

// Global Everything SDK synchronization
static EVERYTHING_SDK_MUTEX: Mutex<()> = Mutex::new(());

// Store original search edit window procedure
static mut ORIGINAL_SEARCH_EDIT_PROC: Option<WNDPROC> = None;

// Search request structure
#[derive(Debug)]
struct SearchRequest {
    query: String,
    generation: u64,
    window: HWND,
    cancel_flag: Arc<AtomicBool>,
}

fn init_logger() {
    unsafe {
        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open("debug.log") {
            let _ = writeln!(file, "=== Application Debug Log Started ===");
            LOG_FILE = Some(file);
        }
    }
}

fn log_debug(message: &str) {
    unsafe {
        if let Some(ref mut file) = LOG_FILE {
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis();
            let _ = writeln!(file, "[{}] {}", timestamp, message);
            let _ = file.flush();
        }
    }
    println!("{}", message); // Also print to console
}

// Helper macros for Win32
macro_rules! LOWORD {
    ($l:expr) => {
        ($l & 0xFFFF) as u16
    };
}

macro_rules! HIWORD {
    ($l:expr) => {
        (($l >> 16) & 0xFFFF) as u16
    };
}

fn GET_WHEEL_DELTA_WPARAM(wparam: WPARAM) -> i16 {
    HIWORD!(wparam.0) as i16
}

// Custom window messages
const WM_SEARCH_RESULTS: u32 = WM_USER + 100;
const WM_SEARCH_DEBOUNCE: u32 = WM_USER + 101;

// Timer IDs
const SEARCH_TIMER_ID: usize = 1001;

// Window class names
const MAIN_WINDOW_CLASS: &str = "EverythingLikeMainWindow";
const LIST_VIEW_CLASS: &str = "EverythingLikeListView";

// Control IDs
const ID_SEARCH_EDIT: i32 = 1001;
const ID_LIST_VIEW: i32 = 1002;
const ID_STATUS_BAR: i32 = 1003;

// Header height for details view
const HEADER_HEIGHT: i32 = 25;

// Menu IDs for view modes
const ID_VIEW_DETAILS: i32 = 2001;
const ID_VIEW_MEDIUM_ICONS: i32 = 2002;
const ID_VIEW_LARGE_ICONS: i32 = 2003;
const ID_VIEW_EXTRALARGE_ICONS: i32 = 2004;

// Menu IDs for thumbnail strategies
const ID_THUMB_DEFAULT: i32 = 3001;
const ID_THUMB_VISIBLE: i32 = 3002;
const ID_THUMB_VISIBLE_PLUS_500: i32 = 3003;

// Menu IDs for thumbnail backgrounds
const ID_BG_TRANSPARENT: i32 = 3101;
const ID_BG_CHECKERBOARD: i32 = 3102;
const ID_BG_BLACK: i32 = 3103;
const ID_BG_WHITE: i32 = 3104;
const ID_BG_GRAY: i32 = 3105;
const ID_BG_LIGHT_GRAY: i32 = 3106;
const ID_BG_DARK_GRAY: i32 = 3107;

// Menu IDs for file context menu
const ID_OPEN_FILE: i32 = 4001;
const ID_OPEN_FILE_LOCATION: i32 = 4002;
const ID_COPY_PATH: i32 = 4003;
const ID_COPY_NAME: i32 = 4004;

// Menu IDs for column management
const ID_COLUMN_NAME: i32 = 5001;
const ID_COLUMN_SIZE: i32 = 5002;
const ID_COLUMN_TYPE: i32 = 5003;
const ID_COLUMN_MODIFIED: i32 = 5004;
const ID_COLUMN_PATH: i32 = 5005;

// Menu IDs for language management
const ID_LANG_ENGLISH: i32 = 6001;
const ID_LANG_CHINESE: i32 = 6002;

// Menu IDs for file operations
const ID_FILE_OPEN_LIST: i32 = 7001;
const ID_FILE_SAVE_LIST: i32 = 7002;
const ID_FILE_EXPORT_LIST: i32 = 7003;
const ID_FILE_CLOSE_LIST: i32 = 7004;

// Menu IDs for sort operations
const ID_SORT_NAME: i32 = 8001;
const ID_SORT_SIZE: i32 = 8002;
const ID_SORT_TYPE: i32 = 8003;
const ID_SORT_DATE: i32 = 8004;
const ID_SORT_PATH: i32 = 8005;
const ID_SORT_ASCENDING: i32 = 8006;
const ID_SORT_DESCENDING: i32 = 8007;

#[derive(Clone, PartialEq, Debug)]
enum ViewMode {
    Details,
    MediumIcons,
    LargeIcons,
    ExtraLargeIcons,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum ColumnType {
    Name,
    Size,
    Type,
    Modified,
    Path,
}

impl ColumnType {
    fn display_name(&self) -> &'static str {
        match self {
            ColumnType::Name => "Name",
            ColumnType::Size => "Size",
            ColumnType::Type => "Type",
            ColumnType::Modified => "Date Modified",
            ColumnType::Path => "Path",
        }
    }
    
    fn default_width(&self) -> i32 {
        match self {
            ColumnType::Name => 200,
            ColumnType::Size => 80,
            ColumnType::Type => 100,
            ColumnType::Modified => 120,
            ColumnType::Path => 300,
        }
    }
}

#[derive(Debug, Clone)]
struct ColumnInfo {
    column_type: ColumnType,
    width: i32,
    visible: bool,
}

impl ColumnInfo {
    fn new(column_type: ColumnType) -> Self {
        Self {
            column_type,
            width: column_type.default_width(),
            visible: true,
        }
    }
}

#[derive(Debug)]
struct ColumnDragState {
    is_dragging: bool,
    column_index: usize,
    start_x: i32,
    start_width: i32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum SortOrder {
    None,
    Ascending,
    Descending,
}

#[derive(Debug, Clone)]
struct SortState {
    column: ColumnType,
    order: SortOrder,
}

// Application state
struct AppState {
    main_window: HWND,
    search_edit: HWND,
    list_view: HWND,
    status_bar: HWND,
    list_data: Vec<FileResult>,
    visible_start: usize,
    visible_count: usize,
    item_height: i32,
    scroll_pos: i32,
    total_height: i32,
    client_height: i32,
    client_width: i32,
    font: HFONT,
    everything_sdk: Option<EverythingSDK>,
    selected_index: Option<usize>,
    view_mode: ViewMode,
    selected_view_size: u32,
    zoom_level: i32, // 0-14: 0=Details, 1-14=Icon sizes
    thumbnail_cache: LruCache<(String, u32), HBITMAP>,
    thumbnail_task_manager: Option<ThumbnailTaskManager>,
    grid_cols: i32,
    cell_size: i32,
    config: AppConfig,
    // Async search state
    search_cancel_flag: Arc<AtomicBool>,
    search_generation: Arc<AtomicU64>,
    last_search_time: Instant,
    pending_search_query: String,
    // Search channel for thread-safe Everything SDK access
    search_sender: Option<mpsc::Sender<SearchRequest>>,
    // Search debounce timer
    search_timer_active: bool,
    // Scrollbar dragging state
    is_scrollbar_dragging: bool,
    // Column configuration
    columns: Vec<ColumnInfo>,
    column_drag_state: Option<ColumnDragState>,
    // Sorting state
    sort_state: Option<SortState>,
    // File list mode state
    is_list_mode: bool,
    current_list_name: Option<String>,
    original_list_data: Vec<FileResult>,
}

static mut APP_STATE: Option<AppState> = None;

impl AppState {
    fn new() -> Self {
        let config = load_config();
        
        // Initialize language manager
        init_language_manager();
        
        // Set language from config
        let language = match config.language {
            LanguageCode::English => Language::English,
            LanguageCode::Chinese => Language::Chinese,
        };
        if let Err(e) = set_language(language) {
            println!("Failed to set language: {}", e);
        }
        
        // Initialize icon cache
        init_icon_cache();
        
        // Initialize default columns
        let mut columns = Vec::new();
        columns.push(ColumnInfo::new(ColumnType::Name));
        columns.push(ColumnInfo::new(ColumnType::Size));
        columns.push(ColumnInfo::new(ColumnType::Type));
        columns.push(ColumnInfo::new(ColumnType::Modified));
        columns.push(ColumnInfo::new(ColumnType::Path));
        
        // Hide some columns by default
        columns[2].visible = false; // Type
        columns[3].visible = false; // Modified
        
        Self {
            main_window: HWND(0),
            search_edit: HWND(0),
            list_view: HWND(0),
            status_bar: HWND(0),
            list_data: Vec::new(),
            visible_start: 0,
            visible_count: 0,
            item_height: 20,
            scroll_pos: 0,
            total_height: 0,
            client_height: 0,
            client_width: 0,
            font: HFONT(0),
            everything_sdk: None,
            selected_index: None,
            view_mode: ViewMode::Details,
            selected_view_size: 0,
            zoom_level: 0, // Start at Details view
            thumbnail_cache: LruCache::new(NonZeroUsize::new(500).unwrap()),
            thumbnail_task_manager: None,
            grid_cols: 1,
            cell_size: 20,
            config,
            // Async search state
            search_cancel_flag: Arc::new(AtomicBool::new(false)),
            search_generation: Arc::new(AtomicU64::new(0)),
            last_search_time: Instant::now(),
            pending_search_query: String::new(),
            // Search channel for thread-safe Everything SDK access
            search_sender: None,
            // Search debounce timer
            search_timer_active: false,
            // Scrollbar dragging state
            is_scrollbar_dragging: false,
            // Column configuration
            columns,
            column_drag_state: None,
            // Sorting state
            sort_state: None,
            // File list mode state
            is_list_mode: false,
            current_list_name: None,
            original_list_data: Vec::new(),
        }
    }

    // Convert zoom level (0-14) to icon size in pixels
    fn get_icon_size_from_zoom_level(zoom_level: i32) -> u32 {
        match zoom_level {
            0 => 0,   // Details view (no icons)
            1 => 32,  // Smallest icons
            2 => 40,
            3 => 48,
            4 => 56,
            5 => 64,  // Medium icons (old default)
            6 => 72,
            7 => 80,
            8 => 96,
            9 => 112,
            10 => 128, // Large icons (old default)
            11 => 160,
            12 => 192,
            13 => 256, // Extra large icons (old default)
            14 => 320, // Largest icons
            _ => 64,   // Fallback
        }
    }

    // Get view mode based on zoom level
    fn get_view_mode_from_zoom_level(zoom_level: i32) -> ViewMode {
        if zoom_level == 0 {
            ViewMode::Details
        } else {
            // All icon levels use the same mode, size determined by zoom_level
            ViewMode::MediumIcons // We'll use this as our "icon mode"
        }
    }

    fn initialize_everything_sdk(&mut self) {
        match EverythingSDK::new() {
            Ok(sdk) => {
                log_debug("Everything SDK loaded successfully");
                
                // Create a channel for search requests
                let (sender, receiver) = mpsc::channel::<SearchRequest>();
                self.search_sender = Some(sender);
                
                // Start a dedicated search thread with the SDK
                log_debug("Starting dedicated Everything SDK search thread");
                std::thread::spawn(move || {
                    log_debug("Everything SDK search thread started");
                    
                    while let Ok(request) = receiver.recv() {
                        log_debug(&format!("Processing search request: {:?}", request.query));
                        
                        // Check if cancelled before starting
                        if request.cancel_flag.load(Ordering::Relaxed) {
                            log_debug("Search request was cancelled before processing");
                            continue;
                        }
                        
                        // Add debounce delay
                        std::thread::sleep(Duration::from_millis(150));
                        
                        // Check if cancelled after delay
                        if request.cancel_flag.load(Ordering::Relaxed) {
                            log_debug("Search request was cancelled during debounce delay");
                            continue;
                        }
                        
                        log_debug("Performing Everything SDK search");
                        
                        // Perform the search with mutex protection
                        let search_result = {
                            let _guard = EVERYTHING_SDK_MUTEX.lock().unwrap();
                            if request.query.trim().is_empty() {
                                sdk.search_files("*.png")
                            } else {
                                sdk.search_files(&request.query)
                            }
                        };
                        
                        // Check if cancelled after search
                        if request.cancel_flag.load(Ordering::Relaxed) {
                            log_debug("Search request was cancelled after SDK search");
                            continue;
                        }
                        
                        log_debug("Everything SDK search completed, sending results");
                        
                        // Send results back to UI thread
                        match search_result {
                            Ok(file_paths) => {
                                log_debug(&format!("Converting {} file paths to FileResult objects", file_paths.len()));
                                
                                let results: Vec<crate::everything_sdk::FileResult> = file_paths
                                    .into_iter()
                                    .map(|path| crate::everything_sdk::FileResult::from_path(&path))
                                    .collect();
                                
                                // Allocate results in a Box and send the pointer
                                let boxed_results = Box::new((results, request.generation));
                                let results_ptr = Box::into_raw(boxed_results) as isize;
                                
                                log_debug(&format!("Posting WM_SEARCH_RESULTS message with ptr: {}", results_ptr));
                                
                                unsafe {
                                    let _ = PostMessageW(request.window, WM_SEARCH_RESULTS, WPARAM(results_ptr as usize), LPARAM(0));
                                }
                            }
                            Err(e) => {
                                log_debug(&format!("Everything SDK search failed: {}", e));
                                // Send empty results on error
                                let boxed_results = Box::new((Vec::<crate::everything_sdk::FileResult>::new(), request.generation));
                                let results_ptr = Box::into_raw(boxed_results) as isize;
                                
                                unsafe {
                                    let _ = PostMessageW(request.window, WM_SEARCH_RESULTS, WPARAM(results_ptr as usize), LPARAM(0));
                                }
                            }
                        }
                        
                        log_debug("Search request processing completed");
                    }
                    
                    log_debug("Everything SDK search thread terminated");
                });
                
                // Start initial async search with PNG files
                self.start_async_search("*.png".to_string());
            }
            Err(e) => {
                log_debug(&format!("Failed to load Everything SDK: {}", e));
                log_debug("Falling back to sample data");
                self.everything_sdk = None;
                
                // For sample data, use the old rayon approach since it's thread-safe
                self.start_async_search(String::new());
            }
        }
    }

    fn initialize_thumbnail_task_manager(&mut self, window: HWND) {
        self.thumbnail_task_manager = Some(ThumbnailTaskManager::new(window));
    }

    fn load_from_everything_sdk(&mut self, query: &str) -> std::result::Result<(), String> {
        if let Some(ref sdk) = self.everything_sdk {
            println!("Searching for: {}", query);
            
            // Search for files
            match sdk.search_files(query) {
                Ok(file_paths) => {
                    println!("Found {} results", file_paths.len());
                    
                    // Convert paths to FileResult objects
                    self.list_data = file_paths
                        .into_iter()
                        .map(|path| FileResult::from_path(&path))
                        .collect();
                    
                    // Limit results to prevent UI slowdown during testing
                    if self.list_data.len() > 50000 {
                        self.list_data.truncate(50000);
                        println!("Truncated results to 50000 items for performance");
                    }
                    
                    // Reset selection when new data loads
                    self.selected_index = if !self.list_data.is_empty() { Some(0) } else { None };
                    
                    // Clear thumbnail cache when loading new data
                    self.thumbnail_cache.clear();
                    
                    self.calculate_layout();
                    Ok(())
                }
                Err(e) => Err(format!("Search failed: {}", e))
            }
        } else {
            Err("Everything SDK not initialized".to_string())
        }
    }

    fn calculate_layout(&mut self) {
        log_debug(&format!("calculate_layout called, current scroll_pos: {}", self.scroll_pos));
        
        match self.view_mode {
            ViewMode::Details => {
                self.item_height = 20;
                self.grid_cols = 1;
                self.cell_size = self.item_height;
                
                // Account for header height in details view
                let available_height = self.client_height - HEADER_HEIGHT;
                self.visible_start = (self.scroll_pos / self.item_height) as usize;
                self.visible_count = ((available_height / self.item_height) + 2) as usize;
                self.total_height = self.list_data.len() as i32 * self.item_height;
            }
            _ => {
                // Icon modes - add extra height for file name display
                let padding = 8;
                let filename_height = 40; // Reserve space for 2 lines of filename text
                self.cell_size = self.selected_view_size as i32 + padding * 2 + filename_height;
                self.grid_cols = if self.client_width > 0 && self.cell_size > 0 {
                    (self.client_width / self.cell_size).max(1)
                } else {
                    1
                };
                
                let total_rows = if self.grid_cols > 0 {
                    (self.list_data.len() as i32 + self.grid_cols - 1) / self.grid_cols
                } else {
                    0
                };
                
                self.total_height = total_rows * self.cell_size;
                
                // Calculate visible range for grid
                let first_visible_row = self.scroll_pos / self.cell_size;
                let visible_rows = (self.client_height / self.cell_size) + 2;
                
                self.visible_start = (first_visible_row * self.grid_cols) as usize;
                self.visible_count = (visible_rows * self.grid_cols) as usize;
            }
        }
        
        // IMPORTANT: Clamp scroll_pos to valid range after layout changes
        // This prevents losing view position when window size changes dramatically
        let max_scroll = (self.total_height - self.client_height).max(0);
        if self.scroll_pos > max_scroll {
            log_debug(&format!("Clamping scroll_pos from {} to {} (max_scroll) due to layout change", 
                self.scroll_pos, max_scroll));
            self.scroll_pos = max_scroll;
            
            // Recalculate visible range with corrected scroll_pos
            match self.view_mode {
                ViewMode::Details => {
                    let available_height = self.client_height - HEADER_HEIGHT;
                    self.visible_start = (self.scroll_pos / self.item_height) as usize;
                    self.visible_count = ((available_height / self.item_height) + 2) as usize;
                }
                _ => {
                    let first_visible_row = self.scroll_pos / self.cell_size;
                    let visible_rows = (self.client_height / self.cell_size) + 2;
                    
                    self.visible_start = (first_visible_row * self.grid_cols) as usize;
                    self.visible_count = (visible_rows * self.grid_cols) as usize;
                }
            }
        }
        
        // Bounds checking
        if self.visible_start >= self.list_data.len() {
            self.visible_start = if self.list_data.len() > 0 { self.list_data.len() - 1 } else { 0 };
        }
        
        if self.visible_start + self.visible_count > self.list_data.len() {
            self.visible_count = self.list_data.len().saturating_sub(self.visible_start);
        }
        
        log_debug(&format!("calculate_layout completed, scroll_pos: {}, total_height: {}, visible_start: {}, visible_count: {}", 
            self.scroll_pos, self.total_height, self.visible_start, self.visible_count));
    }

    fn populate_sample_data(&mut self) {
        self.list_data.clear();
        for i in 0..100000 {
            let path = format!("C:\\Users\\Example\\Documents\\File_{:06}.txt", i);
            self.list_data.push(FileResult::from_path(&path));
        }
        self.selected_index = if !self.list_data.is_empty() { Some(0) } else { None };
        self.calculate_layout();
    }

    fn search_everything(&mut self, query: &str) {
        if query.trim().is_empty() {
            // If empty query, reload default search
            if let Err(e) = self.load_from_everything_sdk("*.png") {
                println!("Search failed: {}", e);
            }
        } else {
            // Search with user query
            if let Err(e) = self.load_from_everything_sdk(query) {
                println!("Search failed: {}", e);
            }
        }
        
        // Only reset scroll position if we're not currently dragging the scrollbar
        // This prevents the scrollbar from jumping back to the top during scroll operations
        if !self.is_scrollbar_dragging {
        self.scroll_pos = 0;
        }
        self.calculate_layout();
        
        // Cancel all thumbnail tasks and recompute
        if let Some(ref task_manager) = self.thumbnail_task_manager {
            task_manager.cancel_all_tasks();
        }
        
        // Clear thumbnail cache
        self.thumbnail_cache.clear();
        
        // Post message to recompute thumbnails
        unsafe {
            let _ = PostMessageW(self.main_window, WM_RECOMPUTE_THUMBS, WPARAM(0), LPARAM(0));
        }
        
        // Update UI
        unsafe {
            if let Some(state) = &APP_STATE {
                update_scrollbar(state.list_view);
                InvalidateRect(state.list_view, None, TRUE);
                update_status_bar();
            }
        }
    }

    fn set_selection(&mut self, index: usize) {
        if index < self.list_data.len() {
            self.selected_index = Some(index);
            self.ensure_selection_visible();
        }
    }

    fn move_selection(&mut self, direction: i32) {
        if self.list_data.is_empty() {
            return;
        }

        let new_index = match self.selected_index {
            Some(current) => {
                match self.view_mode {
                    ViewMode::Details => {
                        let new = current as i32 + direction;
                        if new < 0 {
                            0
                        } else if new >= self.list_data.len() as i32 {
                            self.list_data.len() - 1
                        } else {
                            new as usize
                        }
                    }
                    _ => {
                        // Grid navigation
                        if direction == -1 && current > 0 {
                            current - 1
                        } else if direction == 1 && current < self.list_data.len() - 1 {
                            current + 1
                        } else if direction < 0 {
                            // Page up or similar - move by grid_cols
                            ((current as i32 + direction * self.grid_cols).max(0)) as usize
                        } else {
                            // Page down or similar
                            ((current as i32 + direction * self.grid_cols) as usize).min(self.list_data.len() - 1)
                        }
                    }
                }
            }
            None => 0,
        };

        self.selected_index = Some(new_index);
        self.ensure_selection_visible();
    }

    fn ensure_selection_visible(&mut self) {
        log_debug(&format!("ensure_selection_visible called, current scroll_pos: {}, selected_index: {:?}", 
            self.scroll_pos, self.selected_index));
            
        if let Some(selected) = self.selected_index {
            match self.view_mode {
                ViewMode::Details => {
                    let selected_y = selected as i32 * self.item_height;
                    
                    if selected_y < self.scroll_pos {
                        log_debug(&format!("Adjusting scroll_pos from {} to {} (selection above visible area)", 
                            self.scroll_pos, selected_y));
                        self.scroll_pos = selected_y;
                        self.calculate_layout();
                    } else if selected_y >= self.scroll_pos + self.client_height - self.item_height {
                        let new_pos = selected_y - self.client_height + self.item_height;
                        log_debug(&format!("Adjusting scroll_pos from {} to {} (selection below visible area)", 
                            self.scroll_pos, new_pos));
                        self.scroll_pos = new_pos;
                        self.calculate_layout();
                    }
                }
                _ => {
                    // Grid mode
                    let row = selected as i32 / self.grid_cols;
                    let selected_y = row * self.cell_size;
                    
                    if selected_y < self.scroll_pos {
                        log_debug(&format!("Grid: Adjusting scroll_pos from {} to {} (selection above visible area)", 
                            self.scroll_pos, selected_y));
                        self.scroll_pos = selected_y;
                        self.calculate_layout();
                    } else if selected_y >= self.scroll_pos + self.client_height - self.cell_size {
                        let new_pos = selected_y - self.client_height + self.cell_size;
                        log_debug(&format!("Grid: Adjusting scroll_pos from {} to {} (selection below visible area)", 
                            self.scroll_pos, new_pos));
                        self.scroll_pos = new_pos;
                        self.calculate_layout();
                    }
                }
            }
        }
        
        log_debug(&format!("ensure_selection_visible completed, final scroll_pos: {}", self.scroll_pos));
    }

    fn get_item_at_point(&self, x: i32, y: i32) -> Option<usize> {
        if self.list_data.is_empty() {
            return None;
        }

        match self.view_mode {
            ViewMode::Details => {
                // Account for header height - clicks in header area return None
                if y < HEADER_HEIGHT {
                    return None;
                }
                
                let adjusted_y = y - HEADER_HEIGHT + (self.scroll_pos % self.item_height);
                let item_index = (self.scroll_pos + adjusted_y) / self.item_height;
                
                if item_index >= 0 && (item_index as usize) < self.list_data.len() {
                    Some(item_index as usize)
                } else {
                    None
                }
            }
            _ => {
                // Grid mode
                if self.cell_size <= 0 || self.grid_cols <= 0 {
                    return None;
                }
                
                let row = (y + self.scroll_pos) / self.cell_size;
                let col = x / self.cell_size;
                
                if col >= 0 && col < self.grid_cols && row >= 0 {
                    let index = (row * self.grid_cols + col) as usize;
                    if index < self.list_data.len() {
                        Some(index)
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
        }
    }

    fn open_selected_file(&self) {
        if let Some(selected) = self.selected_index {
            if selected < self.list_data.len() {
                let file_path = &self.list_data[selected].path;
                open_file(file_path);
            }
        }
    }

    fn set_view_mode(&mut self, new_mode: ViewMode) {
        // Convert old view mode to zoom level for backward compatibility
        let new_zoom_level = match new_mode {
            ViewMode::Details => 0,
            ViewMode::MediumIcons => 5,  // 64px
            ViewMode::LargeIcons => 10,  // 128px  
            ViewMode::ExtraLargeIcons => 13, // 256px
        };
        
        self.set_zoom_level(new_zoom_level);
    }

    fn set_zoom_level(&mut self, zoom_level: i32) {
        // Clamp zoom level to valid range
        let zoom_level = zoom_level.max(0).min(14);
        
        if self.zoom_level == zoom_level {
            return; // No change needed
        }
        
        log_debug(&format!("set_zoom_level: changing from {} to {}", self.zoom_level, zoom_level));
        
        // Calculate the current visible item index to preserve relative position
        let current_visible_item = if self.list_data.is_empty() {
            0
        } else {
            match self.view_mode {
                ViewMode::Details => {
                    // In details view, calculate which item is at the top
                    ((self.scroll_pos + HEADER_HEIGHT) / self.item_height) as usize
                }
                _ => {
                    // In grid view, calculate which item is at the top
                    if self.cell_size > 0 && self.grid_cols > 0 {
                        let row = self.scroll_pos / self.cell_size;
                        (row * self.grid_cols) as usize
                    } else {
                        0
                    }
                }
            }.min(self.list_data.len().saturating_sub(1))
        };
        
        log_debug(&format!("Preserving visible item index: {} (from scroll_pos: {})", current_visible_item, self.scroll_pos));
        
        self.zoom_level = zoom_level;
        self.view_mode = Self::get_view_mode_from_zoom_level(zoom_level);
        self.selected_view_size = Self::get_icon_size_from_zoom_level(zoom_level);
        
        // Clear thumbnail cache when switching zoom levels
        self.thumbnail_cache.clear();
        
        // Cancel all thumbnail tasks and recompute
        if let Some(ref task_manager) = self.thumbnail_task_manager {
            task_manager.cancel_all_tasks();
        }
        
        // Recalculate layout with new view mode
        self.calculate_layout();
        
        // Adjust scroll position to show the same item that was visible before
        let new_scroll_pos = if self.list_data.is_empty() {
            0
        } else {
            match self.view_mode {
                ViewMode::Details => {
                    // In details view, position to show the item at the top
                    (current_visible_item as i32 * self.item_height) - HEADER_HEIGHT
                }
                _ => {
                    // In grid view, position to show the item at the top
                    if self.grid_cols > 0 {
                        let row = current_visible_item as i32 / self.grid_cols;
                        row * self.cell_size
                    } else {
                        0
                    }
                }
            }.max(0).min((self.total_height - self.client_height).max(0))
        };
        
        log_debug(&format!("Adjusting scroll_pos from {} to {} to preserve visible item", self.scroll_pos, new_scroll_pos));
        self.scroll_pos = new_scroll_pos;
        
        // Recalculate layout again with the adjusted scroll position
        self.calculate_layout();
        
        // Update menu checkmarks
        update_view_menu_checkmarks(self.main_window, &self.view_mode);
        
        // Post message to recompute thumbnails
        unsafe {
            let _ = PostMessageW(self.main_window, WM_RECOMPUTE_THUMBS, WPARAM(0), LPARAM(0));
        }
        
        log_debug(&format!("set_zoom_level completed: final scroll_pos={}", self.scroll_pos));
    }

    fn set_thumbnail_strategy(&mut self, strategy: ThumbnailStrategy) {
        self.config.thumbnail_strategy = strategy;
        
        // Save configuration
        if let Err(e) = save_config(&self.config) {
            println!("Failed to save config: {}", e);
        }
        
        // Cancel all thumbnail tasks and recompute
        if let Some(ref task_manager) = self.thumbnail_task_manager {
            task_manager.cancel_all_tasks();
        }
        
        // Clear thumbnail cache
        self.thumbnail_cache.clear();
        
        // Post message to recompute thumbnails
        unsafe {
            let _ = PostMessageW(self.main_window, WM_RECOMPUTE_THUMBS, WPARAM(0), LPARAM(0));
        }
        
        // Update menu checkmarks
        update_thumbnail_menu_checkmarks(self.main_window, strategy);
        
        // Invalidate the list view
        unsafe {
            InvalidateRect(self.list_view, None, TRUE);
        }
        
        println!("Switched to thumbnail strategy: {:?}", strategy);
    }
    
    fn set_thumbnail_background(&mut self, background: ThumbnailBackground) {
        self.config.thumbnail_background = background;
        
        // Save configuration
        if let Err(e) = save_config(&self.config) {
            println!("Failed to save config: {}", e);
        }
        
        // Cancel all thumbnail tasks and recompute
        if let Some(ref task_manager) = self.thumbnail_task_manager {
            task_manager.cancel_all_tasks();
        }
        
        // Clear thumbnail cache
        self.thumbnail_cache.clear();
        
        // Post message to recompute thumbnails
        unsafe {
            let _ = PostMessageW(self.main_window, WM_RECOMPUTE_THUMBS, WPARAM(0), LPARAM(0));
        }
        
        // Update menu checkmarks
        update_background_menu_checkmarks(self.main_window, background);
        
        // Invalidate the list view
        unsafe {
            InvalidateRect(self.list_view, None, TRUE);
        }
        
        println!("Switched to thumbnail background: {:?}", background);
    }
    
    fn toggle_column(&mut self, column_type: ColumnType) {
        for column in &mut self.columns {
            if column.column_type == column_type {
                column.visible = !column.visible;
                break;
            }
        }
        
        // Update menu checkmarks
        update_column_menu_checkmarks(self.main_window, &self.columns);
        
        // Invalidate the list view to redraw with new columns
        unsafe {
            InvalidateRect(self.list_view, None, TRUE);
        }
        
        println!("Toggled column visibility: {:?}", column_type);
    }
    
    fn get_visible_columns(&self) -> Vec<&ColumnInfo> {
        self.columns.iter().filter(|col| col.visible).collect()
    }
    
    fn get_column_at_x(&self, x: i32) -> Option<usize> {
        let visible_columns = self.get_visible_columns();
        let mut current_x = 0;
        
        for (index, column) in visible_columns.iter().enumerate() {
            if x >= current_x && x < current_x + column.width {
                return Some(index);
            }
            current_x += column.width;
        }
        
        None
    }
    
    fn get_column_resize_cursor_x(&self, x: i32) -> Option<usize> {
        let visible_columns = self.get_visible_columns();
        let mut current_x = 0;
        let resize_margin = 3; // 3 pixels margin for resize cursor
        
        for (index, column) in visible_columns.iter().enumerate() {
            current_x += column.width;
            if x >= current_x - resize_margin && x <= current_x + resize_margin {
                return Some(index);
            }
        }
        
        None
    }
    
    fn sort_by_column(&mut self, column_type: ColumnType) {
        // Determine new sort order
        let new_order = match &self.sort_state {
            Some(state) if state.column == column_type => {
                match state.order {
                    SortOrder::None | SortOrder::Descending => SortOrder::Ascending,
                    SortOrder::Ascending => SortOrder::Descending,
                }
            }
            _ => SortOrder::Ascending,
        };
        
        // Update sort state
        self.sort_state = Some(SortState {
            column: column_type,
            order: new_order,
        });
        
        // Perform the sort
        match column_type {
            ColumnType::Name => {
                if new_order == SortOrder::Ascending {
                    self.list_data.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
                } else {
                    self.list_data.sort_by(|a, b| b.name.to_lowercase().cmp(&a.name.to_lowercase()));
                }
            }
            ColumnType::Size => {
                // Load metadata for all items before sorting (only for visible items to keep performance)
                for item in &mut self.list_data {
                    if item.size == 0 && item.modified_time == std::time::UNIX_EPOCH {
                        item.load_metadata();
                    }
                }
                
                if new_order == SortOrder::Ascending {
                    self.list_data.sort_by(|a, b| a.size.cmp(&b.size));
                } else {
                    self.list_data.sort_by(|a, b| b.size.cmp(&a.size));
                }
            }
            ColumnType::Type => {
                if new_order == SortOrder::Ascending {
                    self.list_data.sort_by(|a, b| a.file_type.cmp(&b.file_type));
                } else {
                    self.list_data.sort_by(|a, b| b.file_type.cmp(&a.file_type));
                }
            }
            ColumnType::Modified => {
                // Load metadata for all items before sorting
                for item in &mut self.list_data {
                    if item.size == 0 && item.modified_time == std::time::UNIX_EPOCH {
                        item.load_metadata();
                    }
                }
                
                if new_order == SortOrder::Ascending {
                    self.list_data.sort_by(|a, b| a.modified_time.cmp(&b.modified_time));
                } else {
                    self.list_data.sort_by(|a, b| b.modified_time.cmp(&a.modified_time));
                }
            }
            ColumnType::Path => {
                if new_order == SortOrder::Ascending {
                    self.list_data.sort_by(|a, b| a.path.to_lowercase().cmp(&b.path.to_lowercase()));
                } else {
                    self.list_data.sort_by(|a, b| b.path.to_lowercase().cmp(&a.path.to_lowercase()));
                }
            }
        }
        
        // Reset selection to first item
        self.selected_index = if !self.list_data.is_empty() { Some(0) } else { None };
        
        // Recalculate layout
        self.calculate_layout();
        
        println!("Sorted by {:?} in {:?} order", column_type, new_order);
    }
    
    fn set_language(&mut self, language: Language) {
        // Set the language
        if let Err(e) = lang::set_language(language) {
            println!("Failed to set language: {}", e);
            return;
        }
        
        // Update config
        self.config.language = match language {
            Language::English => LanguageCode::English,
            Language::Chinese => LanguageCode::Chinese,
        };
        
        // Save configuration
        if let Err(e) = save_config(&self.config) {
            println!("Failed to save config: {}", e);
        }
        
        // Update menu checkmarks
        update_language_menu_checkmarks(self.main_window, language);
        
        // Recreate the entire menu with new language strings
        recreate_menus_with_language(self.main_window);
        
        // Invalidate the list view to redraw with new language
        unsafe {
            InvalidateRect(self.list_view, None, TRUE);
        }
        
        println!("Language switched to: {:?}", language);
    }

    fn load_file_list(&mut self, file_path: &str) -> Result<()> {
        println!("Loading file list from: {}", file_path);
        
        // Read the file content
        let content = match std::fs::read_to_string(file_path) {
            Ok(content) => content,
            Err(_) => return Err(Error::from_win32()),
        };
        
        // Parse the file list
        let mut file_results = Vec::new();
        
        // Support multiple formats:
        // 1. Simple text list (one file path per line)
        // 2. CSV format (path,size,modified_timestamp)
        // 3. Basic EFU-like format
        
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            
            // Check if it's a CSV format (has commas)
            if line.contains(',') {
                let parts: Vec<&str> = line.split(',').collect();
                if parts.len() >= 1 {
                    let path = parts[0].trim().trim_matches('"');
                    if std::path::Path::new(path).exists() {
                        file_results.push(FileResult::from_path(path));
                    } else {
                        println!("Warning: File not found: {}", path);
                    }
                }
            } else {
                // Simple text format (one path per line)
                let path = line.trim_matches('"');
                if std::path::Path::new(path).exists() {
                    file_results.push(FileResult::from_path(path));
                } else {
                    println!("Warning: File not found: {}", path);
                }
            }
        }
        
        println!("Loaded {} files from list", file_results.len());
        
        // Update the app state
        self.list_data = file_results.clone();
        self.selected_index = if !self.list_data.is_empty() { Some(0) } else { None };
        self.scroll_pos = 0;
        
        // Set list mode state
        self.is_list_mode = true;
        self.current_list_name = Some(
            std::path::Path::new(file_path)
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string()
        );
        self.original_list_data = file_results.clone();
        
        unsafe {
            self.calculate_layout();
            update_scrollbar(self.list_view);
            InvalidateRect(self.list_view, None, TRUE);
            update_status_bar();
            
            // Clear the search edit box to indicate we're in list mode
            SetWindowTextW(self.search_edit, w!(""));
        }
        
        Ok(())
    }
    
    fn save_file_list(&self, file_path: &str) -> Result<()> {
        println!("Saving file list to: {}", file_path);
        
        // Create CSV format with file paths and metadata
        let mut content = String::new();
        content.push_str("# File List Export\n");
        content.push_str("# Format: \"Path\",Size,Modified\n");
        
        for item in &self.list_data {
            // Load metadata if not already loaded
            let mut item_clone = item.clone();
            if item_clone.size == 0 && item_clone.modified_time == std::time::UNIX_EPOCH {
                item_clone.load_metadata();
            }
            
            // Format: "path",size,modified_timestamp
            let modified_timestamp = item_clone.modified_time
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            
            content.push_str(&format!("\"{}\",{},{}\n", 
                item.path, 
                item_clone.size,
                modified_timestamp
            ));
        }
        
        // Write to file
        match std::fs::write(file_path, content) {
            Ok(_) => {
                println!("Successfully saved {} files to list", self.list_data.len());
                Ok(())
            }
            Err(_) => Err(Error::from_win32()),
        }
    }
    
    fn export_simple_list(&self, file_path: &str) -> Result<()> {
        println!("Exporting simple file list to: {}", file_path);
        
        // Create simple text format - one path per line
        let mut content = String::new();
        for item in &self.list_data {
            content.push_str(&format!("{}\n", item.path));
        }
        
        // Write to file
        match std::fs::write(file_path, content) {
            Ok(_) => {
                println!("Successfully exported {} files to simple list", self.list_data.len());
                Ok(())
            }
            Err(_) => Err(Error::from_win32()),
        }
    }

    fn recompute_thumbnail_queue(&self) {
        log_debug("recompute_thumbnail_queue called");
        
        if let Some(ref task_manager) = self.thumbnail_task_manager {
            log_debug(&format!("Thumbnail task manager available, view_mode: {:?}, selected_view_size: {}", 
                self.view_mode, self.selected_view_size));
            
            if self.view_mode != ViewMode::Details && self.selected_view_size > 0 {
                log_debug("Calling task_manager.recompute_thumbnail_queue");
                
                task_manager.recompute_thumbnail_queue(
                    self.config.thumbnail_strategy,
                    self.config.thumbnail_background,
                    self.visible_start,
                    self.visible_count,
                    self.list_data.len(),
                    &self.list_data,
                    self.selected_view_size,
                );
                
                log_debug("task_manager.recompute_thumbnail_queue completed");
            } else {
                log_debug(&format!("Skipping thumbnail queue recomputation (view_mode: {:?}, size: {})", 
                    self.view_mode, self.selected_view_size));
            }
        } else {
            log_debug("No thumbnail task manager available");
        }
        
        log_debug("recompute_thumbnail_queue completed");
    }

    // Async search methods
    fn start_async_search(&mut self, query: String) {
        log_debug(&format!("start_async_search called with query: '{}'", query));
        
        // Cancel any existing search
        self.search_cancel_flag.store(true, Ordering::Relaxed);
        log_debug("Cancelled existing search");
        
        // Increment generation counter and get new values
        let generation = self.search_generation.fetch_add(1, Ordering::Relaxed) + 1;
        let cancel_flag = Arc::new(AtomicBool::new(false));
        self.search_cancel_flag = cancel_flag.clone();
        
        log_debug(&format!("New search generation: {}", generation));
        
        // Store the pending search for debouncing
        self.pending_search_query = query.clone();
        self.last_search_time = Instant::now();
        
        // Check if we have Everything SDK available
        if let Some(ref sender) = self.search_sender {
            log_debug("Sending search request to Everything SDK thread");
            
            let request = SearchRequest {
                query: query.clone(),
                generation,
                window: self.main_window,
                cancel_flag: cancel_flag.clone(),
            };
            
            if let Err(e) = sender.send(request) {
                log_debug(&format!("Failed to send search request: {}", e));
            } else {
                log_debug("Search request sent to Everything SDK thread successfully");
            }
        } else {
            log_debug("No Everything SDK available, using sample data with rayon");
            
            // For sample data, use rayon (thread-safe)
            let window = self.main_window;
            let query_clone = query.clone();
            
            rayon::spawn(move || {
                log_debug(&format!("Sample data background thread started for query: '{}'", query_clone));
                
                // Small delay to allow for more keystrokes (debouncing)
                std::thread::sleep(Duration::from_millis(150));
                
                // Check if we've been cancelled during the delay
                if cancel_flag.load(Ordering::Relaxed) {
                    log_debug("Sample data search cancelled during debounce delay");
                    return;
                }
                
                log_debug("Starting sample data filtering");
                
                // Use sample data filtering
                let search_result: std::result::Result<Vec<String>, String> = if query_clone.trim().is_empty() {
                    // Return all sample data
                    let mut results = Vec::new();
                    for i in 0..100000 {
                        results.push(format!("C:\\Users\\Example\\Documents\\File_{:06}.txt", i));
                    }
                    Ok(results)
                } else {
                    // Filter sample data by query
                    let query_lower = query_clone.to_lowercase();
                    let mut results = Vec::new();
                    for i in 0..100000 {
                        let filename = format!("File_{:06}.txt", i);
                        let path = format!("C:\\Users\\Example\\Documents\\File_{:06}.txt", i);
                        
                        // Simple string matching
                        if filename.to_lowercase().contains(&query_lower) || 
                           path.to_lowercase().contains(&query_lower) {
                            results.push(path);
                        }
                    }
                    Ok(results)
                };
                
                // Check if we've been cancelled after the search
                if cancel_flag.load(Ordering::Relaxed) {
                    log_debug("Sample data search cancelled after filtering");
                    return;
                }
                
                log_debug("Sample data filtering completed, sending results to UI thread");
                
                // Send results back to UI thread
                match search_result {
                    Ok(file_paths) => {
                        log_debug(&format!("Converting {} sample file paths to FileResult objects", file_paths.len()));
                        
                        let results: Vec<crate::everything_sdk::FileResult> = file_paths
                            .into_iter()
                            .map(|path| crate::everything_sdk::FileResult::from_path(&path))
                            .collect();
                        
                        // Allocate results in a Box and send the pointer
                        let boxed_results = Box::new((results, generation));
                        let results_ptr = Box::into_raw(boxed_results) as isize;
                        
                        log_debug(&format!("Posting WM_SEARCH_RESULTS message with ptr: {}", results_ptr));
                        
                        unsafe {
                            let _ = PostMessageW(window, WM_SEARCH_RESULTS, WPARAM(results_ptr as usize), LPARAM(0));
                        }
                    }
                    Err(e) => {
                        log_debug(&format!("Sample data search failed: {}", e));
                        // Send empty results on error
                        let boxed_results = Box::new((Vec::<crate::everything_sdk::FileResult>::new(), generation));
                        let results_ptr = Box::into_raw(boxed_results) as isize;
                        
                        unsafe {
                            let _ = PostMessageW(window, WM_SEARCH_RESULTS, WPARAM(results_ptr as usize), LPARAM(0));
                        }
                    }
                }
                
                log_debug("Sample data background thread completed");
            });
        }
        
        log_debug("start_async_search completed");
    }
    
    fn handle_search_results(&mut self, results_ptr: isize) {
        log_debug(&format!("handle_search_results called with ptr: {}", results_ptr));
        
        unsafe {
            log_debug("Converting pointer back to Box");
            // Convert pointer back to Box
            let boxed_results = Box::from_raw(results_ptr as *mut (Vec<crate::everything_sdk::FileResult>, u64));
            let (mut results, generation) = *boxed_results;
            
            log_debug(&format!("Unpacked results: {} items, generation: {}", results.len(), generation));
            
            // Check if this result is from the current generation
            let current_generation = self.search_generation.load(Ordering::Relaxed);
            if generation != current_generation {
                log_debug(&format!("Ignoring old search results (gen {} vs current {})", generation, current_generation));
                // This is from an old search, ignore it
                return;
            }
            
            log_debug(&format!("Received async search results: {} items", results.len()));
            
            // Limit results to prevent UI slowdown
            if results.len() > 50000 {
                results.truncate(50000);
                log_debug("Truncated results to 50000 items for performance");
            }
            
            log_debug("About to update list_data");
            // Update UI with results
            self.list_data = results;
            log_debug(&format!("Updated list_data, new size: {}", self.list_data.len()));
            
            self.selected_index = if !self.list_data.is_empty() { Some(0) } else { None };
            log_debug("Updated selected_index");
            
            // Only reset scroll position if we're not currently dragging the scrollbar
            // This prevents the scrollbar from jumping back to the top during scroll operations
            if !self.is_scrollbar_dragging {
            self.scroll_pos = 0;
                log_debug("Reset scroll position (not dragging)");
            } else {
                log_debug("Preserving scroll position during scrollbar dragging");
            }
            
            self.calculate_layout();
            log_debug("Calculated layout");
            
            // Cancel all thumbnail tasks and recompute
            if let Some(ref task_manager) = self.thumbnail_task_manager {
                log_debug("Cancelling all thumbnail tasks");
                task_manager.cancel_all_tasks();
            }
            
            // Clear thumbnail cache
            log_debug("Clearing thumbnail cache");
            self.thumbnail_cache.clear();
            
            // Post message to recompute thumbnails
            log_debug("Posting WM_RECOMPUTE_THUMBS message");
            let _ = PostMessageW(self.main_window, WM_RECOMPUTE_THUMBS, WPARAM(0), LPARAM(0));
            
            // Update UI
            log_debug("About to update UI components");
            if let Some(state) = &APP_STATE {
                log_debug("Updating scrollbar");
                update_scrollbar(state.list_view);
                log_debug("Invalidating list view");
                InvalidateRect(state.list_view, None, TRUE);
                log_debug("Updating status bar");
                update_status_bar();
                log_debug("UI update completed");
            } else {
                log_debug("WARNING: APP_STATE is None during UI update");
            }
            
            log_debug("handle_search_results completed successfully");
        }
    }

    fn search_local_list(&mut self, query: &str) {
        if !self.is_list_mode || self.original_list_data.is_empty() {
            return;
        }

        if query.trim().is_empty() {
            // Show all files when query is empty
            self.list_data = self.original_list_data.clone();
        } else {
            // Filter files based on query
            let query_lower = query.to_lowercase();
            self.list_data = self.original_list_data
                .iter()
                .filter(|file| {
                    file.name.to_lowercase().contains(&query_lower) ||
                    file.path.to_lowercase().contains(&query_lower)
                })
                .cloned()
                .collect();
        }

        // Reset selection and scroll
        self.selected_index = if !self.list_data.is_empty() { Some(0) } else { None };
        self.scroll_pos = 0;

        unsafe {
            self.calculate_layout();
            update_scrollbar(self.list_view);
            InvalidateRect(self.list_view, None, TRUE);
            update_status_bar();
        }
    }

    fn close_file_list(&mut self) {
        self.list_data.clear();
        self.selected_index = None;
        self.scroll_pos = 0;
        self.is_list_mode = false;
        self.current_list_name = None;
        self.original_list_data.clear();

        unsafe {
            // Restore default search to show all files
            SetWindowTextW(self.search_edit, w!("*.png"));

            self.calculate_layout();
            update_scrollbar(self.list_view);
            InvalidateRect(self.list_view, None, TRUE);
            update_status_bar();

            // Trigger a search to reload global data
            handle_immediate_search();
        }
    }

    fn change_sort_order(&mut self, new_order: SortOrder) {
        if let Some(ref mut sort_state) = self.sort_state {
            // If we have an existing sort state, just change the order
            sort_state.order = new_order;
            
            // Re-sort with the new order
            self.apply_sort();
        } else {
            // If no sort state exists, create one with the default column (Name)
            self.sort_state = Some(SortState {
                column: ColumnType::Name,
                order: new_order,
            });
            self.apply_sort();
        }
    }

    fn apply_sort(&mut self) {
        if let Some(sort_state) = self.sort_state.clone() {
            let column_type = sort_state.column;
            let order = sort_state.order;
            
            // Perform the sort
            match column_type {
                ColumnType::Name => {
                    if order == SortOrder::Ascending {
                        self.list_data.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
                    } else {
                        self.list_data.sort_by(|a, b| b.name.to_lowercase().cmp(&a.name.to_lowercase()));
                    }
                }
                ColumnType::Size => {
                    // Load metadata for all items before sorting (only for visible items to keep performance)
                    for item in &mut self.list_data {
                        if item.size == 0 && item.modified_time == std::time::UNIX_EPOCH {
                            item.load_metadata();
                        }
                    }
                    
                    if order == SortOrder::Ascending {
                        self.list_data.sort_by(|a, b| a.size.cmp(&b.size));
                    } else {
                        self.list_data.sort_by(|a, b| b.size.cmp(&a.size));
                    }
                }
                ColumnType::Type => {
                    if order == SortOrder::Ascending {
                        self.list_data.sort_by(|a, b| a.file_type.cmp(&b.file_type));
                    } else {
                        self.list_data.sort_by(|a, b| b.file_type.cmp(&a.file_type));
                    }
                }
                ColumnType::Modified => {
                    // Load metadata for all items before sorting
                    for item in &mut self.list_data {
                        if item.size == 0 && item.modified_time == std::time::UNIX_EPOCH {
                            item.load_metadata();
                        }
                    }
                    
                    if order == SortOrder::Ascending {
                        self.list_data.sort_by(|a, b| a.modified_time.cmp(&b.modified_time));
                    } else {
                        self.list_data.sort_by(|a, b| b.modified_time.cmp(&a.modified_time));
                    }
                }
                ColumnType::Path => {
                    if order == SortOrder::Ascending {
                        self.list_data.sort_by(|a, b| a.path.to_lowercase().cmp(&b.path.to_lowercase()));
                    } else {
                        self.list_data.sort_by(|a, b| b.path.to_lowercase().cmp(&a.path.to_lowercase()));
                    }
                }
            }
            
            // Reset selection to first item
            self.selected_index = if !self.list_data.is_empty() { Some(0) } else { None };
            
            // Recalculate layout
            self.calculate_layout();
            
            println!("Applied sort by {:?} in {:?} order", column_type, order);
        }
    }
}

fn main() -> Result<()> {
    unsafe {
        init_logger();
        log_debug("Application starting");
        
        let instance = GetModuleHandleW(None)?;
        log_debug("Got module handle");
        
        APP_STATE = Some(AppState::new());
        log_debug("Created app state");
        
        register_main_window_class(instance)?;
        register_list_view_class(instance)?;
        log_debug("Registered window classes");
        
        let window = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("EverythingLikeMainWindow"),
            w!("Everything-like File Browser"),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            1000,
            700,
            None,
            None,
            instance,
            None,
        );

        if window.0 == 0 {
            log_debug("Failed to create window");
            return Err(Error::from_win32());
        }

        log_debug("Created main window");

        ShowWindow(window, SW_SHOW);
        UpdateWindow(window);
        log_debug("Window shown and updated");

        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).into() {
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }

        log_debug("Message loop ended");
        Ok(())
    }
}

fn register_main_window_class(instance: HMODULE) -> Result<()> {
    unsafe {
        let window_class = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(main_window_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: instance.into(),
            hIcon: LoadIconW(None, IDI_APPLICATION)?,
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            hbrBackground: CreateSolidBrush(COLORREF(0x00F0F0F0)),
            lpszMenuName: PCWSTR::null(),
            lpszClassName: w!("EverythingLikeMainWindow"),
            hIconSm: HICON(0),
        };

        let atom = RegisterClassExW(&window_class);
        if atom == 0 {
            return Err(Error::from_win32());
        }

        Ok(())
    }
}

fn register_list_view_class(instance: HMODULE) -> Result<()> {
    unsafe {
        let window_class = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW | CS_DBLCLKS,
            lpfnWndProc: Some(list_view_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: instance.into(),
            hIcon: HICON(0),
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            hbrBackground: CreateSolidBrush(COLORREF(0x00FFFFFF)),
            lpszMenuName: PCWSTR::null(),
            lpszClassName: w!("EverythingLikeListView"),
            hIconSm: HICON(0),
        };

        let atom = RegisterClassExW(&window_class);
        if atom == 0 {
            return Err(Error::from_win32());
        }

        Ok(())
    }
}

fn create_menus(window: HWND) -> Result<()> {
    recreate_menus_with_language(window)
}

fn recreate_menus_with_language(window: HWND) -> Result<()> {
    unsafe {
        // Destroy existing menu
        let old_menu = GetMenu(window);
        if !old_menu.is_invalid() {
            DestroyMenu(old_menu);
        }
        
        let hmenu = CreateMenu()?;
        let strings = get_strings();
        
        // Create File submenu
        let file_submenu = CreatePopupMenu()?;
        
        let _ = AppendMenuW(
            file_submenu,
            MF_STRING,
            ID_FILE_OPEN_LIST as usize,
            PCWSTR::from_raw(to_wide(&strings.file_open_list).as_ptr()),
        );
        
        let _ = AppendMenuW(
            file_submenu,
            MF_SEPARATOR,
            0,
            PCWSTR::null(),
        );
        
        let _ = AppendMenuW(
            file_submenu,
            MF_STRING,
            ID_FILE_SAVE_LIST as usize,
            PCWSTR::from_raw(to_wide(&strings.file_save_list).as_ptr()),
        );
        
        let _ = AppendMenuW(
            file_submenu,
            MF_STRING,
            ID_FILE_EXPORT_LIST as usize,
            PCWSTR::from_raw(to_wide(&strings.file_export_list).as_ptr()),
        );

        let _ = AppendMenuW(
            file_submenu,
            MF_SEPARATOR,
            0,
            PCWSTR::null(),
        );

        let _ = AppendMenuW(
            file_submenu,
            MF_STRING,
            ID_FILE_CLOSE_LIST as usize,
            PCWSTR::from_raw(to_wide(&strings.file_close_list).as_ptr()),
        );
        
        let _ = AppendMenuW(
            hmenu,
            MF_STRING | MF_POPUP,
            file_submenu.0 as usize,
            PCWSTR::from_raw(to_wide(&strings.menu_file).as_ptr()),
        );
        
        // Create View submenu
        let view_submenu = CreatePopupMenu()?;
        
        let _ = AppendMenuW(
            view_submenu,
            MF_STRING,
            ID_VIEW_DETAILS as usize,
            PCWSTR::from_raw(to_wide(&strings.view_details).as_ptr()),
        );
        
        let _ = AppendMenuW(
            view_submenu,
            MF_STRING,
            ID_VIEW_MEDIUM_ICONS as usize,
            PCWSTR::from_raw(to_wide(&strings.view_medium_icons).as_ptr()),
        );
        
        let _ = AppendMenuW(
            view_submenu,
            MF_STRING,
            ID_VIEW_LARGE_ICONS as usize,
            PCWSTR::from_raw(to_wide(&strings.view_large_icons).as_ptr()),
        );
        
        let _ = AppendMenuW(
            view_submenu,
            MF_STRING,
            ID_VIEW_EXTRALARGE_ICONS as usize,
            PCWSTR::from_raw(to_wide(&strings.view_extra_large_icons).as_ptr()),
        );
        
        let _ = AppendMenuW(
            hmenu,
            MF_STRING | MF_POPUP,
            view_submenu.0 as usize,
            PCWSTR::from_raw(to_wide(&strings.menu_view).as_ptr()),
        );
        
        // Create Columns submenu
        let columns_submenu = CreatePopupMenu()?;
        
        let _ = AppendMenuW(
            columns_submenu,
            MF_STRING,
            ID_COLUMN_NAME as usize,
            PCWSTR::from_raw(to_wide(&strings.column_name).as_ptr()),
        );
        
        let _ = AppendMenuW(
            columns_submenu,
            MF_STRING,
            ID_COLUMN_SIZE as usize,
            PCWSTR::from_raw(to_wide(&strings.column_size).as_ptr()),
        );
        
        let _ = AppendMenuW(
            columns_submenu,
            MF_STRING,
            ID_COLUMN_TYPE as usize,
            PCWSTR::from_raw(to_wide(&strings.column_type).as_ptr()),
        );
        
        let _ = AppendMenuW(
            columns_submenu,
            MF_STRING,
            ID_COLUMN_MODIFIED as usize,
            PCWSTR::from_raw(to_wide(&strings.column_date_modified).as_ptr()),
        );
        
        let _ = AppendMenuW(
            columns_submenu,
            MF_STRING,
            ID_COLUMN_PATH as usize,
            PCWSTR::from_raw(to_wide(&strings.column_path).as_ptr()),
        );
        
        let _ = AppendMenuW(
            hmenu,
            MF_STRING | MF_POPUP,
            columns_submenu.0 as usize,
            PCWSTR::from_raw(to_wide(&strings.menu_columns).as_ptr()),
        );
        
        // Create Language submenu
        let lang_submenu = CreatePopupMenu()?;
        
        let _ = AppendMenuW(
            lang_submenu,
            MF_STRING,
            ID_LANG_ENGLISH as usize,
            PCWSTR::from_raw(to_wide(&strings.lang_english).as_ptr()),
        );
        
        let _ = AppendMenuW(
            lang_submenu,
            MF_STRING,
            ID_LANG_CHINESE as usize,
            PCWSTR::from_raw(to_wide(&strings.lang_chinese).as_ptr()),
        );
        
        let _ = AppendMenuW(
            hmenu,
            MF_STRING | MF_POPUP,
            lang_submenu.0 as usize,
            PCWSTR::from_raw(to_wide(&strings.menu_language).as_ptr()),
        );
        
        // Create Sort submenu
        let sort_submenu = CreatePopupMenu()?;
        
        let _ = AppendMenuW(
            sort_submenu,
            MF_STRING,
            ID_SORT_NAME as usize,
            PCWSTR::from_raw(to_wide(&strings.sort_name).as_ptr()),
        );
        
        let _ = AppendMenuW(
            sort_submenu,
            MF_STRING,
            ID_SORT_SIZE as usize,
            PCWSTR::from_raw(to_wide(&strings.sort_size).as_ptr()),
        );
        
        let _ = AppendMenuW(
            sort_submenu,
            MF_STRING,
            ID_SORT_TYPE as usize,
            PCWSTR::from_raw(to_wide(&strings.sort_type).as_ptr()),
        );
        
        let _ = AppendMenuW(
            sort_submenu,
            MF_STRING,
            ID_SORT_DATE as usize,
            PCWSTR::from_raw(to_wide(&strings.sort_date).as_ptr()),
        );
        
        let _ = AppendMenuW(
            sort_submenu,
            MF_STRING,
            ID_SORT_PATH as usize,
            PCWSTR::from_raw(to_wide(&strings.sort_path).as_ptr()),
        );
        
        // Add separator
        let _ = AppendMenuW(
            sort_submenu,
            MF_SEPARATOR,
            0,
            PCWSTR::null(),
        );
        
        // Add sort order options
        let _ = AppendMenuW(
            sort_submenu,
            MF_STRING,
            ID_SORT_ASCENDING as usize,
            PCWSTR::from_raw(to_wide(&strings.sort_ascending).as_ptr()),
        );
        
        let _ = AppendMenuW(
            sort_submenu,
            MF_STRING,
            ID_SORT_DESCENDING as usize,
            PCWSTR::from_raw(to_wide(&strings.sort_descending).as_ptr()),
        );
        
        let _ = AppendMenuW(
            hmenu,
            MF_STRING | MF_POPUP,
            sort_submenu.0 as usize,
            PCWSTR::from_raw(to_wide(&strings.menu_sort).as_ptr()),
        );
        
        // Create Thumbnail Options submenu
        let thumb_submenu = CreatePopupMenu()?;
        
        let _ = AppendMenuW(
            thumb_submenu,
            MF_STRING,
            ID_THUMB_DEFAULT as usize,
            PCWSTR::from_raw(to_wide(&strings.thumb_default).as_ptr()),
        );
        
        let _ = AppendMenuW(
            thumb_submenu,
            MF_STRING,
            ID_THUMB_VISIBLE as usize,
            PCWSTR::from_raw(to_wide(&strings.thumb_visible).as_ptr()),
        );
        
        let _ = AppendMenuW(
            thumb_submenu,
            MF_STRING,
            ID_THUMB_VISIBLE_PLUS_500 as usize,
            PCWSTR::from_raw(to_wide(&strings.thumb_visible_plus_500).as_ptr()),
        );
        
        let _ = AppendMenuW(
            hmenu,
            MF_STRING | MF_POPUP,
            thumb_submenu.0 as usize,
            PCWSTR::from_raw(to_wide(&strings.menu_thumbnail_options).as_ptr()),
        );
        
        // Create Thumbnail Background submenu
        let bg_submenu = CreatePopupMenu()?;
        
        let _ = AppendMenuW(
            bg_submenu,
            MF_STRING,
            ID_BG_TRANSPARENT as usize,
            PCWSTR::from_raw(to_wide(&strings.bg_transparent).as_ptr()),
        );
        
        let _ = AppendMenuW(
            bg_submenu,
            MF_STRING,
            ID_BG_CHECKERBOARD as usize,
            PCWSTR::from_raw(to_wide(&strings.bg_checkerboard).as_ptr()),
        );
        
        let _ = AppendMenuW(
            bg_submenu,
            MF_SEPARATOR,
            0,
            PCWSTR::null(),
        );
        
        let _ = AppendMenuW(
            bg_submenu,
            MF_STRING,
            ID_BG_BLACK as usize,
            PCWSTR::from_raw(to_wide(&strings.bg_black).as_ptr()),
        );
        
        let _ = AppendMenuW(
            bg_submenu,
            MF_STRING,
            ID_BG_WHITE as usize,
            PCWSTR::from_raw(to_wide(&strings.bg_white).as_ptr()),
        );
        
        let _ = AppendMenuW(
            bg_submenu,
            MF_STRING,
            ID_BG_GRAY as usize,
            PCWSTR::from_raw(to_wide(&strings.bg_gray).as_ptr()),
        );
        
        let _ = AppendMenuW(
            bg_submenu,
            MF_STRING,
            ID_BG_LIGHT_GRAY as usize,
            PCWSTR::from_raw(to_wide(&strings.bg_light_gray).as_ptr()),
        );
        
        let _ = AppendMenuW(
            bg_submenu,
            MF_STRING,
            ID_BG_DARK_GRAY as usize,
            PCWSTR::from_raw(to_wide(&strings.bg_dark_gray).as_ptr()),
        );
        
        let _ = AppendMenuW(
            hmenu,
            MF_STRING | MF_POPUP,
            bg_submenu.0 as usize,
            PCWSTR::from_raw(to_wide(&strings.menu_thumbnail_background).as_ptr()),
        );
        
        let _ = SetMenu(window, hmenu);
        
        // Set initial checkmarks based on loaded config and current view mode
        if let Some(state) = &APP_STATE {
            update_thumbnail_menu_checkmarks(window, state.config.thumbnail_strategy);
            update_background_menu_checkmarks(window, state.config.thumbnail_background);
            update_view_menu_checkmarks(window, &state.view_mode);
            update_column_menu_checkmarks(window, &state.columns);
            update_language_menu_checkmarks(window, get_current_language());
            update_sort_menu_checkmarks(window, &state.sort_state);
        }
        
        Ok(())
    }
}

fn update_thumbnail_menu_checkmarks(window: HWND, strategy: ThumbnailStrategy) {
    unsafe {
        let hmenu = GetMenu(window);
        if !hmenu.is_invalid() {
            // Uncheck all items first
            CheckMenuItem(hmenu, ID_THUMB_DEFAULT as u32, MF_UNCHECKED.0);
            CheckMenuItem(hmenu, ID_THUMB_VISIBLE as u32, MF_UNCHECKED.0);
            CheckMenuItem(hmenu, ID_THUMB_VISIBLE_PLUS_500 as u32, MF_UNCHECKED.0);
            
            // Check the current strategy
            let current_id = match strategy {
                ThumbnailStrategy::DefaultTopToBottom => ID_THUMB_DEFAULT,
                ThumbnailStrategy::OnlyLoadVisible => ID_THUMB_VISIBLE,
                ThumbnailStrategy::LoadVisiblePlus500 => ID_THUMB_VISIBLE_PLUS_500,
            };
            
            CheckMenuItem(hmenu, current_id as u32, MF_CHECKED.0);
        }
    }
}

fn update_view_menu_checkmarks(window: HWND, mode: &ViewMode) {
    unsafe {
        let hmenu = GetMenu(window);
        if !hmenu.is_invalid() {
            // Uncheck all items first
            CheckMenuItem(hmenu, ID_VIEW_DETAILS as u32, MF_UNCHECKED.0);
            CheckMenuItem(hmenu, ID_VIEW_MEDIUM_ICONS as u32, MF_UNCHECKED.0);
            CheckMenuItem(hmenu, ID_VIEW_LARGE_ICONS as u32, MF_UNCHECKED.0);
            CheckMenuItem(hmenu, ID_VIEW_EXTRALARGE_ICONS as u32, MF_UNCHECKED.0);
            
            // Check the current mode
            let current_id = match mode {
                ViewMode::Details => ID_VIEW_DETAILS,
                ViewMode::MediumIcons => ID_VIEW_MEDIUM_ICONS,
                ViewMode::LargeIcons => ID_VIEW_LARGE_ICONS,
                ViewMode::ExtraLargeIcons => ID_VIEW_EXTRALARGE_ICONS,
            };
            
            CheckMenuItem(hmenu, current_id as u32, MF_CHECKED.0);
        }
    }
}

fn update_background_menu_checkmarks(window: HWND, background: ThumbnailBackground) {
    unsafe {
        let hmenu = GetMenu(window);
        if !hmenu.is_invalid() {
            // Uncheck all items first
            CheckMenuItem(hmenu, ID_BG_TRANSPARENT as u32, MF_UNCHECKED.0);
            CheckMenuItem(hmenu, ID_BG_CHECKERBOARD as u32, MF_UNCHECKED.0);
            CheckMenuItem(hmenu, ID_BG_BLACK as u32, MF_UNCHECKED.0);
            CheckMenuItem(hmenu, ID_BG_WHITE as u32, MF_UNCHECKED.0);
            CheckMenuItem(hmenu, ID_BG_GRAY as u32, MF_UNCHECKED.0);
            CheckMenuItem(hmenu, ID_BG_LIGHT_GRAY as u32, MF_UNCHECKED.0);
            CheckMenuItem(hmenu, ID_BG_DARK_GRAY as u32, MF_UNCHECKED.0);
            
            // Check the current background
            let current_id = match background {
                ThumbnailBackground::Transparent => ID_BG_TRANSPARENT,
                ThumbnailBackground::Checkerboard => ID_BG_CHECKERBOARD,
                ThumbnailBackground::Black => ID_BG_BLACK,
                ThumbnailBackground::White => ID_BG_WHITE,
                ThumbnailBackground::Gray => ID_BG_GRAY,
                ThumbnailBackground::LightGray => ID_BG_LIGHT_GRAY,
                ThumbnailBackground::DarkGray => ID_BG_DARK_GRAY,
            };
            
            CheckMenuItem(hmenu, current_id as u32, MF_CHECKED.0);
        }
    }
}

fn update_column_menu_checkmarks(window: HWND, columns: &Vec<ColumnInfo>) {
    unsafe {
        let hmenu = GetMenu(window);
        if !hmenu.is_invalid() {
            // Check columns based on their visibility
            for column in columns {
                let menu_id = match column.column_type {
                    ColumnType::Name => ID_COLUMN_NAME,
                    ColumnType::Size => ID_COLUMN_SIZE,
                    ColumnType::Type => ID_COLUMN_TYPE,
                    ColumnType::Modified => ID_COLUMN_MODIFIED,
                    ColumnType::Path => ID_COLUMN_PATH,
                };
                
                let check_state = if column.visible { MF_CHECKED.0 } else { MF_UNCHECKED.0 };
                CheckMenuItem(hmenu, menu_id as u32, check_state);
            }
        }
    }
}

fn update_language_menu_checkmarks(window: HWND, language: Language) {
    unsafe {
        let hmenu = GetMenu(window);
        if !hmenu.is_invalid() {
            // Uncheck all items first
            CheckMenuItem(hmenu, ID_LANG_ENGLISH as u32, MF_UNCHECKED.0);
            CheckMenuItem(hmenu, ID_LANG_CHINESE as u32, MF_UNCHECKED.0);
            
            // Check the current language
            let current_id = match language {
                Language::English => ID_LANG_ENGLISH,
                Language::Chinese => ID_LANG_CHINESE,
            };
            
            CheckMenuItem(hmenu, current_id as u32, MF_CHECKED.0);
        }
    }
}

fn update_sort_menu_checkmarks(window: HWND, sort_state: &Option<SortState>) {
    unsafe {
        let hmenu = GetMenu(window);
        if !hmenu.is_invalid() {
            // Uncheck all items first
            CheckMenuItem(hmenu, ID_SORT_NAME as u32, MF_UNCHECKED.0);
            CheckMenuItem(hmenu, ID_SORT_SIZE as u32, MF_UNCHECKED.0);
            CheckMenuItem(hmenu, ID_SORT_TYPE as u32, MF_UNCHECKED.0);
            CheckMenuItem(hmenu, ID_SORT_DATE as u32, MF_UNCHECKED.0);
            CheckMenuItem(hmenu, ID_SORT_PATH as u32, MF_UNCHECKED.0);
            CheckMenuItem(hmenu, ID_SORT_ASCENDING as u32, MF_UNCHECKED.0);
            CheckMenuItem(hmenu, ID_SORT_DESCENDING as u32, MF_UNCHECKED.0);
            
            // Check the current sort column and order if any
            if let Some(state) = sort_state {
                let current_id = match state.column {
                    ColumnType::Name => ID_SORT_NAME,
                    ColumnType::Size => ID_SORT_SIZE,
                    ColumnType::Type => ID_SORT_TYPE,
                    ColumnType::Modified => ID_SORT_DATE,
                    ColumnType::Path => ID_SORT_PATH,
                };
                
                CheckMenuItem(hmenu, current_id as u32, MF_CHECKED.0);
                
                // Check the current sort order
                match state.order {
                    SortOrder::Ascending => {
                        CheckMenuItem(hmenu, ID_SORT_ASCENDING as u32, MF_CHECKED.0);
                    }
                    SortOrder::Descending => {
                        CheckMenuItem(hmenu, ID_SORT_DESCENDING as u32, MF_CHECKED.0);
                    }
                    SortOrder::None => {
                        // No order checkmark
                    }
                }
            }
        }
    }
}

extern "system" fn list_view_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        match message {
            WM_CREATE => {
                update_scrollbar(window);
                LRESULT(0)
            }
            WM_SIZE => {
                if let Some(state) = &mut APP_STATE {
                    let mut rect = RECT::default();
                    let _ = GetClientRect(window, &mut rect);
                    state.client_height = rect.bottom - rect.top;
                    state.client_width = rect.right - rect.left;
                    state.calculate_layout();
                    update_scrollbar(window);
                    
                    // Post message to main window to recompute thumbnails
                    let _ = PostMessageW(GetParent(window), WM_RECOMPUTE_THUMBS, WPARAM(0), LPARAM(0));
                }
                LRESULT(0)
            }
            WM_PAINT => {
                paint_list_view(window);
                LRESULT(0)
            }
            WM_LBUTTONDOWN => {
                // Set focus to receive keyboard input
                SetFocus(window);
                
                if let Some(state) = &mut APP_STATE {
                    let x = (lparam.0 & 0xFFFF) as i16 as i32;
                    let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
                    
                    // Check if we're in details view and clicking in header area
                    if state.view_mode == ViewMode::Details && y < HEADER_HEIGHT {
                        // Check if we're clicking on a column resize area
                        if let Some(column_index) = state.get_column_resize_cursor_x(x) {
                            // Start column resize drag
                            let visible_columns = state.get_visible_columns();
                            if column_index < visible_columns.len() {
                                state.column_drag_state = Some(ColumnDragState {
                                    is_dragging: true,
                                    column_index,
                                    start_x: x,
                                    start_width: visible_columns[column_index].width,
                                });
                                
                                // Capture mouse
                                SetCapture(window);
                                
                                // Set resize cursor
                                let resize_cursor = LoadCursorW(None, IDC_SIZEWE).unwrap_or_default();
                                SetCursor(resize_cursor);
                            }
                        } else {
                            // Check for column header click (for sorting)
                            if let Some(column_index) = state.get_column_at_x(x) {
                                let visible_columns = state.get_visible_columns();
                                if column_index < visible_columns.len() {
                                    let column_type = visible_columns[column_index].column_type;
                                    state.sort_by_column(column_type);
                                    
                                    // Update UI
                                    update_scrollbar(window);
                                    InvalidateRect(window, None, TRUE);
                                    update_status_bar();
                                }
                            }
                        }
                    } else {
                        // Normal item selection
                    if let Some(item_index) = state.get_item_at_point(x, y) {
                        state.set_selection(item_index);
                        InvalidateRect(window, None, TRUE);
                        update_status_bar();
                        }
                    }
                }
                LRESULT(0)
            }
            WM_LBUTTONUP => {
                if let Some(state) = &mut APP_STATE {
                    // End column resize if active
                    if let Some(ref drag_state) = state.column_drag_state {
                        if drag_state.is_dragging {
                            state.column_drag_state = None;
                            ReleaseCapture();
                            InvalidateRect(window, None, TRUE);
                        }
                    }
                }
                LRESULT(0)
            }
            WM_MOUSEMOVE => {
                if let Some(state) = &mut APP_STATE {
                    let x = (lparam.0 & 0xFFFF) as i16 as i32;
                    let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
                    
                    // Handle column resize dragging
                    let target_column_type = if let Some(ref drag_state) = state.column_drag_state {
                        if drag_state.is_dragging {
                            let visible_columns: Vec<&ColumnInfo> = state.get_visible_columns();
                            if drag_state.column_index < visible_columns.len() {
                                Some((visible_columns[drag_state.column_index].column_type, drag_state.start_x, drag_state.start_width))
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    };
                    
                    if let Some((column_type, start_x, start_width)) = target_column_type {
                        let delta_x = x - start_x;
                        let new_width = (start_width + delta_x).max(50); // Minimum width 50px
                        
                        // Update the width in the main columns array
                        for column in &mut state.columns {
                            if column.column_type == column_type {
                                column.width = new_width;
                                break;
                            }
                        }
                        
                        InvalidateRect(window, None, TRUE);
                        return LRESULT(0);
                    }
                    
                    // Show resize cursor when hovering over column boundaries
                    if state.view_mode == ViewMode::Details && y < HEADER_HEIGHT {
                        if state.get_column_resize_cursor_x(x).is_some() {
                            let resize_cursor = LoadCursorW(None, IDC_SIZEWE).unwrap_or_default();
                            SetCursor(resize_cursor);
                        } else {
                            let arrow_cursor = LoadCursorW(None, IDC_ARROW).unwrap_or_default();
                            SetCursor(arrow_cursor);
                        }
                    }
                }
                LRESULT(0)
            }
            WM_LBUTTONDBLCLK => {
                if let Some(state) = &mut APP_STATE {
                    let x = (lparam.0 & 0xFFFF) as i16 as i32;
                    let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
                    
                    if let Some(item_index) = state.get_item_at_point(x, y) {
                        state.set_selection(item_index);
                        state.open_selected_file();
                        InvalidateRect(window, None, TRUE);
                        update_status_bar();
                    }
                }
                LRESULT(0)
            }
            WM_RBUTTONUP => {
                let x = (lparam.0 & 0xFFFF) as i16 as i32;
                let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
                
                // Convert to screen coordinates
                let mut pt = POINT { x, y };
                ClientToScreen(window, &mut pt);
                
                // Check if we clicked on a file
                if let Some(state) = &mut APP_STATE {
                    if let Some(item_index) = state.get_item_at_point(x, y) {
                        // Right-clicked on a file - show file context menu
                        state.set_selection(item_index);
                        InvalidateRect(window, None, TRUE);
                        update_status_bar();
                        show_file_context_menu(GetParent(window), pt.x, pt.y, &state.list_data[item_index]);
                    } else {
                        // Right-clicked on empty space - show view context menu
                show_context_menu(GetParent(window), pt.x, pt.y);
                    }
                }
                LRESULT(0)
            }
            WM_KEYDOWN => {
                if let Some(state) = &mut APP_STATE {
                    let old_selected = state.selected_index;
                    
                    match wparam.0 as u32 {
                        0x26 => state.move_selection(-1),      // VK_UP
                        0x28 => state.move_selection(1),       // VK_DOWN
                        0x21 => { // VK_PRIOR (Page Up)
                            let page_size = match state.view_mode {
                                ViewMode::Details => state.client_height / state.item_height,
                                _ => state.grid_cols * (state.client_height / state.cell_size),
                            };
                            state.move_selection(-(page_size.max(1)));
                        }
                        0x22 => { // VK_NEXT (Page Down)
                            let page_size = match state.view_mode {
                                ViewMode::Details => state.client_height / state.item_height,
                                _ => state.grid_cols * (state.client_height / state.cell_size),
                            };
                            state.move_selection(page_size.max(1));
                        }
                        0x24 => { // VK_HOME
                            if !state.list_data.is_empty() {
                                state.set_selection(0);
                            }
                        }
                        0x23 => { // VK_END
                            if !state.list_data.is_empty() {
                                state.set_selection(state.list_data.len() - 1);
                            }
                        }
                        0x0D => { // VK_RETURN
                            state.open_selected_file();
                        }
                        _ => return DefWindowProcW(window, message, wparam, lparam),
                    }
                    
                    if state.selected_index != old_selected {
                        update_scrollbar(window);
                        InvalidateRect(window, None, TRUE);
                        update_status_bar();
                    }
                }
                LRESULT(0)
            }
            WM_VSCROLL => {
                let request = (wparam.0 & 0xFFFF) as u16;
                let pos = ((wparam.0 >> 16) & 0xFFFF) as i16;
                handle_vertical_scroll(window, request, pos);
                LRESULT(0)
            }
            WM_MOUSEWHEEL => {
                let delta = ((wparam.0 >> 16) & 0xFFFF) as i16;
                let delta = delta / 120; // WHEEL_DELTA
                
                // Check if Ctrl key is pressed
                let ctrl_pressed = GetKeyState(VK_CONTROL.0 as i32) < 0;
                
                if ctrl_pressed {
                    // Ctrl+Scroll: Adjust zoom level (15 levels: 0-14)
                    if let Some(state) = &mut APP_STATE {
                        let current_zoom = state.zoom_level;
                        let new_zoom = if delta > 0 {
                            // Scroll up: increase zoom level (larger icons)
                            (current_zoom + 1).min(14)
                        } else {
                            // Scroll down: decrease zoom level (smaller icons)
                            (current_zoom - 1).max(0)
                        };
                        
                        if new_zoom != current_zoom {
                            state.set_zoom_level(new_zoom);
                            update_scrollbar(window);
                            InvalidateRect(window, None, TRUE);
                            
                            // Log zoom level for debugging
                            let icon_size = AppState::get_icon_size_from_zoom_level(new_zoom);
                            if new_zoom == 0 {
                                println!("Zoom level: {} (Details view)", new_zoom);
                            } else {
                                println!("Zoom level: {} ({}px icons)", new_zoom, icon_size);
                            }
                        }
                    }
                } else {
                    // Normal scroll: scroll the list
                scroll_list(window, -delta as i32 * 3);
                }
                LRESULT(0)
            }
            WM_SETFOCUS => {
                InvalidateRect(window, None, TRUE);
                LRESULT(0)
            }
            WM_KILLFOCUS => {
                InvalidateRect(window, None, TRUE);
                LRESULT(0)
            }
            _ if message == WM_THUMBNAIL_READY => {
                // Handle thumbnail completion
                if let Some(state) = &mut APP_STATE {
                    let item_index = wparam.0;
                    let hbitmap = HBITMAP(lparam.0 as isize);
                    
                    if let Some(item) = state.list_data.get(item_index) {
                        let cache_key = (item.path.clone(), state.selected_view_size);
                        state.thumbnail_cache.put(cache_key, hbitmap);
                        
                        // Invalidate only the specific item's area
                        let item_rect = get_item_rect(item_index, state);
                        if let Some(rect) = item_rect {
                            InvalidateRect(window, Some(&rect), FALSE);
                        }
                    }
                }
                LRESULT(0)
            }
            _ => DefWindowProcW(window, message, wparam, lparam),
        }
    }
}

fn get_item_rect(item_index: usize, state: &AppState) -> Option<RECT> {
    match state.view_mode {
        ViewMode::Details => {
            let y = item_index as i32 * state.item_height - state.scroll_pos;
            if y >= -state.item_height && y < state.client_height + state.item_height {
                Some(RECT {
                    left: 0,
                    top: y,
                    right: state.client_width,
                    bottom: y + state.item_height,
                })
            } else {
                None
            }
        }
        _ => {
            // Grid mode
            if state.grid_cols <= 0 {
                return None;
            }
            
            let row = item_index as i32 / state.grid_cols;
            let col = item_index as i32 % state.grid_cols;
            let x = col * state.cell_size;
            let y = row * state.cell_size - state.scroll_pos;
            
            if y >= -state.cell_size && y < state.client_height + state.cell_size {
                Some(RECT {
                    left: x,
                    top: y,
                    right: x + state.cell_size,
                    bottom: y + state.cell_size,
                })
            } else {
                None
            }
        }
    }
}

fn paint_list_view(window: HWND) {
    log_debug("paint_list_view called");
    
    unsafe {
        let mut ps = PAINTSTRUCT::default();
        log_debug("About to call BeginPaint");
        let hdc = BeginPaint(window, &mut ps);
        log_debug("BeginPaint completed");
        
        if let Some(state) = &APP_STATE {
            log_debug(&format!("APP_STATE available for painting, list_data size: {}", state.list_data.len()));
            
            let mem_dc = CreateCompatibleDC(hdc);
            let mut rect = RECT::default();
            let _ = GetClientRect(window, &mut rect);
            
            log_debug("Created memory DC and got client rect");
            
            let bitmap = CreateCompatibleBitmap(hdc, rect.right - rect.left, rect.bottom - rect.top);
            let old_bitmap = SelectObject(mem_dc, bitmap);
            
            log_debug("Created compatible bitmap");
            
            let bg_brush = CreateSolidBrush(COLORREF(0x00FFFFFF));
            FillRect(mem_dc, &rect, bg_brush);
            DeleteObject(bg_brush);
            
            SetBkMode(mem_dc, TRANSPARENT);
            SelectObject(mem_dc, state.font);
            
            let has_focus = GetFocus() == window;
            
            log_debug(&format!("About to paint view mode: {:?}", state.view_mode));
            
            match state.view_mode {
                ViewMode::Details => {
                    log_debug("Calling paint_details_view");
                    paint_details_view(mem_dc, &rect, state, has_focus);
                    log_debug("paint_details_view completed");
                }
                _ => {
                    log_debug("Calling paint_icon_view");
                    paint_icon_view(mem_dc, &rect, state, has_focus);
                    log_debug("paint_icon_view completed");
                }
            }
            
            log_debug("About to BitBlt to screen");
            let _ = BitBlt(
                hdc,
                0, 0,
                rect.right - rect.left,
                rect.bottom - rect.top,
                mem_dc,
                0, 0,
                SRCCOPY,
            );
            log_debug("BitBlt completed");
            
            SelectObject(mem_dc, old_bitmap);
            DeleteObject(bitmap);
            DeleteDC(mem_dc);
            log_debug("Cleaned up GDI objects");
        } else {
            log_debug("ERROR: APP_STATE is None during painting");
        }
        
        log_debug("About to call EndPaint");
        EndPaint(window, &ps);
        log_debug("paint_list_view completed successfully");
    }
}

fn paint_details_view(hdc: HDC, client_rect: &RECT, state: &AppState, has_focus: bool) {
    unsafe {
        let visible_columns = state.get_visible_columns();
        if visible_columns.is_empty() {
            return;
        }
        
        // Constants for icon display
        const ICON_SIZE: i32 = 16;
        const ICON_MARGIN: i32 = 2;
        const TEXT_OFFSET: i32 = ICON_SIZE + ICON_MARGIN * 2;
        
        // Draw header bar
        let header_rect = RECT {
            left: 0,
            top: 0,
            right: client_rect.right,
            bottom: HEADER_HEIGHT,
        };
        
        // Header background
        let header_brush = CreateSolidBrush(COLORREF(0x00E0E0E0)); // Light gray
        FillRect(hdc, &header_rect, header_brush);
        DeleteObject(header_brush);
        
        // Header border
        let border_pen = CreatePen(PS_SOLID, 1, COLORREF(0x00C0C0C0));
        let old_pen = SelectObject(hdc, border_pen);
        MoveToEx(hdc, 0, HEADER_HEIGHT - 1, None);
        LineTo(hdc, client_rect.right, HEADER_HEIGHT - 1);
        
        // Draw column headers and separators
        let mut current_x = 0;
        for (index, column) in visible_columns.iter().enumerate() {
            // Column separator (except for first column)
            if index > 0 {
                MoveToEx(hdc, current_x, 0, None);
                LineTo(hdc, current_x, HEADER_HEIGHT);
            }
            
            // Header text
            SetTextColor(hdc, COLORREF(0x00000000));
            SetBkMode(hdc, TRANSPARENT);
            
            let header_text_with_sort = {
                let base_text = column.column_type.display_name();
                
                // Add sort indicator if this column is sorted
                if let Some(ref sort_state) = state.sort_state {
                    if sort_state.column == column.column_type {
                        match sort_state.order {
                            SortOrder::Ascending => format!("{} ↑", base_text),
                            SortOrder::Descending => format!("{} ↓", base_text),
                            SortOrder::None => base_text.to_string(),
                        }
                    } else {
                        base_text.to_string()
                    }
                } else {
                    base_text.to_string()
                }
            };
            
            let header_text: Vec<u16> = header_text_with_sort.encode_utf16().collect();
            // For the name column, offset text to account for icon space
            let text_x = if index == 0 && visible_columns[0].column_type == ColumnType::Name {
                current_x + TEXT_OFFSET + 5
            } else {
                current_x + 5
            };
            TextOutW(hdc, text_x, 5, &header_text);
            
            current_x += column.width;
        }
        
        SelectObject(hdc, old_pen);
        DeleteObject(border_pen);
        
        // Calculate item painting area (below header)
        let content_top = HEADER_HEIGHT;
        let base_start_y = content_top - (state.scroll_pos % state.item_height);
        
        // Ensure we start at or below the header
        let mut start_y = base_start_y;
        let mut first_item_offset = 0;
        
        // If start_y would place items above header, adjust
        if start_y < content_top {
            // Calculate how many items we need to skip to get below header
            let items_above_header = ((content_top - start_y + state.item_height - 1) / state.item_height) as usize;
            first_item_offset = items_above_header;
            start_y = base_start_y + (items_above_header as i32 * state.item_height);
        }
        
        for i in 0..state.visible_count {
            let item_index = state.visible_start + i + first_item_offset;
            if item_index >= state.list_data.len() {
                break;
            }
            
            let item = &state.list_data[item_index];
            let y = start_y + (i as i32 * state.item_height);
            
            // Double-check: ensure this item is not drawn above the header
            if y < content_top {
                continue;
            }
            
            // Stop drawing if we're below the visible area
            if y >= client_rect.bottom {
                break;
            }
            
            let item_rect = RECT {
                left: 0,
                top: y,
                right: client_rect.right,
                bottom: y + state.item_height,
            };
            
            // Draw selection highlight
            if Some(item_index) == state.selected_index {
                let selection_color = if has_focus {
                    COLORREF(0x00316AC5) // Blue selection when focused
                } else {
                    COLORREF(0x00C0C0C0) // Gray selection when not focused
                };
                let selection_brush = CreateSolidBrush(selection_color);
                FillRect(hdc, &item_rect, selection_brush);
                DeleteObject(selection_brush);
                
                SetTextColor(hdc, if has_focus { COLORREF(0x00FFFFFF) } else { COLORREF(0x00000000) });
            } else if item_index % 2 == 1 {
                // Alternate row colors for non-selected items
                let alt_brush = CreateSolidBrush(COLORREF(0x00F8F8F8));
                FillRect(hdc, &item_rect, alt_brush);
                DeleteObject(alt_brush);
                SetTextColor(hdc, COLORREF(0x00000000));
            } else {
                SetTextColor(hdc, COLORREF(0x00000000));
            }
            
            // Draw column data
            let mut current_x = 0;
            for (col_index, column) in visible_columns.iter().enumerate() {
                let text = match column.column_type {
                    ColumnType::Name => item.name.clone(),
                    ColumnType::Size => {
                        // Load metadata on demand for visible items
                        let mut item_clone = item.clone();
                        if item_clone.size == 0 && item_clone.modified_time == std::time::UNIX_EPOCH {
                            item_clone.load_metadata();
                        }
                        item_clone.format_size()
                    },
                    ColumnType::Type => item.file_type.clone(),
                    ColumnType::Modified => {
                        // Load metadata on demand for visible items
                        let mut item_clone = item.clone();
                        if item_clone.size == 0 && item_clone.modified_time == std::time::UNIX_EPOCH {
                            item_clone.load_metadata();
                        }
                        item_clone.format_modified_time()
                    },
                    ColumnType::Path => item.path.clone(),
                };
                
                // For the first column (Name), draw icon and adjust text position
                if col_index == 0 && column.column_type == ColumnType::Name {
                    // Get and draw file icon
                    if let Some(icon) = get_file_icon(&item.path, true) { // true for small icon
                        let icon_x = current_x + ICON_MARGIN;
                        let icon_y = y + (state.item_height - ICON_SIZE) / 2; // Center vertically
                        draw_icon(hdc, icon, icon_x, icon_y, ICON_SIZE);
                    } else if let Some(default_icon) = get_default_file_icon(true) {
                        // Fallback to default file icon
                        let icon_x = current_x + ICON_MARGIN;
                        let icon_y = y + (state.item_height - ICON_SIZE) / 2;
                        draw_icon(hdc, default_icon, icon_x, icon_y, ICON_SIZE);
                    }
                    
                    // Create clipping rect for text (offset by icon space)
                    let column_rect = RECT {
                        left: current_x + TEXT_OFFSET + 2,
                        top: y,
                        right: current_x + column.width - 2,
                        bottom: y + state.item_height,
                    };
                    
                    // Draw text with clipping and ellipsis
                    if !text.is_empty() {
                        let mut text_utf16: Vec<u16> = text.encode_utf16().collect();
                        let mut text_rect = column_rect;
                        DrawTextW(hdc, &mut text_utf16, &mut text_rect, DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS);
                    }
                } else {
                    // For other columns, normal text rendering
                    let column_rect = RECT {
                        left: current_x + 2,
                        top: y,
                        right: current_x + column.width - 2, // Leave 2px margin on each side
                        bottom: y + state.item_height,
                    };
                    
                    // Draw text with clipping and ellipsis
                    if !text.is_empty() {
                        let mut text_utf16: Vec<u16> = text.encode_utf16().collect();
                        let mut text_rect = column_rect;
                        DrawTextW(hdc, &mut text_utf16, &mut text_rect, DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS);
                    }
                }
                
                current_x += column.width;
            }
        }
    }
}

fn paint_icon_view(hdc: HDC, client_rect: &RECT, state: &AppState, has_focus: bool) {
    unsafe {
        if state.grid_cols <= 0 || state.cell_size <= 0 {
            return;
        }
        
        let first_visible_row = state.scroll_pos / state.cell_size;
        let visible_rows = (state.client_height / state.cell_size) + 2;
        
        for row in first_visible_row..first_visible_row + visible_rows {
            for col in 0..state.grid_cols {
                let item_index = (row * state.grid_cols + col) as usize;
                if item_index >= state.list_data.len() {
                    break;
                }
                
                let item = &state.list_data[item_index];
                let x = col * state.cell_size;
                let y = row * state.cell_size - state.scroll_pos;
                
                // Skip if completely outside visible area
                if y + state.cell_size < 0 || y > state.client_height {
                    continue;
                }
                
                let cell_rect = RECT {
                    left: x,
                    top: y,
                    right: x + state.cell_size,
                    bottom: y + state.cell_size,
                };
                
                // Draw selection highlight
                if Some(item_index) == state.selected_index {
                    let selection_color = if has_focus {
                        COLORREF(0x00316AC5)
                    } else {
                        COLORREF(0x00C0C0C0)
                    };
                    let selection_brush = CreateSolidBrush(selection_color);
                    FillRect(hdc, &cell_rect, selection_brush);
                    DeleteObject(selection_brush);
                }
                
                // Draw thumbnail or placeholder
                let thumbnail_size = state.selected_view_size;
                let thumbnail_x = x + (state.cell_size - thumbnail_size as i32) / 2;
                let thumbnail_y = y + 4;
                
                let cache_key = (item.path.clone(), thumbnail_size);
                if let Some(&cached_bitmap) = state.thumbnail_cache.peek(&cache_key) {
                    // Draw cached thumbnail
                    draw_bitmap(hdc, cached_bitmap, thumbnail_x, thumbnail_y, thumbnail_size as i32);
                } else {
                    // Draw placeholder - thumbnail will be requested by background system
                    let placeholder = create_placeholder_bitmap(thumbnail_size);
                    draw_bitmap(hdc, placeholder, thumbnail_x, thumbnail_y, thumbnail_size as i32);
                    DeleteObject(placeholder);
                }
                
                // Draw filename below thumbnail
                let text_y = thumbnail_y + thumbnail_size as i32 + 4;
                let text_rect = RECT {
                    left: x + 2,
                    top: text_y,
                    right: x + state.cell_size - 2,
                    bottom: y + state.cell_size - 2,
                };
                
                SetTextColor(hdc, if Some(item_index) == state.selected_index && has_focus {
                    COLORREF(0x00FFFFFF)
                } else {
                    COLORREF(0x00000000)
                });
                
                let mut name_utf16: Vec<u16> = item.name.encode_utf16().collect();
                let mut text_rect = text_rect;
                DrawTextW(hdc, &mut name_utf16, &mut text_rect, DT_CENTER | DT_WORDBREAK | DT_END_ELLIPSIS);
            }
        }
    }
}

fn draw_bitmap(hdc: HDC, bitmap: HBITMAP, x: i32, y: i32, size: i32) {
    unsafe {
        let bitmap_dc = CreateCompatibleDC(hdc);
        let old_bitmap = SelectObject(bitmap_dc, bitmap);
        
        let _ = BitBlt(hdc, x, y, size, size, bitmap_dc, 0, 0, SRCCOPY);
        
        SelectObject(bitmap_dc, old_bitmap);
        DeleteDC(bitmap_dc);
    }
}

fn update_scrollbar(window: HWND) {
    unsafe {
        log_debug("update_scrollbar called");
        
        if let Some(state) = &APP_STATE {
            log_debug(&format!("Setting scrollbar info: total_height={}, client_height={}, scroll_pos={}", 
                state.total_height, state.client_height, state.scroll_pos));
            
            // Calculate the maximum scroll position
            let max_scroll = (state.total_height - state.client_height).max(0);
            
            // Use a fixed scrollbar range (0-10000) for better Windows compatibility
            const SCROLLBAR_RANGE: i32 = 10000;
            let scrollbar_pos = if max_scroll > 0 {
                ((state.scroll_pos as f64 / max_scroll as f64) * SCROLLBAR_RANGE as f64) as i32
            } else {
                0
            };
            
            let scrollbar_page = if max_scroll > 0 {
                ((state.client_height as f64 / state.total_height as f64) * SCROLLBAR_RANGE as f64) as u32
            } else {
                SCROLLBAR_RANGE as u32
            };
            
            log_debug(&format!("Scrollbar mapping: actual_pos={}, scrollbar_pos={}, max_scroll={}, scrollbar_page={}", 
                state.scroll_pos, scrollbar_pos, max_scroll, scrollbar_page));
            
            let si = SCROLLINFO {
                cbSize: std::mem::size_of::<SCROLLINFO>() as u32,
                fMask: SIF_RANGE | SIF_PAGE | SIF_POS,
                nMin: 0,
                nMax: SCROLLBAR_RANGE,
                nPage: scrollbar_page.max(1),
                nPos: scrollbar_pos.max(0).min(SCROLLBAR_RANGE),
                nTrackPos: 0,
            };
            
            SetScrollInfo(window, SB_VERT, &si, TRUE);
            log_debug(&format!("Scrollbar updated: nMax={}, nPage={}, nPos={}", si.nMax, si.nPage, si.nPos));
        } else {
            log_debug("WARNING: update_scrollbar called but APP_STATE is None");
        }
    }
}

fn handle_vertical_scroll(window: HWND, request: u16, pos: i16) {
    unsafe {
        if let Some(state) = &mut APP_STATE {
            log_debug(&format!("handle_vertical_scroll called: request={}, pos={}, current_scroll_pos={}", 
                request, pos, state.scroll_pos));
                
            let old_pos = state.scroll_pos;
            let scroll_unit = match state.view_mode {
                ViewMode::Details => state.item_height,
                _ => state.cell_size,
            };
            
            match request {
                0 => {
                    log_debug("SB_LINEUP");
                    state.scroll_pos -= scroll_unit;
                }
                1 => {
                    log_debug("SB_LINEDOWN");
                    state.scroll_pos += scroll_unit;
                }
                2 => {
                    log_debug("SB_PAGEUP");
                    state.scroll_pos -= state.client_height;
                }
                3 => {
                    log_debug("SB_PAGEDOWN");
                    state.scroll_pos += state.client_height;
                }
                4 => { // SB_THUMBTRACK - user is dragging
                    // Check for Windows scrollbar position overflow (16-bit signed integer overflow)
                    if pos < 0 {
                        log_debug(&format!("SB_THUMBTRACK: ignoring negative position {} (16-bit overflow), keeping current position {}", 
                            pos, state.scroll_pos));
                        // Keep current position, don't update
                    } else {
                        log_debug(&format!("SB_THUMBTRACK: setting is_scrollbar_dragging=true, converting scrollbar_pos {} to actual position", pos));
                        state.is_scrollbar_dragging = true;
                        
                        // Convert scrollbar position to actual scroll position
                        const SCROLLBAR_RANGE: i32 = 10000;
                        let max_scroll = (state.total_height - state.client_height).max(0);
                        let actual_pos = if max_scroll > 0 && SCROLLBAR_RANGE > 0 {
                            ((pos as f64 / SCROLLBAR_RANGE as f64) * max_scroll as f64) as i32
                        } else {
                            0
                        };
                        
                        log_debug(&format!("SB_THUMBTRACK: scrollbar_pos={}, actual_pos={}, max_scroll={}", pos, actual_pos, max_scroll));
                        state.scroll_pos = actual_pos;
                    }
                }
                5 => { // SB_THUMBPOSITION - user released drag
                    // Check for Windows scrollbar position overflow (16-bit signed integer overflow)
                    if pos < 0 {
                        log_debug(&format!("SB_THUMBPOSITION: ignoring negative position {} (16-bit overflow), keeping current position {}", 
                            pos, state.scroll_pos));
                        // Keep current position, just set dragging to false
                        state.is_scrollbar_dragging = false;
                    } else {
                        log_debug(&format!("SB_THUMBPOSITION: setting is_scrollbar_dragging=false, converting scrollbar_pos {} to actual position", pos));
                        state.is_scrollbar_dragging = false;
                        
                        // Convert scrollbar position to actual scroll position
                        const SCROLLBAR_RANGE: i32 = 10000;
                        let max_scroll = (state.total_height - state.client_height).max(0);
                        let actual_pos = if max_scroll > 0 && SCROLLBAR_RANGE > 0 {
                            ((pos as f64 / SCROLLBAR_RANGE as f64) * max_scroll as f64) as i32
                        } else {
                            0
                        };
                        
                        log_debug(&format!("SB_THUMBPOSITION: scrollbar_pos={}, actual_pos={}, max_scroll={}", pos, actual_pos, max_scroll));
                        state.scroll_pos = actual_pos;
                    }
                }
                6 => {
                    log_debug("SB_TOP");
                    state.scroll_pos = 0;
                }
                7 => {
                    log_debug("SB_BOTTOM");
                    state.scroll_pos = state.total_height - state.client_height;
                }
                8 => {
                    log_debug("SB_ENDSCROLL: setting is_scrollbar_dragging=false");
                    // SB_ENDSCROLL - dragging ended, update scrollbar to synchronize
                    state.is_scrollbar_dragging = false;
                    update_scrollbar(window);
                    return;
                }
                _ => {
                    log_debug(&format!("Unknown scroll request: {}", request));
                    return;
                }
            }
            
            state.scroll_pos = state.scroll_pos.max(0).min(state.total_height - state.client_height);
            log_debug(&format!("Clamped scroll_pos to: {}", state.scroll_pos));
            
            if state.scroll_pos != old_pos {
                log_debug(&format!("Scroll position changed from {} to {}", old_pos, state.scroll_pos));
                
                // Only do minimal updates during dragging
                if state.is_scrollbar_dragging {
                    log_debug("During dragging: minimal update (no scrollbar updates, no thumbnails)");
                    // During drag: only update visible range, no scrollbar updates, no thumbnails
                    state.calculate_layout();
                    InvalidateRect(window, None, TRUE);
                } else {
                    log_debug("Normal scrolling: full update");
                    // Normal scrolling: full update
                state.calculate_layout();
                update_scrollbar(window);
                InvalidateRect(window, None, TRUE);
                
                // Post message to recompute thumbnails
                let _ = PostMessageW(GetParent(window), WM_RECOMPUTE_THUMBS, WPARAM(0), LPARAM(0));
            }
            } else {
                log_debug("No scroll position change detected");
            }
            
            log_debug(&format!("handle_vertical_scroll completed: final_scroll_pos={}, is_dragging={}", 
                state.scroll_pos, state.is_scrollbar_dragging));
        } else {
            log_debug("ERROR: handle_vertical_scroll called but APP_STATE is None");
        }
    }
}

fn scroll_list(window: HWND, lines: i32) {
    unsafe {
        if let Some(state) = &mut APP_STATE {
            let old_pos = state.scroll_pos;
            let scroll_unit = match state.view_mode {
                ViewMode::Details => state.item_height,
                _ => state.cell_size,
            };
            
            state.scroll_pos += lines * scroll_unit;
            state.scroll_pos = state.scroll_pos.max(0).min(state.total_height - state.client_height);
            
            if state.scroll_pos != old_pos {
                state.calculate_layout();
                update_scrollbar(window);
                InvalidateRect(window, None, TRUE);
                
                // Post message to recompute thumbnails
                let _ = PostMessageW(GetParent(window), WM_RECOMPUTE_THUMBS, WPARAM(0), LPARAM(0));
            }
        }
    }
} 

extern "system" fn search_edit_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        match message {
            WM_KEYDOWN => {
                if wparam.0 == 0x0D { // VK_RETURN (Enter key)
                    log_debug("Enter key pressed in search edit - triggering immediate search");
                    handle_immediate_search();
                    return LRESULT(0);
                }
            }
            _ => {}
        }
        
        // Call original window procedure for all other messages
        if let Some(original_proc) = ORIGINAL_SEARCH_EDIT_PROC {
            CallWindowProcW(original_proc, window, message, wparam, lparam)
        } else {
            DefWindowProcW(window, message, wparam, lparam)
        }
    }
} 

extern "system" fn main_window_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        match message {
            WM_CREATE => {
                if let Some(state) = &mut APP_STATE {
                    state.main_window = window;
                    
                    state.font = CreateFontW(
                        16, 0, 0, 0,
                        400,  // FW_NORMAL
                        0, 0, 0,
                        1,    // DEFAULT_CHARSET
                        0,    // OUT_DEFAULT_PRECIS
                        0,    // CLIP_DEFAULT_PRECIS
                        0,    // DEFAULT_QUALITY
                        0,    // DEFAULT_PITCH | FF_DONTCARE
                        w!("Segoe UI"),
                    );
                    
                    create_child_controls(window);
                    let _ = create_menus(window);
                    state.initialize_everything_sdk();
                    state.initialize_thumbnail_task_manager(state.list_view);
                    update_status_bar();
                }
                LRESULT(0)
            }
            WM_SIZE => {
                let width = (lparam.0 & 0xFFFF) as i32;
                let height = ((lparam.0 >> 16) & 0xFFFF) as i32;
                resize_controls(width, height);
                
                // Post message to recompute thumbnails
                let _ = PostMessageW(window, WM_RECOMPUTE_THUMBS, WPARAM(0), LPARAM(0));
                LRESULT(0)
            }
            WM_COMMAND => {
                let control_id = (wparam.0 & 0xFFFF) as i32;
                let notification = ((wparam.0 >> 16) & 0xFFFF) as u16;
                
                match control_id {
                    ID_SEARCH_EDIT => {
                        if notification == 0x0300 { // EN_CHANGE
                            handle_search_change();
                        }
                    }
                    ID_VIEW_DETAILS => {
                        if let Some(state) = &mut APP_STATE {
                            state.set_view_mode(ViewMode::Details);
                            update_scrollbar(state.list_view);
                            InvalidateRect(state.list_view, None, TRUE);
                        }
                    }
                    ID_VIEW_MEDIUM_ICONS => {
                        if let Some(state) = &mut APP_STATE {
                            state.set_view_mode(ViewMode::MediumIcons);
                            update_scrollbar(state.list_view);
                            InvalidateRect(state.list_view, None, TRUE);
                        }
                    }
                    ID_VIEW_LARGE_ICONS => {
                        if let Some(state) = &mut APP_STATE {
                            state.set_view_mode(ViewMode::LargeIcons);
                            update_scrollbar(state.list_view);
                            InvalidateRect(state.list_view, None, TRUE);
                        }
                    }
                    ID_VIEW_EXTRALARGE_ICONS => {
                        if let Some(state) = &mut APP_STATE {
                            state.set_view_mode(ViewMode::ExtraLargeIcons);
                            update_scrollbar(state.list_view);
                            InvalidateRect(state.list_view, None, TRUE);
                        }
                    }
                    ID_SORT_NAME => {
                        if let Some(state) = &mut APP_STATE {
                            state.sort_by_column(ColumnType::Name);
                            update_scrollbar(state.list_view);
                            InvalidateRect(state.list_view, None, TRUE);
                            update_status_bar();
                            update_sort_menu_checkmarks(window, &state.sort_state);
                        }
                    }
                    ID_SORT_SIZE => {
                        if let Some(state) = &mut APP_STATE {
                            state.sort_by_column(ColumnType::Size);
                            update_scrollbar(state.list_view);
                            InvalidateRect(state.list_view, None, TRUE);
                            update_status_bar();
                            update_sort_menu_checkmarks(window, &state.sort_state);
                        }
                    }
                    ID_SORT_TYPE => {
                        if let Some(state) = &mut APP_STATE {
                            state.sort_by_column(ColumnType::Type);
                            update_scrollbar(state.list_view);
                            InvalidateRect(state.list_view, None, TRUE);
                            update_status_bar();
                            update_sort_menu_checkmarks(window, &state.sort_state);
                        }
                    }
                    ID_SORT_DATE => {
                        if let Some(state) = &mut APP_STATE {
                            state.sort_by_column(ColumnType::Modified);
                            update_scrollbar(state.list_view);
                            InvalidateRect(state.list_view, None, TRUE);
                            update_status_bar();
                            update_sort_menu_checkmarks(window, &state.sort_state);
                        }
                    }
                    ID_SORT_PATH => {
                        if let Some(state) = &mut APP_STATE {
                            state.sort_by_column(ColumnType::Path);
                            update_scrollbar(state.list_view);
                            InvalidateRect(state.list_view, None, TRUE);
                            update_status_bar();
                            update_sort_menu_checkmarks(window, &state.sort_state);
                        }
                    }
                    ID_SORT_ASCENDING => {
                        if let Some(state) = &mut APP_STATE {
                            state.change_sort_order(SortOrder::Ascending);
                            update_scrollbar(state.list_view);
                            InvalidateRect(state.list_view, None, TRUE);
                            update_status_bar();
                            update_sort_menu_checkmarks(window, &state.sort_state);
                        }
                    }
                    ID_SORT_DESCENDING => {
                        if let Some(state) = &mut APP_STATE {
                            state.change_sort_order(SortOrder::Descending);
                            update_scrollbar(state.list_view);
                            InvalidateRect(state.list_view, None, TRUE);
                            update_status_bar();
                            update_sort_menu_checkmarks(window, &state.sort_state);
                        }
                    }
                    ID_FILE_OPEN_LIST => {
                        // Show file dialog to select file list
                        if let Some(file_path) = show_open_file_dialog(window) {
                            if let Some(state) = &mut APP_STATE {
                                match state.load_file_list(&file_path) {
                                    Ok(_) => {
                                        update_scrollbar(state.list_view);
                                        InvalidateRect(state.list_view, None, TRUE);
                                        update_status_bar();
                                        println!("Successfully loaded file list: {}", file_path);
                                    }
                                    Err(e) => {
                                        let message = format!("Failed to load file list: {}", e);
                                        let message_wide: Vec<u16> = message.encode_utf16().chain(std::iter::once(0)).collect();
                                        let title_wide: Vec<u16> = "Error".encode_utf16().chain(std::iter::once(0)).collect();
                                        
                                        MessageBoxW(
                                            window,
                                            PCWSTR::from_raw(message_wide.as_ptr()),
                                            PCWSTR::from_raw(title_wide.as_ptr()),
                                            MB_ICONERROR | MB_OK,
                                        );
                                    }
                                }
                            }
                        }
                    }
                    ID_FILE_SAVE_LIST => {
                        // Show save dialog with default filename
                        if let Some(save_path) = show_save_file_dialog(window, "file_list.csv") {
                            if let Some(state) = &APP_STATE {
                                match state.save_file_list(&save_path) {
                                    Ok(_) => {
                                        let message = format!("File list saved to: {}", save_path);
                                        let message_wide: Vec<u16> = message.encode_utf16().chain(std::iter::once(0)).collect();
                                        let title_wide: Vec<u16> = "Success".encode_utf16().chain(std::iter::once(0)).collect();
                                        
                                        MessageBoxW(
                                            window,
                                            PCWSTR::from_raw(message_wide.as_ptr()),
                                            PCWSTR::from_raw(title_wide.as_ptr()),
                                            MB_ICONINFORMATION | MB_OK,
                                        );
                                    }
                                    Err(_) => {
                                        let message = "Failed to save file list".to_string();
                                        let message_wide: Vec<u16> = message.encode_utf16().chain(std::iter::once(0)).collect();
                                        let title_wide: Vec<u16> = "Error".encode_utf16().chain(std::iter::once(0)).collect();
                                        
                                        MessageBoxW(
                                            window,
                                            PCWSTR::from_raw(message_wide.as_ptr()),
                                            PCWSTR::from_raw(title_wide.as_ptr()),
                                            MB_ICONERROR | MB_OK,
                                        );
                                    }
                                }
                            }
                        }
                    }
                    ID_FILE_EXPORT_LIST => {
                        // Show save dialog for simple export
                        if let Some(export_path) = show_save_file_dialog(window, "simple_list.txt") {
                            if let Some(state) = &APP_STATE {
                                match state.export_simple_list(&export_path) {
                                    Ok(_) => {
                                        let message = format!("Simple file list exported to: {}", export_path);
                                        let message_wide: Vec<u16> = message.encode_utf16().chain(std::iter::once(0)).collect();
                                        let title_wide: Vec<u16> = "Success".encode_utf16().chain(std::iter::once(0)).collect();
                                        
                                        MessageBoxW(
                                            window,
                                            PCWSTR::from_raw(message_wide.as_ptr()),
                                            PCWSTR::from_raw(title_wide.as_ptr()),
                                            MB_ICONINFORMATION | MB_OK,
                                        );
                                    }
                                    Err(_) => {
                                        let message = "Failed to export file list".to_string();
                                        let message_wide: Vec<u16> = message.encode_utf16().chain(std::iter::once(0)).collect();
                                        let title_wide: Vec<u16> = "Error".encode_utf16().chain(std::iter::once(0)).collect();
                                        
                                        MessageBoxW(
                                            window,
                                            PCWSTR::from_raw(message_wide.as_ptr()),
                                            PCWSTR::from_raw(title_wide.as_ptr()),
                                            MB_ICONERROR | MB_OK,
                                        );
                                    }
                                }
                            }
                        }
                    }
                    ID_FILE_CLOSE_LIST => {
                        // Show confirmation dialog before closing the list
                        let strings = get_strings();
                        let result = MessageBoxW(
                            window,
                            PCWSTR::from_raw(to_wide(&strings.confirm_close_list).as_ptr()),
                            PCWSTR::from_raw(to_wide(&strings.confirm_title).as_ptr()),
                            MB_ICONQUESTION | MB_YESNO | MB_DEFBUTTON2,
                        );

                        if result == IDYES {
                            if let Some(state) = &mut APP_STATE {
                                state.close_file_list();
                            }
                        }
                    }
                    // Language menu items
                    ID_LANG_ENGLISH => {
                        if let Some(state) = &mut APP_STATE {
                            state.set_language(Language::English);
                        }
                    }
                    ID_LANG_CHINESE => {
                        if let Some(state) = &mut APP_STATE {
                            state.set_language(Language::Chinese);
                        }
                    }
                    // Thumbnail strategy options
                    ID_THUMB_DEFAULT => {
                        // Show warning for Mode A
                        let strings = get_strings();
                        let result = MessageBoxW(
                            window,
                            PCWSTR::from_raw(to_wide(&strings.warning_thumbnail_mode).as_ptr()),
                            PCWSTR::from_raw(to_wide(&strings.warning_title).as_ptr()),
                            MB_ICONEXCLAMATION | MB_YESNO | MB_DEFBUTTON2,
                        );
                        
                        if result == IDYES {
                            if let Some(state) = &mut APP_STATE {
                                state.set_thumbnail_strategy(ThumbnailStrategy::DefaultTopToBottom);
                            }
                        }
                        // If IDNO or user pressed Enter (default No), do nothing
                    }
                    ID_THUMB_VISIBLE => {
                        if let Some(state) = &mut APP_STATE {
                            state.set_thumbnail_strategy(ThumbnailStrategy::OnlyLoadVisible);
                        }
                    }
                    ID_THUMB_VISIBLE_PLUS_500 => {
                        if let Some(state) = &mut APP_STATE {
                            state.set_thumbnail_strategy(ThumbnailStrategy::LoadVisiblePlus500);
                        }
                    }
                    // Thumbnail background options
                    ID_BG_TRANSPARENT => {
                        if let Some(state) = &mut APP_STATE {
                            state.set_thumbnail_background(ThumbnailBackground::Transparent);
                        }
                    }
                    ID_BG_CHECKERBOARD => {
                        if let Some(state) = &mut APP_STATE {
                            state.set_thumbnail_background(ThumbnailBackground::Checkerboard);
                        }
                    }
                    ID_BG_BLACK => {
                        if let Some(state) = &mut APP_STATE {
                            state.set_thumbnail_background(ThumbnailBackground::Black);
                        }
                    }
                    ID_BG_WHITE => {
                        if let Some(state) = &mut APP_STATE {
                            state.set_thumbnail_background(ThumbnailBackground::White);
                        }
                    }
                    ID_BG_GRAY => {
                        if let Some(state) = &mut APP_STATE {
                            state.set_thumbnail_background(ThumbnailBackground::Gray);
                        }
                    }
                    ID_BG_LIGHT_GRAY => {
                        if let Some(state) = &mut APP_STATE {
                            state.set_thumbnail_background(ThumbnailBackground::LightGray);
                        }
                    }
                    ID_BG_DARK_GRAY => {
                        if let Some(state) = &mut APP_STATE {
                            state.set_thumbnail_background(ThumbnailBackground::DarkGray);
                        }
                    }
                    // Column visibility toggles
                    ID_COLUMN_NAME => {
                        if let Some(state) = &mut APP_STATE {
                            state.toggle_column(ColumnType::Name);
                        }
                    }
                    ID_COLUMN_SIZE => {
                        if let Some(state) = &mut APP_STATE {
                            state.toggle_column(ColumnType::Size);
                        }
                    }
                    ID_COLUMN_TYPE => {
                        if let Some(state) = &mut APP_STATE {
                            state.toggle_column(ColumnType::Type);
                        }
                    }
                    ID_COLUMN_MODIFIED => {
                        if let Some(state) = &mut APP_STATE {
                            state.toggle_column(ColumnType::Modified);
                        }
                    }
                    ID_COLUMN_PATH => {
                        if let Some(state) = &mut APP_STATE {
                            state.toggle_column(ColumnType::Path);
                        }
                    }
                    // Sort options
                    ID_SORT_ASCENDING => {
                        if let Some(state) = &mut APP_STATE {
                            state.change_sort_order(SortOrder::Ascending);
                        }
                    }
                    ID_SORT_DESCENDING => {
                        if let Some(state) = &mut APP_STATE {
                            state.change_sort_order(SortOrder::Descending);
                        }
                    }
                    _ => {}
                }
                LRESULT(0)
            }
            WM_SEARCH_RESULTS => {
                if let Some(state) = &mut APP_STATE {
                    log_debug("Received WM_SEARCH_RESULTS message");
                    let results_ptr = wparam.0 as isize;
                    log_debug("APP_STATE is available, calling handle_search_results");
                    state.handle_search_results(results_ptr);
                    log_debug("handle_search_results completed");
                } else {
                    log_debug("WARNING: WM_SEARCH_RESULTS received but APP_STATE is None");
                }
                LRESULT(0)
            }
            WM_TIMER => {
                let timer_id = wparam.0 as usize;
                log_debug(&format!("Received WM_TIMER message with ID: {}", timer_id));
                
                if timer_id == SEARCH_TIMER_ID {
                    log_debug("Search timer expired, executing delayed search");
                    if let Some(state) = &mut APP_STATE {
                        // Kill the timer
                        let _ = KillTimer(state.main_window, SEARCH_TIMER_ID as usize);
                        state.search_timer_active = false;
                        
                        // Get current text from search edit control
                        let mut buffer: [u16; 1024] = [0; 1024];
                        let len = GetWindowTextW(state.search_edit, &mut buffer);
                        
                        let search_text = if len > 0 {
                            String::from_utf16_lossy(&buffer[..len as usize])
                        } else {
                            String::new()
                        };
                        
                        log_debug(&format!("Executing delayed search for: '{}'", search_text));
                        state.start_async_search(search_text);
                    }
                }
                LRESULT(0)
            }
            WM_RECOMPUTE_THUMBS => {
                log_debug("Received WM_RECOMPUTE_THUMBS message");
                if let Some(state) = &APP_STATE {
                    log_debug("APP_STATE is available, checking if scrollbar is being dragged");
                    if !state.is_scrollbar_dragging {
                        log_debug("Not dragging, calling recompute_thumbnail_queue");
                        state.recompute_thumbnail_queue();
                        log_debug("recompute_thumbnail_queue completed");
                    } else {
                        log_debug("Currently dragging scrollbar, skipping thumbnail recomputation");
                    }
                } else {
                    log_debug("WARNING: WM_RECOMPUTE_THUMBS received but APP_STATE is None");
                }
                log_debug("WM_RECOMPUTE_THUMBS handler completed");
                LRESULT(0)
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                LRESULT(0)
            }
            _ => DefWindowProcW(window, message, wparam, lparam),
        }
    }
}

fn show_simple_file_input_dialog(_window: HWND, _title: &str) -> Option<String> {
    // For demonstration, return a default path
    Some("file_list.txt".to_string())
}

fn handle_immediate_search() {
    unsafe {
        if let Some(state) = &mut APP_STATE {
            log_debug("handle_immediate_search called");
            
            // Kill existing timer if active
            if state.search_timer_active {
                let _ = KillTimer(state.main_window, SEARCH_TIMER_ID as usize);
                state.search_timer_active = false;
                log_debug("Killed existing search timer for immediate search");
            }
            
            // Get current text from search edit control
            let mut buffer: [u16; 1024] = [0; 1024];
            let len = GetWindowTextW(state.search_edit, &mut buffer);
            
            let search_text = if len > 0 {
                String::from_utf16_lossy(&buffer[..len as usize])
            } else {
                String::new()
            };
            
            log_debug(&format!("Immediate search for: '{}'", search_text));
            
            // Start async search immediately
            state.start_async_search(search_text);
            
            log_debug("handle_immediate_search completed");
        } else {
            log_debug("WARNING: handle_immediate_search called but APP_STATE is None");
        }
    }
}

fn update_status_bar() {
    unsafe {
        log_debug("update_status_bar called");

        if let Some(state) = &APP_STATE {
            log_debug(&format!("Status bar update: {} items total", state.list_data.len()));
            let strings = get_strings();

            let status_text = if let Some(selected) = state.selected_index {
                if selected < state.list_data.len() {
                    let file = &state.list_data[selected];
                    let file_info = get_file_info(&file.path);

                    format!("{} {} | {}: {} {}",
                        state.list_data.len(),
                        strings.status_objects,
                        strings.status_selected,
                        file.name,
                        file_info
                    )
                } else {
                    format!("{} {}", state.list_data.len(), strings.status_objects)
                }
            } else {
                format!("{} {}", state.list_data.len(), strings.status_objects)
            };

            // Add list name if in list mode
            let final_status = if state.is_list_mode {
                if let Some(ref list_name) = state.current_list_name {
                    format!("{} | List: {}", status_text, list_name)
                } else {
                    format!("{} | List Mode", status_text)
                }
            } else {
                status_text
            };

            log_debug(&format!("Setting status text: '{}'", final_status));
            let status_utf16: Vec<u16> = final_status.encode_utf16().chain(std::iter::once(0)).collect();
            let _ = SetWindowTextW(state.status_bar, PCWSTR::from_raw(status_utf16.as_ptr()));
            log_debug("update_status_bar completed successfully");
        } else {
            log_debug("WARNING: update_status_bar called but APP_STATE is None");
        }
    }
}

fn get_file_info(path: &str) -> String {
    match fs::metadata(path) {
        Ok(metadata) => {
            let size = metadata.len();
            let size_str = if size > 1024 * 1024 * 1024 {
                format!("({:.1} GB)", size as f64 / (1024.0 * 1024.0 * 1024.0))
            } else if size > 1024 * 1024 {
                format!("({:.1} MB)", size as f64 / (1024.0 * 1024.0))
            } else if size > 1024 {
                format!("({:.1} KB)", size as f64 / 1024.0)
            } else {
                format!("({} bytes)", size)
            };
            size_str
        }
        Err(_) => String::new(),
    }
}

fn open_file(path: &str) {
    unsafe {
        let path_utf16: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
        let operation = w!("open");
        
        let result = ShellExecuteW(
            None,
            operation,
            PCWSTR::from_raw(path_utf16.as_ptr()),
            None,
            None,
            SW_SHOWNORMAL,
        );
        
        if result.0 <= 32 {
            println!("Failed to open file: {}", path);
        }
    }
}

fn show_file_context_menu(window: HWND, x: i32, y: i32, _file: &FileResult) {
    unsafe {
        let hmenu = CreatePopupMenu().unwrap();
        let strings = get_strings();
        
        let _ = AppendMenuW(hmenu, MF_STRING, ID_OPEN_FILE as usize, 
                           PCWSTR::from_raw(to_wide(&strings.ctx_open).as_ptr()));
        
        let _ = AppendMenuW(hmenu, MF_SEPARATOR, 0, PCWSTR::null());
        
        let _ = AppendMenuW(hmenu, MF_STRING, ID_OPEN_FILE_LOCATION as usize, 
                           PCWSTR::from_raw(to_wide(&strings.ctx_open_location).as_ptr()));
        
        let _ = AppendMenuW(hmenu, MF_SEPARATOR, 0, PCWSTR::null());
        
        let _ = AppendMenuW(hmenu, MF_STRING, ID_COPY_PATH as usize, 
                           PCWSTR::from_raw(to_wide(&strings.ctx_copy_path).as_ptr()));
        
        let _ = AppendMenuW(hmenu, MF_STRING, ID_COPY_NAME as usize, 
                           PCWSTR::from_raw(to_wide(&strings.ctx_copy_name).as_ptr()));
        
        let _ = TrackPopupMenu(
            hmenu, 
            TPM_RIGHTALIGN | TPM_TOPALIGN, 
            x, y, 0, 
            window, 
            None
        );
        
        let _ = DestroyMenu(hmenu);
    }
}

fn show_context_menu(window: HWND, x: i32, y: i32) {
    unsafe {
        let hmenu = CreatePopupMenu().unwrap();
        let strings = get_strings();
        
        let _ = AppendMenuW(hmenu, MF_STRING, ID_VIEW_DETAILS as usize, 
                           PCWSTR::from_raw(to_wide(&strings.view_details).as_ptr()));
        let _ = AppendMenuW(hmenu, MF_STRING, ID_VIEW_MEDIUM_ICONS as usize, 
                           PCWSTR::from_raw(to_wide(&strings.view_medium_icons).as_ptr()));
        let _ = AppendMenuW(hmenu, MF_STRING, ID_VIEW_LARGE_ICONS as usize, 
                           PCWSTR::from_raw(to_wide(&strings.view_large_icons).as_ptr()));
        let _ = AppendMenuW(hmenu, MF_STRING, ID_VIEW_EXTRALARGE_ICONS as usize, 
                           PCWSTR::from_raw(to_wide(&strings.view_extra_large_icons).as_ptr()));
        
        // Check current view mode
        if let Some(state) = &APP_STATE {
            let current_id = match state.view_mode {
                ViewMode::Details => ID_VIEW_DETAILS,
                ViewMode::MediumIcons => ID_VIEW_MEDIUM_ICONS,
                ViewMode::LargeIcons => ID_VIEW_LARGE_ICONS,
                ViewMode::ExtraLargeIcons => ID_VIEW_EXTRALARGE_ICONS,
            };
            let _ = CheckMenuItem(hmenu, current_id as u32, MF_CHECKED.0);
        }
        
        let _ = TrackPopupMenu(
            hmenu, 
            TPM_RIGHTALIGN | TPM_TOPALIGN, 
            x, y, 0, 
            window, 
            None
        );
        
        let _ = DestroyMenu(hmenu);
    }
}

fn create_child_controls(parent: HWND) {
    unsafe {
        if let Some(state) = &mut APP_STATE {
            let instance = HINSTANCE(GetModuleHandleW(None).unwrap().0);
            
            // Create search edit box
            state.search_edit = CreateWindowExW(
                WS_EX_CLIENTEDGE,
                w!("EDIT"),
                w!("*.png"),
                WS_CHILD | WS_VISIBLE | WS_BORDER,
                10, 10, 980, 25,
                parent,
                HMENU(ID_SEARCH_EDIT as isize),
                instance,
                None,
            );

            SendMessageW(state.search_edit, WM_SETFONT, WPARAM(state.font.0 as usize), LPARAM(1));

            // Subclass the search edit to handle Enter key
            ORIGINAL_SEARCH_EDIT_PROC = Some(std::mem::transmute(SetWindowLongPtrW(
                state.search_edit,
                GWLP_WNDPROC,
                search_edit_proc as usize as isize,
            )));

            // Create custom list view
            state.list_view = CreateWindowExW(
                WS_EX_CLIENTEDGE,
                w!("EverythingLikeListView"),
                w!(""),
                WS_CHILD | WS_VISIBLE | WS_VSCROLL | WS_TABSTOP,
                10, 45, 980, 600,
                parent,
                HMENU(ID_LIST_VIEW as isize),
                instance,
                None,
            );

            // Create status bar
            state.status_bar = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                STATUSCLASSNAMEW,
                w!(""),
                WS_CHILD | WS_VISIBLE,
                0, 0, 0, 0,
                parent,
                HMENU(ID_STATUS_BAR as isize),
                instance,
                None,
            );

            SendMessageW(state.status_bar, WM_SETFONT, WPARAM(state.font.0 as usize), LPARAM(1));
        }
    }
}

fn resize_controls(width: i32, height: i32) {
    unsafe {
        if let Some(state) = &mut APP_STATE {
            let margin = 10;
            let edit_height = 25;
            let status_height = 25;
            let gap = 10;
            
            // Resize search edit
            let _ = SetWindowPos(
                state.search_edit,
                None,
                margin,
                margin,
                width - 2 * margin,
                edit_height,
                SWP_NOZORDER,
            );
            
            // Resize status bar (it auto-sizes its height)
            let _ = SetWindowPos(
                state.status_bar,
                None,
                0,
                height - status_height,
                width,
                status_height,
                SWP_NOZORDER,
            );
            
            // Resize list view
            let list_y = margin + edit_height + gap;
            let list_height = height - list_y - status_height - margin;
            
            let _ = SetWindowPos(
                state.list_view,
                None,
                margin,
                list_y,
                width - 2 * margin,
                list_height,
                SWP_NOZORDER,
            );
            
            // Update client dimensions and recalculate layout
            state.client_width = width - 2 * margin;
            state.client_height = list_height;
            state.calculate_layout();
            update_scrollbar(state.list_view);
        }
    }
}

fn handle_search_change() {
    unsafe {
        if let Some(state) = &mut APP_STATE {
            log_debug("handle_search_change called");
            
            // Get text from search edit control
            let mut buffer: [u16; 1024] = [0; 1024];
            let len = GetWindowTextW(state.search_edit, &mut buffer);
            
            let search_text = if len > 0 {
                String::from_utf16_lossy(&buffer[..len as usize])
            } else {
                String::new()
            };
            
            log_debug(&format!("Search text changed: '{}'", search_text));
            
            // Store the pending search query
            state.pending_search_query = search_text.clone();
            
            // Check if we're in list mode
            if state.is_list_mode {
                // For list mode, search locally without delay
                log_debug("List mode detected, performing local search");
                state.search_local_list(&search_text);
                log_debug("handle_search_change completed (list mode)");
                return;
            }
            
            // Kill existing timer if active
            if state.search_timer_active {
                KillTimer(state.main_window, SEARCH_TIMER_ID as usize);
                state.search_timer_active = false;
                log_debug("Killed existing search timer");
            }
            
            // Start new timer (500ms delay)
            if SetTimer(state.main_window, SEARCH_TIMER_ID as usize, 500, None) != 0 {
                state.search_timer_active = true;
                log_debug("Started new search timer (500ms)");
            } else {
                log_debug("ERROR: Failed to set search timer");
                // Fallback to immediate search if timer fails
                state.start_async_search(state.pending_search_query.clone());
            }
            
            log_debug("handle_search_change completed");
        } else {
            log_debug("WARNING: handle_search_change called but APP_STATE is None");
        }
    }
}

fn show_open_file_dialog(window: HWND) -> Option<String> {
    unsafe {
        use windows::Win32::System::Com::*;
        use windows::Win32::UI::Shell::*;
        use windows::Win32::UI::Shell::Common::*;
        
        // Initialize COM
        if CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE).is_err() {
            return None;
        }
        
        let file_dialog: IFileOpenDialog = match CoCreateInstance(
            &FileOpenDialog,
            None,
            CLSCTX_INPROC_SERVER,
        ) {
            Ok(dialog) => dialog,
            Err(_) => {
                CoUninitialize();
                return None;
            }
        };
        
        // Set title
        let strings = get_strings();
        let title_utf16: Vec<u16> = strings.file_open_list.encode_utf16().chain(std::iter::once(0)).collect();
        let _ = file_dialog.SetTitle(PCWSTR::from_raw(title_utf16.as_ptr()));
        
        // Create persistent storage for filter strings
        let filter_names: Vec<Vec<u16>> = vec![
            "Everything File Lists (*.efu)".encode_utf16().chain(std::iter::once(0)).collect(),
            "CSV Files (*.csv)".encode_utf16().chain(std::iter::once(0)).collect(),
            "Text Files (*.txt)".encode_utf16().chain(std::iter::once(0)).collect(),
            "All Files (*.*)".encode_utf16().chain(std::iter::once(0)).collect(),
        ];
        
        let filter_specs: Vec<Vec<u16>> = vec![
            "*.efu".encode_utf16().chain(std::iter::once(0)).collect(),
            "*.csv".encode_utf16().chain(std::iter::once(0)).collect(),
            "*.txt".encode_utf16().chain(std::iter::once(0)).collect(),
            "*.*".encode_utf16().chain(std::iter::once(0)).collect(),
        ];
        
        let filter_structs: Vec<COMDLG_FILTERSPEC> = filter_names.iter().zip(filter_specs.iter()).map(|(name, spec)| {
            COMDLG_FILTERSPEC {
                pszName: PCWSTR::from_raw(name.as_ptr()),
                pszSpec: PCWSTR::from_raw(spec.as_ptr()),
            }
        }).collect();
        
        let _ = file_dialog.SetFileTypes(&filter_structs);
        let _ = file_dialog.SetFileTypeIndex(1); // Default to .efu files
        
        // Show the dialog
        if file_dialog.Show(window).is_ok() {
            if let Ok(item) = file_dialog.GetResult() {
                if let Ok(path_bstr) = item.GetDisplayName(SIGDN_FILESYSPATH) {
                    let path_str = String::from_utf16_lossy(
                        std::slice::from_raw_parts(
                            path_bstr.as_ptr(), 
                            wcslen(path_bstr.as_ptr())
                        )
                    );
                    CoUninitialize();
                    return Some(path_str);
                }
            }
        }
        
        CoUninitialize();
        None
    }
}

fn show_save_file_dialog(window: HWND, default_name: &str) -> Option<String> {
    unsafe {
        use windows::Win32::System::Com::*;
        use windows::Win32::UI::Shell::*;
        use windows::Win32::UI::Shell::Common::*;
        
        // Initialize COM
        if CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE).is_err() {
            return None;
        }
        
        let file_dialog: IFileSaveDialog = match CoCreateInstance(
            &FileSaveDialog,
            None,
            CLSCTX_INPROC_SERVER,
        ) {
            Ok(dialog) => dialog,
            Err(_) => {
                CoUninitialize();
                return None;
            }
        };
        
        // Set default filename
        let filename_utf16: Vec<u16> = default_name.encode_utf16().chain(std::iter::once(0)).collect();
        let _ = file_dialog.SetFileName(PCWSTR::from_raw(filename_utf16.as_ptr()));
        
        // Set title
        let strings = get_strings();
        let title_utf16: Vec<u16> = strings.file_save_list.encode_utf16().chain(std::iter::once(0)).collect();
        let _ = file_dialog.SetTitle(PCWSTR::from_raw(title_utf16.as_ptr()));
        
        // Set file type filters
        let filter_names: Vec<Vec<u16>> = vec![
            "CSV Files (*.csv)".encode_utf16().chain(std::iter::once(0)).collect(),
            "Everything File Lists (*.efu)".encode_utf16().chain(std::iter::once(0)).collect(),
            "Text Files (*.txt)".encode_utf16().chain(std::iter::once(0)).collect(),
            "All Files (*.*)".encode_utf16().chain(std::iter::once(0)).collect(),
        ];
        
        let filter_specs: Vec<Vec<u16>> = vec![
            "*.csv".encode_utf16().chain(std::iter::once(0)).collect(),
            "*.efu".encode_utf16().chain(std::iter::once(0)).collect(),
            "*.txt".encode_utf16().chain(std::iter::once(0)).collect(),
            "*.*".encode_utf16().chain(std::iter::once(0)).collect(),
        ];
        
        let filter_structs: Vec<COMDLG_FILTERSPEC> = filter_names.iter().zip(filter_specs.iter()).map(|(name, spec)| {
            COMDLG_FILTERSPEC {
                pszName: PCWSTR::from_raw(name.as_ptr()),
                pszSpec: PCWSTR::from_raw(spec.as_ptr()),
            }
        }).collect();
        
        let _ = file_dialog.SetFileTypes(&filter_structs);
        let _ = file_dialog.SetFileTypeIndex(1); // Default to CSV files for saving
        
        // Show the dialog
        if file_dialog.Show(window).is_ok() {
            if let Ok(item) = file_dialog.GetResult() {
                if let Ok(path_bstr) = item.GetDisplayName(SIGDN_FILESYSPATH) {
                    let path_str = String::from_utf16_lossy(
                        std::slice::from_raw_parts(
                            path_bstr.as_ptr(), 
                            wcslen(path_bstr.as_ptr())
                        )
                    );
                    CoUninitialize();
                    return Some(path_str);
                }
            }
        }
        
        CoUninitialize();
        None
    }
}

fn wcslen(ptr: *const u16) -> usize {
    let mut len = 0;
    unsafe {
        while *ptr.add(len) != 0 {
            len += 1;
        }
    }
    len
}

// Parse EFU date format (MM/DD/YYYY HH:MM:SS AM/PM)
fn parse_efu_date(date_str: &str) -> std::result::Result<std::time::SystemTime, ()> {
    // EFU dates are typically in format like "1/1/2024 12:00:00 AM"
    // For now, return current time as fallback
    // TODO: Implement proper date parsing if needed for more accuracy
    if date_str.is_empty() {
        return Err(());
    }
    
    // Simple heuristic: if it looks like a date, return a reasonable fallback
    if date_str.contains("/") && (date_str.contains("AM") || date_str.contains("PM")) {
        // Return UNIX epoch + some time to indicate it was parsed from EFU
        Ok(std::time::UNIX_EPOCH + std::time::Duration::from_secs(946684800)) // Year 2000
    } else {
        Err(())
    }
}
