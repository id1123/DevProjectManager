# Project Hub

本地开发项目管理器（`0.5.0-alpha`）。它把相关代码按“业务项目 → 项目”组织起来，集中保存项目路径、标签和 IDE 启动配置，适合在本机快速找到并打开 API、Web、App 或其他代码项目。

> 当前为 Alpha 版本，适合个人或小范围试用。数据和启动配置均保存在本机；尚未承诺跨设备同步、团队协作或生产环境级别的迁移兼容性。

## 当前功能

- 以业务项目组织多个项目，并支持项目描述、图标、颜色和收藏。
- 为业务项目和项目添加标签；主窗口支持多选标签的 **AND（交集）筛选**，选中的标签必须全部匹配。
- 主窗口支持按项目名称、标签或路径搜索，并提供收藏筛选。
- “最近打开”显示扁平的项目列表，不按业务项目分组；项目成功启动后会记录打开时间。
- 快速启动窗口支持全局快捷键。搜索结果以项目名称为优先匹配对象，并可继续按标签、路径或 IDE 查找和启动项目。
- 支持文件或文件夹作为项目目标路径；API 项目可保存 `.sln` 文件路径。
- 内置 Rider、WebStorm、Visual Studio、VS Code、HBuilderX 配置，也可以新增自定义 IDE、启动命令和参数模板。
- 支持通过 PATH 命令或可执行文件启动；VS Code 可使用 `code` 命令。
- 支持浅色/深色主题、SQLite 本地持久化，以及带版本号的 JSON 配置导入/导出。
- Windows 下由后端隐藏启动命令、PATH 检测和 IDE 探测的控制台窗口，避免启动时闪出 CMD 窗口。

## 技术栈

- [Tauri 2](https://tauri.app/) / Rust
- React 19 / TypeScript / Vite
- SQLite（`rusqlite` bundled）
- Fuse.js / Lucide React

## 开发环境

Windows：

- Node.js 20+
- Rust stable（MSVC toolchain）
- Microsoft C++ Build Tools
- WebView2 Runtime

macOS：

- Node.js 20+
- Rust stable
- Xcode Command Line Tools

安装依赖并启动桌面开发模式：

```bash
npm install
npm run tauri dev
```

只构建前端进行快速检查：

```bash
npm run build
```

构建当前平台的桌面安装包：

```bash
npm run tauri build
```

macOS 安装包需要在 macOS 上构建；正式分发时还需要按目标平台配置代码签名。构建命令会生成安装包，不是查看开发效果的必要步骤。

## 数据位置与安全

SQLite 数据库文件名为 `dev-project-manager.sqlite3`，保存在操作系统为应用分配的本地数据目录中。删除业务项目或项目只会删除管理器中的配置，不会删除真实代码文件。

启动配置和导出的 JSON 文件可能包含本机路径、命令及参数，请按本地配置文件对待，不要把敏感路径或凭据提交到公开仓库。应用不会把这些数据自动上传到远程服务。

## Alpha 说明

这是首个 `0.5.0-alpha` 版本，主要验证本地项目管理、标签筛选、最近打开和快速启动流程。升级前建议备份导出的 JSON 配置；数据库结构、配置格式和 UI 仍可能在后续版本调整。

## 许可证

本项目采用 [MIT License](LICENSE)。
