use windows::{
    core::*,
    Win32::{
        Foundation::*,
        Graphics::Gdi::*,
        System::Com::*,
        UI::{
            Shell::*,
            WindowsAndMessaging::PostMessageW,
        },
    },
};
use rayon::ThreadPool;
use std::sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}};
use std::collections::{HashSet, HashMap};
use crate::config::{ThumbnailStrategy, ThumbnailBackground};

// Custom messages for thumbnail system
pub const WM_THUMBNAIL_READY: u32 = 0x0400 + 2; // WM_APP + 2
pub const WM_RECOMPUTE_THUMBS: u32 = 0x0400 + 10; // WM_APP + 10

#[derive(Clone)]
pub struct ThumbnailRequest {
    pub item_index: usize,
    pub file_path: String,
    pub size: u32,
    pub background: ThumbnailBackground,
    pub cancellation_token: Arc<AtomicBool>,
}

#[derive(Clone)]
pub struct ThumbnailTaskManager {
    pub queued_set: Arc<Mutex<HashSet<usize>>>,
    pub cancellation_tokens: Arc<Mutex<HashMap<usize, Arc<AtomicBool>>>>,
    pub thread_pool: Arc<ThreadPool>,
    pub window_handle: HWND,
}

impl ThumbnailTaskManager {
    pub fn new(window_handle: HWND) -> Self {
        let thread_pool = rayon::ThreadPoolBuilder::new()
            .num_threads(4) // Use 4 background threads for thumbnail generation
            .build()
            .expect("Failed to create thread pool");

        Self {
            queued_set: Arc::new(Mutex::new(HashSet::new())),
            cancellation_tokens: Arc::new(Mutex::new(HashMap::new())),
            thread_pool: Arc::new(thread_pool),
            window_handle,
        }
    }

    pub fn cancel_all_tasks(&self) {
        println!("Cancelling all thumbnail tasks");
        
        // Cancel all existing tasks
        if let Ok(tokens) = self.cancellation_tokens.lock() {
            for (_, token) in tokens.iter() {
                token.store(true, Ordering::Relaxed);
            }
        }
        
        // Clear queued set and cancellation tokens
        if let Ok(mut queued) = self.queued_set.lock() {
            queued.clear();
        }
        
        if let Ok(mut tokens) = self.cancellation_tokens.lock() {
            tokens.clear();
        }
    }

    pub fn cancel_task(&self, index: usize) {
        if let Ok(tokens) = self.cancellation_tokens.lock() {
            if let Some(token) = tokens.get(&index) {
                token.store(true, Ordering::Relaxed);
            }
        }
        
        if let Ok(mut queued) = self.queued_set.lock() {
            queued.remove(&index);
        }
        
        if let Ok(mut tokens) = self.cancellation_tokens.lock() {
            tokens.remove(&index);
        }
    }

    pub fn is_task_queued(&self, index: usize) -> bool {
        if let Ok(queued) = self.queued_set.lock() {
            queued.contains(&index)
        } else {
            false
        }
    }

    pub fn request_thumbnail(&self, request: ThumbnailRequest) {
        let index = request.item_index;
        
        // Check if already queued
        if self.is_task_queued(index) {
            return;
        }
        
        // Add to queued set
        if let Ok(mut queued) = self.queued_set.lock() {
            queued.insert(index);
        }
        
        // Store cancellation token
        if let Ok(mut tokens) = self.cancellation_tokens.lock() {
            tokens.insert(index, request.cancellation_token.clone());
        }
        
        // Spawn background task
        let task_manager = self.clone();
        let request_clone = request.clone();
        
        self.thread_pool.spawn(move || {
            // Initialize COM for this thread
            unsafe {
                let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
            }
            
            // Check cancellation before starting work
            if request_clone.cancellation_token.load(Ordering::Relaxed) {
                task_manager.cleanup_task(index);
                unsafe { CoUninitialize(); }
                return;
            }
            
            // Generate thumbnail
            if let Some(thumbnail) = get_shell_thumbnail(&request_clone.file_path, request_clone.size, request_clone.background) {
                // Check cancellation again before posting result
                if !request_clone.cancellation_token.load(Ordering::Relaxed) {
                    unsafe {
                        let _ = PostMessageW(
                            task_manager.window_handle,
                            WM_THUMBNAIL_READY,
                            WPARAM(request_clone.item_index),
                            LPARAM(thumbnail.0 as isize),
                        );
                    }
                } else {
                    // Task was cancelled, delete the bitmap
                    unsafe {
                        DeleteObject(thumbnail);
                    }
                }
            }
            
            task_manager.cleanup_task(index);
            
            unsafe {
                CoUninitialize();
            }
        });
    }
    
