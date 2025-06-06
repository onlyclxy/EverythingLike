use libloading::{Library, Symbol};
use windows::core::PCWSTR;
use windows::Win32::Foundation::BOOL;

// Everything SDK function signatures
type EverythingSetSearchW = extern "system" fn(search: PCWSTR);
type EverythingQueryW = extern "system" fn(wait: BOOL) -> BOOL;
type EverythingGetNumResults = extern "system" fn() -> u32;
type EverythingGetResultFullPathNameW = extern "system" fn(index: u32, buf: *mut u16, buf_size: u32) -> u32;
type EverythingCleanUp = extern "system" fn();

pub struct EverythingSDK {
    _lib: Library,
    set_search: EverythingSetSearchW,
    query: EverythingQueryW,
    get_num_results: EverythingGetNumResults,
    get_result_full_path: EverythingGetResultFullPathNameW,
    cleanup: EverythingCleanUp,
}

impl EverythingSDK {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        unsafe {
            // Try to load Everything64.dll from system path
            let lib = Library::new("Everything64.dll")
                .or_else(|_| Library::new("Everything32.dll"))
                .or_else(|_| Library::new("Everything.dll"))?;
            
            // Get function pointers
            let set_search: Symbol<EverythingSetSearchW> = lib.get(b"Everything_SetSearchW")?;
            let query: Symbol<EverythingQueryW> = lib.get(b"Everything_QueryW")?;
            let get_num_results: Symbol<EverythingGetNumResults> = lib.get(b"Everything_GetNumResults")?;
            let get_result_full_path: Symbol<EverythingGetResultFullPathNameW> = lib.get(b"Everything_GetResultFullPathNameW")?;
            let cleanup: Symbol<EverythingCleanUp> = lib.get(b"Everything_CleanUp")?;
            
            // Store the function pointers
            let set_search_fn = *set_search;
            let query_fn = *query;
            let get_num_results_fn = *get_num_results;
            let get_result_full_path_fn = *get_result_full_path;
            let cleanup_fn = *cleanup;
            
            Ok(Self {
                _lib: lib,
                set_search: set_search_fn,
                query: query_fn,
                get_num_results: get_num_results_fn,
                get_result_full_path: get_result_full_path_fn,
                cleanup: cleanup_fn,
            })
        }
    }
    
    pub fn set_search(&self, query: &str) -> Result<(), Box<dyn std::error::Error>> {
        let query_utf16: Vec<u16> = query.encode_utf16().chain(std::iter::once(0)).collect();
        let query_pcwstr = PCWSTR::from_raw(query_utf16.as_ptr());
        
        unsafe {
            (self.set_search)(query_pcwstr);
        }
        
        Ok(())
    }
    
    pub fn query(&self, wait: bool) -> Result<bool, Box<dyn std::error::Error>> {
        let wait_bool = BOOL::from(wait);
        
        unsafe {
            let result = (self.query)(wait_bool);
            Ok(result.as_bool())
        }
    }
    
    pub fn get_num_results(&self) -> u32 {
        unsafe {
            (self.get_num_results)()
        }
    }
    
    pub fn get_result_full_path(&self, index: u32) -> Result<String, Box<dyn std::error::Error>> {
        const MAX_PATH_SIZE: u32 = 32768; // Large buffer for long paths
        let mut buffer: Vec<u16> = vec![0; MAX_PATH_SIZE as usize];
        
        unsafe {
            let chars_copied = (self.get_result_full_path)(index, buffer.as_mut_ptr(), MAX_PATH_SIZE);
            
            if chars_copied == 0 {
                return Err("Failed to get result path".into());
            }
            
            // Find the null terminator
            let end = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
            buffer.truncate(end);
            
            // Convert UTF-16 to String
            String::from_utf16(&buffer).map_err(|e| e.into())
        }
    }
    
    pub fn search_files(&self, query: &str) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        // Set the search query
        self.set_search(query)?;
        
        // Execute the search
        if !self.query(true)? {
            return Err("Query failed".into());
        }
        
        // Get number of results
        let num_results = self.get_num_results();
        let mut results = Vec::new();
        
        // Collect all results
        for i in 0..num_results {
            match self.get_result_full_path(i) {
                Ok(path) => results.push(path),
                Err(_) => continue, // Skip failed entries
            }
        }
        
        Ok(results)
    }
}

impl Drop for EverythingSDK {
    fn drop(&mut self) {
        unsafe {
            (self.cleanup)();
        }
    }
}

#[derive(Debug, Clone)]
pub struct FileResult {
    pub name: String,
    pub path: String,
    pub size: u64,
    pub modified_time: std::time::SystemTime,
    pub file_type: String,
    pub extension: String,
}

impl FileResult {
    pub fn from_path(path: &str) -> Self {
        let name = std::path::Path::new(path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Unknown")
            .to_string();
            
        let extension = std::path::Path::new(path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_string();
            
        let file_type = if extension.is_empty() {
            "File".to_string()
        } else {
            format!("{} File", extension.to_uppercase())
        };

        Self {
            name,
            path: path.to_string(),
            size: 0,  // Lazy load when needed
            modified_time: std::time::UNIX_EPOCH,  // Lazy load when needed
            file_type,
            extension,
        }
    }
    
    pub fn load_metadata(&mut self) {
        if self.size == 0 && self.modified_time == std::time::UNIX_EPOCH {
            if let Ok(metadata) = std::fs::metadata(&self.path) {
                self.size = metadata.len();
                self.modified_time = metadata.modified().unwrap_or(std::time::UNIX_EPOCH);
            }
        }
    }
    
    pub fn format_size(&self) -> String {
        if self.size == 0 {
            return String::new();
        }
        
        if self.size > 1024 * 1024 * 1024 {
            format!("{:.1} GB", self.size as f64 / (1024.0 * 1024.0 * 1024.0))
        } else if self.size > 1024 * 1024 {
            format!("{:.1} MB", self.size as f64 / (1024.0 * 1024.0))
        } else if self.size > 1024 {
            format!("{:.1} KB", self.size as f64 / 1024.0)
        } else {
            format!("{} bytes", self.size)
        }
    }
    
    pub fn format_modified_time(&self) -> String {
        if self.modified_time == std::time::UNIX_EPOCH {
            return String::new();
        }
        
        match self.modified_time.duration_since(std::time::UNIX_EPOCH) {
            Ok(duration) => {
                let secs = duration.as_secs();
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                
                let diff_secs = now.saturating_sub(secs);
                let diff_days = diff_secs / (24 * 3600);
                
                // Use a simple fallback if we can't get language strings
                if diff_days == 0 {
                    "Today".to_string()
                } else if diff_days == 1 {
                    "Yesterday".to_string()
                } else if diff_days < 7 {
                    format!("{} days ago", diff_days)
                } else if diff_days < 30 {
                    format!("{} weeks ago", diff_days / 7)
                } else if diff_days < 365 {
                    format!("{} months ago", diff_days / 30)
                } else {
                    // For files older than a year, show actual date
                    let days_since_epoch = secs / (24 * 3600);
                    let epoch_days = 719162; // Days from 1/1/1 to 1/1/1970
                    let total_days = epoch_days + days_since_epoch;
                    
                    // Simple date calculation (year/month/day)
                    let year = 1 + total_days / 365; // Rough approximation
                    let remaining_days = total_days % 365;
                    let month = 1 + remaining_days / 30; // Rough approximation
                    let day = 1 + remaining_days % 30;
                    
                    format!("{}/{}/{}", month, day, year)
                }
            }
            Err(_) => String::new(),
        }
    }
} 