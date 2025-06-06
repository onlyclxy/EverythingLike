# About EverythingLike | 关于 EverythingLike

## 中文版本

### 项目背景

EverythingLike 是一个致力于为 Windows 用户提供快速文件搜索体验的开源项目。该项目受到知名文件搜索工具 Everything 的启发，使用现代的 Rust 编程语言重新实现了核心功能，并在此基础上增加了更多实用特性。

### 设计理念

- **性能至上**: 利用 Everything SDK 和 Rust 的零成本抽象特性，实现毫秒级文件搜索
- **用户友好**: 提供直观的界面和丰富的视图模式，满足不同用户的使用习惯
- **现代化**: 采用异步架构和现代 UI 设计理念，提供流畅的用户体验
- **可扩展**: 模块化设计，易于添加新功能和自定义

### 核心技术

#### Rust 语言优势
- **内存安全**: 避免缓冲区溢出和内存泄漏等常见问题
- **并发安全**: 编译时保证线程安全，避免数据竞争
- **零成本抽象**: 高级语言特性不影响运行时性能
- **跨平台兼容**: 为未来可能的跨平台扩展奠定基础

#### Windows 原生集成
- **Win32 API**: 直接调用 Windows 原生 API，获得最佳性能
- **Everything SDK**: 集成 Everything 的核心搜索引擎
- **系统主题**: 自动适配 Windows 系统主题和字体设置
- **文件关联**: 支持系统默认的文件打开方式

### 功能特色

#### 智能搜索
- **实时搜索**: 输入即搜索，无需按回车
- **搜索防抖**: 智能延迟避免过度查询
- **模糊匹配**: 支持部分匹配和通配符搜索
- **搜索历史**: 记住常用搜索词

#### 多样化视图
- **详细列表**: 类似文件管理器的详细信息显示
- **图标视图**: 多种图标大小选择（中、大、超大）
- **缩略图**: 支持图片和视频文件缩略图预览
- **自定义列**: 可选择显示的文件属性列

#### 文件管理
- **快速打开**: 双击或回车直接打开文件
- **打开位置**: 在文件管理器中显示文件位置
- **复制路径**: 快速复制文件路径到剪贴板
- **右键菜单**: 集成Windows右键上下文菜单

### 技术亮点

#### 异步架构
```
UI线程 ──→ 搜索请求 ──→ 搜索线程
   ↑                        ↓
搜索结果 ←── Windows消息 ←── Everything SDK
```

- **非阻塞UI**: 搜索过程不影响界面响应
- **线程安全**: 专用搜索线程处理SDK调用
- **消息通信**: 使用Windows消息机制确保线程间安全通信

#### 性能优化
- **虚拟滚动**: 大量结果时只渲染可见部分
- **LRU缓存**: 智能缓存缩略图减少重复生成
- **并行处理**: 利用多核CPU并行处理文件操作
- **内存管理**: 及时释放不需要的资源

#### 用户体验
- **响应式设计**: 界面元素随窗口大小自适应
- **键盘导航**: 完整的键盘快捷键支持
- **多语言**: 中英文界面切换
- **配置持久化**: 记住用户的偏好设置

### 开发历程

1. **概念阶段**: 分析 Everything 的优缺点，确定改进方向
2. **原型开发**: 使用 Rust 和 Win32 API 构建基础框架
3. **核心功能**: 实现搜索、显示、导航等基本功能
4. **优化改进**: 添加缩略图、多视图、多语言等高级功能
5. **稳定性测试**: 大量测试确保软件稳定性和性能

### 未来规划

#### 短期目标
- [ ] 添加搜索过滤器（文件类型、大小、日期等）
- [ ] 实现搜索结果排序选项
- [ ] 增加更多文件预览功能
- [ ] 优化缩略图生成性能

#### 中期目标
- [ ] 支持正则表达式搜索
- [ ] 添加文件标签和收藏功能
- [ ] 实现插件系统
- [ ] 支持网络驱动器搜索

#### 长期目标
- [ ] 跨平台支持（Linux、macOS）
- [ ] 云存储集成
- [ ] AI 智能搜索建议
- [ ] 分布式搜索集群

---

## English Version

### Project Background

EverythingLike is an open-source project dedicated to providing Windows users with a fast file search experience. Inspired by the popular Everything file search tool, this project reimplements core functionality using the modern Rust programming language while adding additional practical features.