    fn cleanup_task(&self, index: usize) {
        if let Ok(mut queued) = self.queued_set.lock() {
            queued.remove(&index);
        }
        
        if let Ok(mut tokens) = self.cancellation_tokens.lock() {
            tokens.remove(&index);
        }
    }

    pub fn recompute_thumbnail_queue(
        &self,
        strategy: ThumbnailStrategy,
        background: ThumbnailBackground,
        visible_start: usize,
        visible_count: usize,
        total_items: usize,
        list_data: &[crate::everything_sdk::FileResult],
        selected_view_size: u32,
    ) {
        // Compute desired set based on strategy
        let desired_set: HashSet<usize> = match strategy {
            ThumbnailStrategy::DefaultTopToBottom => {
                // Mode A: All items from 0 to total_items
                (0..total_items).collect()
            }
            ThumbnailStrategy::OnlyLoadVisible => {
                // Mode B: Only visible items
                let visible_end = (visible_start + visible_count).min(total_items);
                (visible_start..visible_end).collect()
            }
            ThumbnailStrategy::LoadVisiblePlus500 => {
                // Mode C: Visible + next 500
                let visible_end = (visible_start + visible_count).min(total_items);
                let extended_end = (visible_end + 500).min(total_items);
                (visible_start..extended_end).collect()
            }
        };

        // Get current queued set
        let current_queued: HashSet<usize> = if let Ok(queued) = self.queued_set.lock() {
            queued.clone()
        } else {
            HashSet::new()
        };

        // Cancel tasks not in desired set
        for &index in &current_queued {
            if !desired_set.contains(&index) {
                self.cancel_task(index);
            }
        }

        // Queue new tasks for desired items not already queued
        for &index in &desired_set {
            if !current_queued.contains(&index) && index < list_data.len() {
                let cancellation_token = Arc::new(AtomicBool::new(false));
                let request = ThumbnailRequest {
                    item_index: index,
                    file_path: list_data[index].path.clone(),
                    size: selected_view_size,
                    background: background,
                    cancellation_token,
                };
                self.request_thumbnail(request);
            }
        }

        println!(
            "Recomputed thumbnail queue - Strategy: {:?}, Desired: {}, Currently queued: {}",
            strategy,
            desired_set.len(),
            self.get_queued_count()
        );
    }

    pub fn get_queued_count(&self) -> usize {
        if let Ok(queued) = self.queued_set.lock() {
            queued.len()
        } else {
            0
        }
    }
}

pub fn get_shell_thumbnail(path: &str, size: u32, background: ThumbnailBackground) -> Option<HBITMAP> {
    unsafe {
        // Convert path to wide string
        let path_wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
        
        // Create shell item from path
        let shell_item: IShellItem = match SHCreateItemFromParsingName(
            PCWSTR::from_raw(path_wide.as_ptr()),
            None,
        ) {
            Ok(item) => item,
            Err(_) => return None,
        };
        
        // Get the image factory interface
        let image_factory: IShellItemImageFactory = match shell_item.cast() {
            Ok(factory) => factory,
            Err(_) => return None,
        };
        
        // Create SIZE structure
        let thumbnail_size = SIZE {
            cx: size as i32,
            cy: size as i32,
        };
        
        // Get the original thumbnail bitmap
        let original_bitmap = match image_factory.GetImage(thumbnail_size, SIIGBF_RESIZETOFIT) {
            Ok(hbitmap) => hbitmap,
            Err(_) => return None,
        };
        
        // Apply custom background if needed
        match background {
            ThumbnailBackground::Transparent => {
                // Return original thumbnail as-is for transparent background
                Some(original_bitmap)
            }
            _ => {
                // Create a new bitmap with custom background
                Some(apply_custom_background(original_bitmap, size, background))
            }
        }
    }
}

