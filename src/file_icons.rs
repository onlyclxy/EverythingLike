use windows::{
    core::*,
    Win32::{
        Storage::FileSystem::*,
        Graphics::Gdi::*,
        UI::{
            WindowsAndMessaging::*,
            Shell::*,
        },
    },
};
use lru::LruCache;
use std::num::NonZeroUsize;
use std::path::Path;

// Icon cache for file extensions
static mut ICON_CACHE: Option<LruCache<String, HICON>> = None;

// Initialize the icon cache
pub fn init_icon_cache() {
    unsafe {
        ICON_CACHE = Some(LruCache::new(NonZeroUsize::new(200).unwrap()));
    }
}

// Get file icon by file path
pub fn get_file_icon(file_path: &str, small: bool) -> Option<HICON> {
    unsafe {
        // Get file extension for caching
        let extension = Path::new(file_path)
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_lowercase();
        
        // Create cache key based on extension and size
        let cache_key = format!("{}_{}", extension, if small { "small" } else { "large" });
        
        // Check cache first
        if let Some(ref mut cache) = ICON_CACHE {
            if let Some(&cached_icon) = cache.get(&cache_key) {
                return Some(cached_icon);
            }
        }
        
        // Get icon using SHGetFileInfoW
        let mut file_info = SHFILEINFOW::default();
        let file_path_wide: Vec<u16> = file_path.encode_utf16().chain(std::iter::once(0)).collect();
        
        let flags = SHGFI_ICON | if small { SHGFI_SMALLICON } else { SHGFI_LARGEICON };
        
        let result = SHGetFileInfoW(
            PCWSTR::from_raw(file_path_wide.as_ptr()),
            FILE_FLAGS_AND_ATTRIBUTES(0),
            Some(&mut file_info),
            std::mem::size_of::<SHFILEINFOW>() as u32,
            flags,
        );
        
        if result != 0 && !file_info.hIcon.is_invalid() {
            let icon = file_info.hIcon;
            
            // Cache the icon
            if let Some(ref mut cache) = ICON_CACHE {
                cache.put(cache_key, icon);
            }
            
            Some(icon)
        } else {
            None
        }
    }
}

// Get default file icon for unknown types
pub fn get_default_file_icon(small: bool) -> Option<HICON> {
    unsafe {
        let mut file_info = SHFILEINFOW::default();
        let flags = SHGFI_ICON | SHGFI_USEFILEATTRIBUTES | if small { SHGFI_SMALLICON } else { SHGFI_LARGEICON };
        
        let result = SHGetFileInfoW(
            w!(""),
            FILE_FLAGS_AND_ATTRIBUTES(FILE_ATTRIBUTE_NORMAL.0),
            Some(&mut file_info),
            std::mem::size_of::<SHFILEINFOW>() as u32,
            flags,
        );
        
        if result != 0 && !file_info.hIcon.is_invalid() {
            Some(file_info.hIcon)
        } else {
            None
        }
    }
}

// Get folder icon
pub fn get_folder_icon(small: bool) -> Option<HICON> {
    unsafe {
        let mut file_info = SHFILEINFOW::default();
        let flags = SHGFI_ICON | SHGFI_USEFILEATTRIBUTES | if small { SHGFI_SMALLICON } else { SHGFI_LARGEICON };
        
        let result = SHGetFileInfoW(
            w!(""),
            FILE_FLAGS_AND_ATTRIBUTES(FILE_ATTRIBUTE_DIRECTORY.0),
            Some(&mut file_info),
            std::mem::size_of::<SHFILEINFOW>() as u32,
            flags,
        );
        
        if result != 0 && !file_info.hIcon.is_invalid() {
            Some(file_info.hIcon)
        } else {
            None
        }
    }
}

// Draw icon at specified position
pub fn draw_icon(hdc: HDC, icon: HICON, x: i32, y: i32, size: i32) {
    unsafe {
        let _ = DrawIconEx(hdc, x, y, icon, size, size, 0, HBRUSH::default(), DI_NORMAL);
    }
}

// Cleanup icon cache
pub fn cleanup_icon_cache() {
    unsafe {
        if let Some(ref mut cache) = ICON_CACHE {
            // Icons are system resources, Windows manages them
            // We don't need to explicitly destroy them
            cache.clear();
        }
    }
} 