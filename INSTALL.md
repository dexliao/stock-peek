# stock-peek 安装教程

先说结论：**项目源码已经完成，但这台电脑上还没有生成 `.exe` 安装包**，因为缺少 Rust 和 Visual Studio C++ 编译工具。你需要先在一台 Windows 电脑上构建一次，之后就可以像普通软件一样双击安装。

下面给两种方法：

- **方法 A：在本机构建安装包**（推荐，之后使用最方便）
- **方法 B：用 GitHub Actions 云端构建**（本机不用装 Rust / Visual Studio，但需要 GitHub 账号）

如果你打不开 GitHub 或国外网站，先看下面的 **“国内网络安装”**。另外，安装包是独立的 `.exe`，其实可以请有开发环境的朋友帮你构建一次，把 `.exe` 发给你；你的电脑不需要装 Rust 和 Visual Studio。

---

## 国内网络安装（不使用 GitHub）

### 1. Node.js 用国内镜像

下载地址（约 31MB）：

<https://registry.npmmirror.com/-/binary/node/v22.20.0/node-v22.20.0-x64.msi>

下载后双击安装，保持默认选项。

安装完重新打开 PowerShell，执行：

```powershell
npm config set registry https://registry.npmmirror.com
npm install -g pnpm@11.28.5
```

### 2. Rust 用中科大镜像

> 如果还没装 Visual Studio Build Tools，请先跳到第 4 步把它装好，再回来装 Rust。

下载 `rustup-init.exe`：

<https://mirrors.ustc.edu.cn/rust-static/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe>

把它放到 `D:\rustup-init.exe`，然后在这个 PowerShell 窗口里执行：

```powershell
$env:RUSTUP_DIST_SERVER="https://mirrors.ustc.edu.cn/rust-static"
$env:RUSTUP_UPDATE_ROOT="https://mirrors.ustc.edu.cn/rust-static/rustup"
cd D:\
.\rustup-init.exe -y --default-host x86_64-pc-windows-msvc --default-toolchain stable
```

安装完成后 **关闭并重新打开 PowerShell**，检查：

```powershell
rustc -V
cargo -V
```

### 3. Cargo 依赖用 RsProxy 镜像

在 PowerShell 里执行（会创建/覆盖 `C:\Users\你的用户名\.cargo\config.toml`）：

```powershell
$cargoDir = "$env:USERPROFILE\.cargo"
New-Item -ItemType Directory -Force $cargoDir | Out-Null
@'
[source.crates-io]
replace-with = "rsproxy-sparse"

[source.rsproxy-sparse]
registry = "sparse+https://rsproxy.cn/index/"

[net]
git-fetch-with-cli = true
'@ | Set-Content "$cargoDir\config.toml" -Encoding utf8
```

### 4. Visual Studio Build Tools

这个只能在微软官方下载，通常国内可以访问，只是速度可能慢一些：

<https://aka.ms/vs/17/release/vs_BuildTools.exe>

安装时仍然要勾选：

- **使用 C++ 的桌面开发**
- **MSVC v143**
- **Windows 10/11 SDK**

如果微软下载非常慢或打不开，建议让能正常下载的朋友做一个离线安装目录，再把整个目录拷给你；不要随便下载网盘里的“VS 精简版/魔改版”。

### 5. 构建时让 Tauri 也走国内镜像

Tauri 打包 NSIS 安装包时会从 GitHub 下载工具。在 **同一个 PowerShell 窗口**里设置：

```powershell
$env:TAURI_BUNDLER_TOOLS_GITHUB_MIRROR_TEMPLATE="https://ghfast.top/https://github.com/<owner>/<repo>/releases/download/<version>/<asset>"
```

如果 `ghfast.top` 不通，可以换成：

```powershell
$env:TAURI_BUNDLER_TOOLS_GITHUB_MIRROR_TEMPLATE="https://gh-proxy.com/https://github.com/<owner>/<repo>/releases/download/<version>/<asset>"
```

然后构建：

```powershell
cd "D:\deepseek工作区\stock-peek"
pnpm config set registry https://registry.npmmirror.com
pnpm install
pnpm tauri build
```

> `<owner>`、`<repo>` 这些尖括号是 Tauri 的模板占位符，原样粘贴即可，不需要替换成具体名字。

### 最省事的办法

安装包是独立的，**你的电脑不需要开发环境**。

如果你有朋友、同事或淘宝上的开发者能运行 Tauri，把整个 `stock-peek` 项目发给他，让他在 Windows 上执行：

```powershell
pnpm install
pnpm tauri build
```

然后把下面这个文件发给你：

```text
src-tauri\target\release\bundle\nsis\stock-peek_0.1.0_x64-setup.exe
```

你拿到后直接双击安装即可。