fn apply_custom_background(original_bitmap: HBITMAP, size: u32, background: ThumbnailBackground) -> HBITMAP {
    unsafe {
        let hdc = GetDC(HWND(0));
        let mem_dc = CreateCompatibleDC(hdc);
        let background_dc = CreateCompatibleDC(hdc);
        
        // Create new bitmap for the result
        let result_bitmap = CreateCompatibleBitmap(hdc, size as i32, size as i32);
        let old_result = SelectObject(mem_dc, result_bitmap);
        let old_bg = SelectObject(background_dc, original_bitmap);
        
        // Fill background first
        let rect = RECT {
            left: 0,
            top: 0,
            right: size as i32,
            bottom: size as i32,
        };
        
        match background {
            ThumbnailBackground::Checkerboard => {
                draw_checkerboard_background(mem_dc, &rect);
            }
            _ => {
                // Solid color background
                let color = background.to_color_ref();
                let bg_brush = CreateSolidBrush(COLORREF(color));
                FillRect(mem_dc, &rect, bg_brush);
                DeleteObject(bg_brush);
            }
        }
        
        // Get original bitmap dimensions
        let mut bitmap_info = BITMAP::default();
        GetObjectW(
            original_bitmap, 
            std::mem::size_of::<BITMAP>() as i32,
            Some(&mut bitmap_info as *mut _ as *mut std::ffi::c_void)
        );
        
        // Calculate position to center the original thumbnail
        let src_width = bitmap_info.bmWidth;
        let src_height = bitmap_info.bmHeight;
        let dest_x = ((size as i32) - src_width) / 2;
        let dest_y = ((size as i32) - src_height) / 2;
        
        // Draw the original thumbnail on top of the background with alpha blending
        let blend_func = BLENDFUNCTION {
            BlendOp: 0, // AC_SRC_OVER
            BlendFlags: 0,
            SourceConstantAlpha: 255, // Opaque
            AlphaFormat: 1, // AC_SRC_ALPHA
        };
        
        let _ = AlphaBlend(
            mem_dc,
            dest_x.max(0),
            dest_y.max(0),
            src_width.min(size as i32),
            src_height.min(size as i32),
            background_dc,
            0,
            0,
            src_width,
            src_height,
            blend_func,
        );
        
        // Clean up
        SelectObject(mem_dc, old_result);
        SelectObject(background_dc, old_bg);
        DeleteDC(mem_dc);
        DeleteDC(background_dc);
        ReleaseDC(HWND(0), hdc);
        
        // Delete the original bitmap since we're returning a new one
        DeleteObject(original_bitmap);
        
        result_bitmap
    }
}

fn draw_checkerboard_background(hdc: HDC, rect: &RECT) {
    unsafe {
        let checker_size = 8i32; // Size of each checker square
        let light_brush = CreateSolidBrush(COLORREF(0x00F0F0F0)); // Light gray
        let dark_brush = CreateSolidBrush(COLORREF(0x00E0E0E0));  // Slightly darker gray
        
        let width = rect.right - rect.left;
        let height = rect.bottom - rect.top;
        
        for y in (0..height).step_by(checker_size as usize) {
            for x in (0..width).step_by(checker_size as usize) {
                let checker_rect = RECT {
                    left: rect.left + x,
                    top: rect.top + y,
                    right: (rect.left + x + checker_size).min(rect.right),
                    bottom: (rect.top + y + checker_size).min(rect.bottom),
                };
                
                // Alternate pattern: if (x/checker_size + y/checker_size) is even, use light, else dark
                let is_light = ((x / checker_size) + (y / checker_size)) % 2 == 0;
                let brush = if is_light { light_brush } else { dark_brush };
                
                FillRect(hdc, &checker_rect, brush);
            }
        }
        
        DeleteObject(light_brush);
        DeleteObject(dark_brush);
    }
}

pub fn create_placeholder_bitmap(size: u32) -> HBITMAP {
    unsafe {
        let hdc = GetDC(HWND(0));
        let mem_dc = CreateCompatibleDC(hdc);
        let bitmap = CreateCompatibleBitmap(hdc, size as i32, size as i32);
        let old_bitmap = SelectObject(mem_dc, bitmap);
        
        // Draw a simple placeholder (folder icon representation)
        let rect = RECT {
            left: 0,
            top: 0,
            right: size as i32,
            bottom: size as i32,
        };
        
        let bg_brush = CreateSolidBrush(COLORREF(0x00F0F0F0));
        FillRect(mem_dc, &rect, bg_brush);
        DeleteObject(bg_brush);
        
        // Draw a simple folder-like shape
        let border_brush = CreateSolidBrush(COLORREF(0x00808080));
        let old_brush = SelectObject(mem_dc, border_brush);
        let pen = CreatePen(PS_SOLID, 1, COLORREF(0x00404040));
        let old_pen = SelectObject(mem_dc, pen);
        
        let margin = (size / 8) as i32;
        Rectangle(mem_dc, margin, margin, size as i32 - margin, size as i32 - margin);
        
        SelectObject(mem_dc, old_pen);
        SelectObject(mem_dc, old_brush);
        SelectObject(mem_dc, old_bitmap);
        DeleteObject(pen);
        DeleteObject(border_brush);
        DeleteDC(mem_dc);
        ReleaseDC(HWND(0), hdc);
        
        bitmap
    }
}

// Helper function to convert string to wide string
pub fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
} 