### Design Philosophy

- **Performance First**: Leverage Everything SDK and Rust's zero-cost abstractions for millisecond-level file searching
- **User-Friendly**: Provide intuitive interface and rich view modes to accommodate different user preferences
- **Modern**: Adopt async architecture and modern UI design principles for smooth user experience
- **Extensible**: Modular design makes it easy to add new features and customizations

### Core Technology

#### Rust Language Advantages
- **Memory Safety**: Prevents buffer overflows and memory leaks
- **Concurrency Safety**: Compile-time thread safety guarantees, avoiding data races
- **Zero-Cost Abstractions**: High-level features don't impact runtime performance
- **Cross-Platform Ready**: Foundation for potential future cross-platform expansion

#### Native Windows Integration
- **Win32 API**: Direct Windows native API calls for optimal performance
- **Everything SDK**: Integration with Everything's core search engine
- **System Themes**: Automatic adaptation to Windows system themes and fonts
- **File Associations**: Support for system default file opening methods

### Feature Highlights

#### Intelligent Search
- **Real-time Search**: Search as you type, no need to press Enter
- **Search Debouncing**: Smart delays to prevent excessive queries
- **Fuzzy Matching**: Support for partial matching and wildcards
- **Search History**: Remember frequently used search terms

#### Diverse Views
- **Details List**: File manager-like detailed information display
- **Icon Views**: Multiple icon size options (medium, large, extra large)
- **Thumbnails**: Support for image and video file thumbnail previews
- **Custom Columns**: Selectable file attribute columns

#### File Management
- **Quick Open**: Double-click or Enter to open files directly
- **Show Location**: Display file location in file manager
- **Copy Path**: Quick copy file path to clipboard
- **Context Menu**: Integrated Windows right-click context menu

### Technical Highlights

#### Async Architecture
```
UI Thread ──→ Search Request ──→ Search Thread
    ↑                              ↓
Search Results ←── Windows Message ←── Everything SDK
```

- **Non-blocking UI**: Search process doesn't affect interface responsiveness
- **Thread Safety**: Dedicated search thread handles SDK calls
- **Message Communication**: Windows message mechanism ensures safe inter-thread communication

#### Performance Optimizations
- **Virtual Scrolling**: Only render visible items for large result sets
- **LRU Cache**: Smart thumbnail caching reduces redundant generation
- **Parallel Processing**: Utilize multi-core CPU for parallel file operations
- **Memory Management**: Timely release of unnecessary resources

#### User Experience
- **Responsive Design**: UI elements adapt to window size changes
- **Keyboard Navigation**: Complete keyboard shortcut support
- **Multi-language**: Chinese and English interface switching
- **Configuration Persistence**: Remember user preference settings

### Development Journey

1. **Concept Phase**: Analyze Everything's pros and cons, determine improvement directions
2. **Prototype Development**: Build basic framework using Rust and Win32 API
3. **Core Features**: Implement basic functions like search, display, and navigation
4. **Optimization**: Add advanced features like thumbnails, multiple views, and multi-language
5. **Stability Testing**: Extensive testing to ensure software stability and performance

### Future Roadmap

#### Short-term Goals
- [ ] Add search filters (file type, size, date, etc.)
- [ ] Implement search result sorting options
- [ ] Add more file preview capabilities
- [ ] Optimize thumbnail generation performance

#### Medium-term Goals
- [ ] Support regular expression search
- [ ] Add file tagging and favorites functionality
- [ ] Implement plugin system
- [ ] Support network drive searching

#### Long-term Goals
- [ ] Cross-platform support (Linux, macOS)
- [ ] Cloud storage integration
- [ ] AI-powered intelligent search suggestions
- [ ] Distributed search cluster

### Contributing

We welcome contributions from the community! Whether it's:
- 🐛 Bug reports
- 💡 Feature requests
- 📝 Documentation improvements
- 🔧 Code contributions

Please feel free to open issues or submit pull requests on our GitHub repository.

### Acknowledgments

Special thanks to:
- **voidtools** for creating the amazing Everything software
- **The Rust Community** for building such a fantastic ecosystem
- **Microsoft** for the comprehensive Win32 API documentation
- **All Contributors** who have helped improve this project

---

*Last updated: December 2024* 