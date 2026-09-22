# Project Hub

本地开发项目管理器（`0.6.1`）。它把相关代码按“业务项目 → 项目”组织起来，集中保存项目路径、标签和 IDE 启动配置，适合在本机快速找到并打开 API、Web、App 或其他代码项目。

> 当前为早期版本，适合个人或小范围使用。数据和启动配置均保存在本机；尚未承诺跨设备同步、团队协作或生产环境级别的迁移兼容性。

## 当前功能

- 以业务项目组织多个项目，并支持项目描述、图标、颜色和收藏。
- 为业务项目和项目添加标签；主窗口支持多选标签的 **AND（交集）筛选**，选中的标签必须全部匹配。
- 主窗口支持按项目名称、标签或路径搜索，并提供收藏筛选。
- “最近打开”显示扁平的项目列表，不按业务项目分组；项目成功启动后会记录打开时间。
- 快速启动窗口支持全局快捷键。搜索结果以项目名称为优先匹配对象，支持中文项目名的全拼和拼音首字母，并可继续按标签、路径或 IDE 查找和启动项目。
- 应用仅运行一个实例；重复启动会直接显示已有主窗口。
- 关闭主窗口后常驻系统托盘；托盘菜单可直接打开最近使用的项目，也可按业务项目的二级菜单启动项目、恢复主窗口或退出程序。
- 全部项目支持详细/精简模式；精简模式仅显示项目名称和编辑入口。
- 新建项目时会根据 API、Web、App 等类型自动带入对应标签。
- 支持文件或文件夹作为项目目标路径；API 项目可保存 `.sln` 文件路径。
- 内置 Rider、WebStorm、Visual Studio、VS Code、HBuilderX 配置，也可以新增自定义 IDE、启动命令和参数模板。
- 支持通过 PATH 命令或可执行文件启动；VS Code 可使用 `code` 命令。
- 支持浅色/深色主题、SQLite 本地持久化，以及带版本号的 JSON 配置导入/导出。
- 设置页可从 GitHub Releases 检查、下载并安装签名更新。
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

## 发布与应用内更新

应用内更新从 GitHub Releases 获取版本信息和安装包。发布新版本时，先更新 `src-tauri/tauri.conf.json` 中的版本号并提交，再推送一个版本 tag（例如 `0.6.1` 或 `v0.6.1`）；`.github/workflows/release.yml` 会在 Windows runner 上自动构建 NSIS 安装包、生成 updater manifest，并创建 GitHub Release。也可以在 GitHub Actions 页面手动运行该工作流，并填写要发布的 tag。

仓库的 Actions Secrets 需要配置更新签名密钥：

- `TAURI_SIGNING_PRIVATE_KEY`
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`（仅当私钥设置了密码时需要）

对应的公钥写入 `src-tauri/tauri.conf.json` 的 updater 配置中。私钥只应保存在本机安全位置和 GitHub Actions Secrets，绝不能提交到仓库、README 或 Release 附件。GitHub Actions 使用仓库自带的 `GITHUB_TOKEN` 创建 Release；当前只发布 NSIS 包，以保持 Windows 安装与应用内更新流程一致。Release 保持为 GitHub 的正式 Release，这样固定的 `/releases/latest/download/latest.json` 地址才能持续提供最新更新清单。工作流会在上传完成后将清单中的 GitHub API asset 地址自动改为 Release 的 `browser_download_url`，避免客户端匿名调用 GitHub API 触发速率限制。

## 数据位置与安全

SQLite 数据库文件名为 `dev-project-manager.sqlite3`，保存在操作系统为应用分配的本地数据目录中。删除业务项目或项目只会删除管理器中的配置，不会删除真实代码文件。

启动配置和导出的 JSON 文件可能包含本机路径、命令及参数，请按本地配置文件对待，不要把敏感路径或凭据提交到公开仓库。应用不会把这些数据自动上传到远程服务。

## 版本说明

当前为 `0.6.1`，已覆盖本地项目管理、标签筛选、最近打开、托盘运行和应用内更新流程。升级前建议备份导出的 JSON 配置；数据库结构、配置格式和 UI 仍可能在后续版本调整。

## 许可证

本项目采用 [MIT License](LICENSE)。