---

## 方法 A：在本机构建安装包

### 第 1 步：安装 Visual Studio Build Tools

Tauri 在 Windows 上需要微软的 C++ 编译器和 Windows SDK。

1. 用管理员账号打开浏览器，下载：
   <https://aka.ms/vs/17/release/vs_BuildTools.exe>
2. 右键安装包，选择“以管理员身份运行”。
3. 在安装器里勾选工作负载：
   - **使用 C++ 的桌面开发**
   - 英文界面是 **Desktop development with C++**
4. 右侧确认包含：
   - **MSVC v143 - VS 2022 C++ x64/x86 生成工具**
   - **Windows 10 SDK** 或 **Windows 11 SDK**
5. 点击安装，等待 10～30 分钟。安装过程需要联网，磁盘至少留 8GB。

> 如果电脑没有管理员权限，先看方法 B。

### 第 2 步：安装 Rust

1. 打开 <https://rustup.rs/>
2. 下载 `rustup-init.exe`
3. 双击运行，直接按回车选择默认安装：
   - 默认工具链应为 `stable-x86_64-pc-windows-msvc`
4. 安装完成后关闭窗口。

如果安装时问工具链，选择：

```text
stable-x86_64-pc-windows-msvc
```

### 第 3 步：安装 Node.js 和 pnpm

1. 打开 <https://nodejs.org/en/download>
2. 下载 **LTS** 版本的 Windows 安装包（`.msi`），双击安装。
3. 安装时保持默认选项即可。
4. 关闭所有终端窗口，再重新打开 PowerShell。
5. 安装 pnpm：

```powershell
npm install -g pnpm@11.28.5
```

### 第 4 步：确认 WebView2

Windows 10/11 通常已经自带 Microsoft Edge WebView2 Runtime。

如果没有，下载安装：

<https://developer.microsoft.com/microsoft-edge/webview2/>

选择 **Evergreen Runtime** 安装即可。

### 第 5 步：检查环境

关闭所有 PowerShell，重新打开一个，执行：

```powershell
node -v
pnpm -v
rustc -V
cargo -V
```

应该能看到类似：

```text
v22.x.x
11.x.x
rustc 1.9x.x
cargo 1.9x.x
```

然后进入项目目录：

```powershell
cd "D:\deepseek工作区\stock-peek"
```

### 第 6 步：构建安装包

```powershell
pnpm install
pnpm tauri build
```

第一次构建要下载 Rust 依赖和 NSIS 打包工具，可能需要 10～30 分钟，属于正常现象。

构建成功后，安装包位置：

```text
D:\deepseek工作区\stock-peek\src-tauri\target\release\bundle\nsis\stock-peek_0.1.0_x64-setup.exe
```

也可以直接运行项目自带的构建脚本：

```powershell
cd "D:\deepseek工作区\stock-peek"
.\scripts\build-windows.ps1
```

