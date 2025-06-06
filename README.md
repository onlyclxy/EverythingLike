# EverythingLike | 类Everything文件搜索工具

[English](#english) | [中文](#中文)

## 中文

### 项目简介

EverythingLike 是一个用 Rust 编写的高性能文件搜索工具，灵感来源于知名的 Everything 软件。本项目使用 Win32 API 和 Everything SDK 实现了快速的文件索引和搜索功能。

### 主要功能

- 🚀 **高速搜索**: 集成 Everything SDK，提供毫秒级文件搜索
- 🔄 **异步架构**: 非阻塞搜索，支持搜索防抖，避免频繁查询
- 🎨 **多种视图模式**: 
  - 详细信息视图
  - 中等图标视图  
  - 大图标视图
  - 超大图标视图
- 🖼️ **缩略图支持**: 智能缩略图生成和缓存
- 🌍 **多语言支持**: 支持中文和英文界面
- ⌨️ **键盘导航**: 完整的键盘快捷键支持
- 📂 **文件列表管理**: 支持保存、加载和导出搜索结果
- 🎯 **智能排序**: 支持按名称、大小、类型、修改时间、路径排序
- 🔧 **可配置界面**: 可自定义列显示和缩略图策略

### 技术特性

#### 架构设计
- **线程安全**: 专用搜索线程处理 Everything SDK 调用
- **消息驱动**: 使用 Windows 消息进行线程间通信
- **内存管理**: 使用 LRU 缓存优化缩略图内存使用
- **虚拟滚动**: 只渲染可见项目，提升大量文件时的性能

#### 性能优化
- **搜索防抖**: 300ms 延迟避免输入时的过度搜索
- **结果限制**: 限制最大结果数量防止界面卡顿
- **双缓冲绘制**: 消除界面闪烁
- **并行处理**: 使用 Rayon 并行处理文件操作

### 构建要求

- Rust 1.70 或更高版本
- Windows 10 或更高版本
- Everything 软件 (可选，用于最佳性能)

### 构建步骤

```bash
# 克隆仓库
git clone https://github.com/onlyclxy/EverythingLike.git
cd EverythingLike

# 构建项目
cargo build --release

# 运行程序
cargo run --release
```

### 使用说明

1. **安装 Everything**: 从 [voidtools.com](https://www.voidtools.com/) 下载安装以获得最佳性能
2. **运行程序**: 启动编译后的 `everything-like.exe`
3. **搜索文件**: 在搜索框中输入关键词，结果会自动更新
4. **切换视图**: 通过菜单选择不同的查看模式
5. **打开文件**: 双击文件或按回车键打开选中的文件

### 项目结构

```
src/
├── main.rs              # 主程序入口和 UI 逻辑
├── everything_sdk.rs    # Everything SDK 封装
├── thumbnail.rs         # 缩略图生成和管理
├── config.rs            # 配置管理
├── lang.rs              # 多语言支持
└── file_icons.rs        # 文件图标管理
```

### 配置文件

程序会自动创建配置文件 `config.json`，包含：
- 界面语言设置
- 缩略图策略配置
- 视图模式偏好
- 列显示设置

---

## English

### Project Overview

EverythingLike is a high-performance file search tool written in Rust, inspired by the popular Everything software. This project uses Win32 API and Everything SDK to implement fast file indexing and search capabilities.

### Key Features

- 🚀 **Lightning Fast Search**: Integrated Everything SDK for millisecond-level file searching
- 🔄 **Async Architecture**: Non-blocking search with debouncing to prevent excessive queries
- 🎨 **Multiple View Modes**:
  - Details view
  - Medium icons view
  - Large icons view
  - Extra large icons view
- 🖼️ **Thumbnail Support**: Smart thumbnail generation and caching
- 🌍 **Multi-language Support**: Chinese and English interface
- ⌨️ **Keyboard Navigation**: Complete keyboard shortcut support
- 📂 **File List Management**: Save, load, and export search results
- 🎯 **Smart Sorting**: Sort by name, size, type, modified time, and path
- 🔧 **Configurable UI**: Customizable column display and thumbnail strategies

### Technical Features

#### Architecture Design
- **Thread Safety**: Dedicated search thread for Everything SDK calls
- **Message-Driven**: Windows messages for thread-safe communication
- **Memory Management**: LRU cache for efficient thumbnail memory usage
- **Virtual Scrolling**: Only render visible items for better performance with large result sets

#### Performance Optimizations
- **Search Debouncing**: 300ms delay to prevent excessive searches during typing
- **Result Limiting**: Cap maximum results to prevent UI slowdown
- **Double-Buffered Rendering**: Eliminates UI flickering
- **Parallel Processing**: Uses Rayon for parallel file operations

### Build Requirements

- Rust 1.70 or higher
- Windows 10 or higher
- Everything software (optional, for best performance)

### Build Instructions

```bash
# Clone the repository
git clone https://github.com/onlyclxy/EverythingLike.git
cd EverythingLike

# Build the project
cargo build --release

# Run the application
cargo run --release
```

### Usage

1. **Install Everything**: Download from [voidtools.com](https://www.voidtools.com/) for optimal performance
2. **Run Application**: Launch the compiled `everything-like.exe`
3. **Search Files**: Type keywords in the search box - results update automatically
4. **Switch Views**: Use menu options to select different view modes
5. **Open Files**: Double-click files or press Enter to open selected files

### Project Structure

```
src/
├── main.rs              # Main entry point and UI logic
├── everything_sdk.rs    # Everything SDK wrapper
├── thumbnail.rs         # Thumbnail generation and management
├── config.rs            # Configuration management
├── lang.rs              # Multi-language support
└── file_icons.rs        # File icon management
```

### Configuration

The application automatically creates a `config.json` file containing:
- Interface language settings
- Thumbnail strategy configuration
- View mode preferences
- Column display settings

### Dependencies

Key Rust dependencies:
- `windows`: Win32 API bindings
- `lru`: LRU cache implementation
- `rayon`: Parallel processing
- `serde`: Configuration serialization
- `rusqlite`: Database support for file lists
- `chrono`: Date/time handling

### License

This project is licensed under the MIT License - see the [LICENSE.txt](LICENSE.txt) file for details.

### Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

### Acknowledgments

- Inspired by [Everything](https://www.voidtools.com/) by voidtools
- Built with the amazing Rust ecosystem
- Uses Windows Win32 API for native performance 