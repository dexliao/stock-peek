# stock-peek

一个基于 **Tauri 2 + Vue 3 + TypeScript** 的 A 股自选行情小窗。半透明、无边框、始终置顶，数据通过 Rust 侧的 **akshare-rs**（akshare 的纯 Rust 实现）获取。

> 不会安装或构建？请直接看 [INSTALL.md](INSTALL.md) 的分步教程。

## 已实现

- 正常尺寸 `320 × 272`，透明、无边框、圆角深灰背景；背景透明度可在 `30%～90%` 调节
- 默认外观伪装成 `todo.txt` 待办小窗：标题、待办列表和底部状态栏都是普通记事本样式
- 股票信息默认隐藏，只有鼠标悬停窗口时，才在底部以极淡灰字显示前 3 只摘要；移开即淡出
- 始终置顶，不显示在任务栏；启动后位于当前显示器工作区右下角，留 16px 边距
- 标题栏可拖动；`Ctrl + Shift + H` 是老板键，窗口失焦时也有效
- 老板键会把窗口缩成 `30 × 30` 灰色小圆点，再按一次恢复原来的大小和位置
- 鼠标穿透开关：开启后窗口不接收鼠标事件，点击会落到桌面或下层窗口；只能再用老板键恢复交互
- 系统托盘常驻图标；关闭窗口时隐藏到托盘，托盘菜单可“显示窗口”或“退出”
- 本地 `watchlist.json` 持久化自选股
- 通过标题栏 `⋯` 菜单进入自选股管理：按代码或名称模糊搜索、添加、修改成本价 / 股数、删除
- 每只股票展示：名称 / 代码、现价、成本价、相对成本价涨跌幅、浮动盈亏
- 股数默认 100 股，可在持仓设置中修改
- 刷新间隔可配置，默认 **10 秒**（5～60 秒）；只更新数值，不清空列表，因此刷新时不闪烁
- 管理列表固定显示 5 行，超过 5 只时上下滚动
- 所有文字、数值和状态均使用灰阶，没有红绿涨跌色

## 三层伪装

1. **默认伪装态**：界面是一个 `todo.txt` 待办小窗，股票数据不直接显示。鼠标悬停时底部才浮现一行极淡的行情摘要。
2. **老板键**：`Ctrl + Shift + H` 在正常窗口和 `30 × 30` 灰色圆点之间切换。热键由
   `tauri-plugin-global-shortcut` 在 Rust 侧注册，所以窗口失焦或鼠标穿透时仍有效。
3. **鼠标穿透**：从 `⋯` → 自选股 → 设置 → “鼠标穿透”开关启用。启用后设置面板会收起、底部显示“穿透模式”，窗口不拦截点击；再按 `Ctrl + Shift + H` 会关闭穿透并恢复交互，之后再按一次才会进入老板键圆点。

## 可调设置

点击 `⋯` 打开自选股，再点底部“设置”：

- **窗口透明度**：`30%～90%`，默认 `75%`；只影响窗口背景，不影响文字
- **刷新间隔**：`5～60` 秒，默认 `10` 秒
- **鼠标穿透**：开启后只能通过老板键恢复鼠标交互

设置保存在应用数据目录的 `settings.json`，首次启动自动生成：

```text
%APPDATA%\com.stockpeek.app\settings.json
```

```json
{
  "opacity": 0.75,
  "refreshSeconds": 10
}
```

## 数据层