如果 PowerShell 提示“禁止运行脚本”，改用：

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\build-windows.ps1
```

### 第 7 步：安装

1. 双击 `stock-peek_0.1.0_x64-setup.exe`
2. 如果 Windows 弹出 SmartScreen：
   - 点击“更多信息”
   - 点击“仍要运行”
3. 按提示完成安装。安装模式是当前用户安装，不需要管理员权限。
4. 从开始菜单启动 `stock-peek`。

---

## 方法 B：用 GitHub Actions 云端构建

适合没有管理员权限、或者不想在本机装 Visual Studio 的情况。

### 先在 GitHub 建仓库

1. 登录 GitHub。
2. 点右上角的 `+` → **New repository**。
3. 仓库名填 `stock-peek`。
4. 选择 **Private**（私有）或 **Public**（公开）都可以。
5. **不要**勾选 “Add a README file” 等初始化选项。
6. 点 **Create repository**。

> 注意：GitHub 首页绿色的 **Create project** 是“项目看板”，不是代码仓库，不要点它。

### 上传项目文件（网页版，不用 Git）

1. 打开刚创建的仓库。
2. 点击 **uploading an existing file**，或者 **Add file** → **Upload files**。
3. 在 Windows 文件资源管理器打开：
   `D:\deepseek工作区\stock-peek`
4. 按 `Ctrl + A` 全选文件夹里的内容，拖到 GitHub 的上传区域。
   - 是选中文件夹里的**内容**，不是把 `stock-peek` 文件夹本身拖进去。
   - 不要把 `node_modules`、`dist`、`src-tauri\target` 上传；当前项目里这些已经清理掉了。
5. 等文件列表加载完，底部填写提交说明，例如 `initial commit`。
6. 点 **Commit changes**。

上传后检查仓库根目录是否有 `.github` 文件夹。没有它，Actions 里不会出现打包按钮。

如果 `.github` 没上传成功：

1. 仓库页面点 **Add file** → **Create new file**
2. 文件名填：
   `.github/workflows/build-windows.yml`
3. 用记事本打开本机的
   `D:\deepseek工作区\stock-peek\.github\workflows\build-windows.yml`
4. 把内容全部复制到 GitHub 编辑器里
5. 点 **Commit changes**

### 运行云端打包

1. 打开仓库的 **Actions** 标签。
2. 左侧选择 `build-windows-installer`。
3. 点右侧 **Run workflow** → 再点 **Run workflow**。
4. 等待 10～30 分钟。黄色转圈表示正在构建，绿色勾表示成功。
5. 点进这次运行，页面底部 **Artifacts** 下载：
   `stock-peek-windows-nsis`
6. 解压下载的 zip，得到 `stock-peek_0.1.0_x64-setup.exe`。
7. 双击安装。

如果 Actions 页面没有 **Run workflow** 按钮：

- 确认工作流文件在默认分支（通常是 `main`）
- 打开仓库 **Settings** → **Actions** → **General**，确认允许运行 Actions

构建失败时，把红色步骤最后的日志截图发出来，通常能直接定位到 Rust 或 Tauri 配置问题。

### GitHub Actions 文件

项目里已经包含工作流文件：

```text
.github/workflows/build-windows.yml
```

---

## 安装后的使用

- 程序启动后会在屏幕右下角显示小窗。
- **不会出现在任务栏**，这是正常的；它只保留系统托盘图标。
- 托盘图标可能被 Windows 折叠到 `^` 里，点开就能看到。
- 左键托盘图标：显示窗口。
- 右键托盘图标：`显示窗口` / `退出`。
- `Alt + F4`：隐藏到托盘，不会退出。
- 真正退出请用托盘菜单的“退出”。

### 添加自选股

1. 点击窗口标题栏右侧的 `⋯`
2. 点击 `+ 添加`
3. 输入代码或名称，例如 `600519` 或 `茅台`
4. 选择搜索结果
5. 填写成本价和股数（默认 100 股）
6. 点击“加入自选”

### 设置

`⋯` → 自选股 → 底部“设置”：

- 窗口透明度：30%～90%
- 刷新间隔：5～60 秒，默认 10 秒
- 鼠标穿透：开启后只能按 `Ctrl + Shift + H` 恢复交互

### 老板键

`Ctrl + Shift + H`

- 正常窗口 → 30×30 灰色小圆点
- 再按一次 → 恢复正常窗口

开启鼠标穿透时，按一次会先恢复鼠标交互；再按一次才进入圆点模式。

### 数据文件

```text
%APPDATA%\com.stockpeek.app\watchlist.json
%APPDATA%\com.stockpeek.app\settings.json
```

---

## 常见问题

### 1. `cargo` 或 `rustc` 不是内部或外部命令

Rust 没装好，或者装完没有重新打开 PowerShell。

重新打开终端，执行：

```powershell
rustc -V
cargo -V
```

如果还是没有，重新运行 `rustup-init.exe`。

### 2. `link.exe not found` / 找不到链接器

Visual Studio Build Tools 没装完整。

打开 **Visual Studio Installer** → 修改 → 确认勾选：

- 使用 C++ 的桌面开发
- MSVC v143
- Windows 10/11 SDK

### 3. `pnpm` 不是内部或外部命令

执行：

```powershell
npm install -g pnpm@11.28.5
```

然后关闭并重新打开 PowerShell。

### 4. `pnpm install` 很慢或失败

可以临时换国内镜像：

```powershell
pnpm config set registry https://registry.npmmirror.com
pnpm install
```

构建成功后可以改回：

```powershell
pnpm config set registry https://registry.npmjs.org
```

### 5. `pnpm tauri build` 下载 NSIS 失败

这是 Tauri 在下载 Windows 安装包制作工具，通常重试一次即可：

```powershell
pnpm tauri build
```

如果一直失败，检查网络/代理后重试。

### 6. 安装后找不到窗口

先看屏幕右下角托盘区域，点 `^` 展开隐藏图标。

如果还是没有，按一次 `Ctrl + Shift + H`。

### 7. 窗口不显示在任务栏

这是设计行为。应用使用 `skipTaskbar`，只保留托盘图标。

### 8. 想卸载

Windows 设置 → 应用 → 已安装的应用 → 找到 `stock-peek` → 卸载。

自选股和设置文件在 `%APPDATA%\com.stockpeek.app`，如不需要可手动删除。

---

## 最省事的做法

如果你能在这台电脑上装好 **Visual Studio Build Tools + Rust + Node.js + pnpm**，装好后告诉我，我可以继续帮你执行构建，并排查具体报错。
