# Everything-like File Browser (C++)

A high-performance file browser inspired by Everything, implemented in C++ with Win32 API.

## Features

- **Async Search**: Non-blocking search with proper debouncing
- **Everything SDK Integration**: Uses Everything SDK for ultra-fast file indexing
- **Multiple View Modes**: Details, Medium Icons, Large Icons, Extra Large Icons
- **Thread-Safe Architecture**: Single dedicated search thread prevents race conditions
- **Efficient Rendering**: Double-buffered painting with virtual scrolling
- **Keyboard Navigation**: Full keyboard support for file navigation
- **Sample Data Fallback**: Works even without Everything installed

## Architecture

### Thread Safety
- **Single Search Thread**: All Everything SDK calls go through one dedicated thread
- **Message-Based Communication**: Uses Windows messages for thread-safe UI updates
- **Proper Cancellation**: Search requests can be cancelled cleanly
- **No Race Conditions**: Eliminates the thread safety issues from the Rust version

### Performance Optimizations
- **Debounced Search**: 150ms delay prevents excessive searches during typing
- **Virtual Scrolling**: Only renders visible items for smooth performance
- **Result Limiting**: Caps results at 50,000 items to prevent UI slowdown
- **Memory Management**: Proper RAII and resource cleanup

## Building

### Prerequisites
- Visual Studio 2022 (or compatible C++ compiler)
- CMake 3.16 or later
- Windows 10 SDK

### Build Steps
```batch
# Using the provided build script
build.bat

# Or manually with CMake
mkdir build
cd build
cmake .. -G "Visual Studio 17 2022" -A x64
cmake --build . --config Release
```

### Output
The executable will be generated at: `build/bin/Release/EverythingLike.exe`

## Usage

1. **Install Everything**: Download from [voidtools.com](https://www.voidtools.com/) for best performance
2. **Run the Application**: Launch `EverythingLike.exe`
3. **Search**: Type in the search box - results update automatically
4. **Navigate**: Use keyboard arrows or mouse to select files
5. **Open Files**: Double-click or press Enter to open selected files

## Key Improvements over Rust Version

1. **Stability**: No more STATUS_ACCESS_VIOLATION crashes
2. **Simplicity**: Cleaner architecture without complex async frameworks
3. **Performance**: Direct Win32 API calls for maximum efficiency
4. **Thread Safety**: Proper synchronization eliminates race conditions
5. **Resource Management**: RAII ensures proper cleanup

## Code Structure

```
src/
├── main.cpp              # Application entry point
├── Common.h              # Shared types and utilities
├── EverythingSDK.*       # Everything SDK wrapper
├── SearchManager.*       # Async search management
├── MainWindow.*          # Main application window
├── ListView.*            # File list display component
├── ThumbnailManager.*    # Thumbnail support (placeholder)
└── ConfigManager.*       # Configuration management (placeholder)
```

## Technical Details

### Search Flow
1. User types in search box (EN_CHANGE notification)
2. SearchManager cancels previous searches and queues new request
3. Dedicated search thread processes request after debounce delay
4. Everything SDK performs the actual search
5. Results sent back to UI thread via Windows message
6. ListView updates display with new results

### Thread Architecture
- **UI Thread**: Handles all Windows messages and rendering
- **Search Thread**: Dedicated thread for Everything SDK calls
- **Communication**: Thread-safe queues and Windows messages

This C++ implementation provides a solid foundation that can be extended with additional features while maintaining stability and performance. 