数据依赖 [`akshare`](https://crates.io/crates/akshare) `0.1.14` 的 `equity` 特性。它是 akshare 的纯 Rust 实现，因此不需要安装 Python 或 `pip install akshare`。

- 现价：`AkShareClient::a_share_quote`（腾讯 / 新浪 / Tushare 回退）
- 搜索：`AkShareClient::a_share_search`（东方财富联想搜索）
- 并发拉取自选列表中的所有股票，单次请求超时 5 秒
- 前端按 `settings.json` 中的 `refreshSeconds` 定时刷新，默认 10 秒
- Rust 命令：`get_watchlist`、`search_stocks`、`add_watch_item`、
  `update_watch_item`、`remove_watch_item`、`refresh_quotes`、
  `get_window_flags`、`set_click_through`、`get_settings`、`update_settings`

计算规则：

```text
涨跌幅(%) = (现价 - 成本价) / 成本价 × 100
浮动盈亏  = (现价 - 成本价) × 股数
```

## 本地自选文件

应用首次启动时会在 Tauri 的应用数据目录创建 `watchlist.json`。Windows 下通常为：

```text
%APPDATA%\com.stockpeek.app\watchlist.json
```

文件结构：

```json
[
  {
    "code": "600519",
    "name": "贵州茅台",
    "costPrice": 1400.0,
    "shares": 100
  }
]
```

`shares` 是可选字段，缺失时按 100 股处理。`costPrice` 必须大于 0。

## 技术栈

| 部分 | 版本 / 说明 |
| --- | --- |
| Tauri | 2.x |
| Vue | 3.5+ |
| TypeScript | 6.x |
| Vite | 8.x |
| 行情 | `akshare` 0.1.14（纯 Rust） |
| 全局热键 | `tauri-plugin-global-shortcut` 2.x |
| 系统托盘 | Tauri 内置 `tray-icon` |
| 安装包 | NSIS（currentUser） |

## 环境要求

- Node.js `^20.19.0 || >=22.12.0`
- pnpm 9+
- Rust stable 1.90+
- Windows：Microsoft C++ Build Tools + WebView2 Runtime

## 开发与构建

```bash
pnpm install
pnpm tauri dev
```

仅构建前端：

```bash
pnpm build
```

## Windows 安装包

生产构建会生成 NSIS 安装包：

```bash
pnpm tauri build
```

输出位置：

```text
src-tauri\target\release\bundle\nsis\stock-peek_0.1.0_x64-setup.exe
```

也可以使用脚本（会检查 `pnpm` / `cargo`，安装依赖后构建并打印安装包路径）：

```powershell
.\scripts\build-windows.ps1
```

安装包配置在 [`src-tauri/tauri.conf.json`](src-tauri/tauri.conf.json) 的 `bundle` 段：

- `targets: ["nsis"]`
- 当前用户安装（`installMode: "currentUser"`），不要求管理员权限
- 应用窗口 `skipTaskbar: true`，运行时不显示在任务栏
- 系统托盘常驻图标；关闭窗口会隐藏到托盘，托盘菜单提供“显示窗口”和“退出”

如果本机没有 Rust/MSVC，也可以把项目放到 GitHub 仓库后运行
[`.github/workflows/build-windows.yml`](.github/workflows/build-windows.yml) 的
`build-windows-installer` workflow，下载 `stock-peek-windows-nsis` artifact。

安装包未做代码签名，首次安装可能触发 Windows SmartScreen 提示。

## 窗口配置

核心配置在 [`src-tauri/tauri.conf.json`](src-tauri/tauri.conf.json)：

```json
{
  "width": 320,
  "height": 272,
  "resizable": false,
  "decorations": false,
  "transparent": true,
  "alwaysOnTop": true,
  "skipTaskbar": true,
  "shadow": false,
  "visible": false
}
```

`visible: false` 是为了让 Rust 在定位和页面加载完成后再显示窗口：

- [`src-tauri/src/lib.rs`](src-tauri/src/lib.rs) 中的 `position_bottom_right` 使用显示器
  **work area**（会避开任务栏）计算右下角位置，并按 DPI 缩放 16px 边距。
- 页面加载完成后通过 `on_page_load` 显示窗口。
- 全局热键通过 Rust 侧插件注册，因此窗口失焦或鼠标穿透时仍然有效。
- 老板键圆点尺寸由 Rust 侧的 `BOSS_SIZE = 30.0` 控制，正常尺寸由
  `NORMAL_WIDTH` / `NORMAL_HEIGHT` 控制。

窗口拖动依赖 [`src-tauri/capabilities/default.json`](src-tauri/capabilities/default.json) 中的
`core:window:allow-start-dragging` 权限；标题栏使用 `data-tauri-drag-region` 标记拖动区域。

## 项目结构

```text
stock-peek/
├─ .github/workflows/
│  └─ build-windows.yml # Windows NSIS 安装包 CI
├─ scripts/
│  └─ build-windows.ps1 # 本地 Windows 打包脚本
├─ src/
│  ├─ App.vue           # todo 伪装、悬浮行情、自选管理、设置、穿透
│  └─ main.ts
├─ src-tauri/
│  ├─ capabilities/
│  │  └─ default.json   # 窗口拖动 / 事件监听权限
│  ├─ src/
│  │  ├─ lib.rs         # 行情、watchlist、settings、老板键、托盘、穿透
│  │  └─ main.rs
│  ├─ Cargo.toml
│  └─ tauri.conf.json   # 320×272 / 透明 / 无边框 / 置顶 / NSIS
├─ index.html
├─ package.json
└─ vite.config.ts
```

## 使用说明

- 标题栏 `⋯`：打开自选股管理；底部“设置”可调透明度 / 刷新间隔 / 鼠标穿透
- 默认界面是 `todo.txt` 待办，鼠标悬停窗口底部会出现极淡的行情摘要
- `Ctrl + Shift + H`：正常窗口 ↔ `30 × 30` 灰色圆点
- 开启鼠标穿透后：`Ctrl + Shift + H` 先关闭穿透并恢复交互；再按一次进入圆点模式
- `Alt + F4`：隐藏到系统托盘，不退出应用
- 托盘左键：显示窗口；托盘右键：显示窗口 / 退出
- 窗口不显示在任务栏，退出请使用托盘菜单“退出”
