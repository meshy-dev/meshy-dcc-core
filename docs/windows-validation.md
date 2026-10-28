# Windows 验证：给 Windows 上的 Claude Code session

这份文档让一台 Windows x64 机器上的新 Claude Code session 拿到背景、接着做 Windows 验证。
在 `meshy-dcc-core` 仓库根目录启动 Claude Code，第一句可以是：

> 读 `docs/windows-validation.md`，按它做 Windows 验证。先检查环境并告诉我缺什么，装软件前先问我。

本轮是第二轮（0.2.2），重点在第 6 节；第 4、5 节是每轮都走的基本流程。

## 1. 背景（一屏）

- **Meshy DCC Core**：一个本地 CLI（`meshy-dcc`）+ 常驻 gateway（`meshy-dcc-server`，端口 19765）+ 编进 CLI 的 agent Skill。
  每个 DCC 软件装一个 Meshy 连接器（adapter），实例注册到本机，agent 通过 CLI 点名实例调用工具。
  定位、架构、命令见 `README.md`、`docs/plan.md`、`skills/meshy-dcc/SKILL.md` 与 `skills/meshy-dcc/references/`。
- **两个仓库**（都在 `meshy-dev`，private）：
  - `meshy-dcc-core`（本仓库）：Rust core、Python wheel `meshy_dcc_core`、`meshy-dcc-catalog.yml`（内置安装目录）、release workflow。
  - `meshy-dcc-adapters`：14 个宿主的 adapter，一个宿主一个目录 `meshy-dcc-<host>/`，`scripts/build_release.py <host>` 打包。
- **现状**：0.2.1 已发布（2026-10-27：npm、CDN、GitHub Release）。main 上是 0.2.2 候选，版本号已是 0.2.2，还没发布。
  13 个连接器已发布（catalog 里 `available`），0.2.2 发布后会全部用 0.2.2 的 wheel 重新打包发布。
  第一轮 Windows 验证（2026-10-18）的结果在文末，那一轮的 core 与 adapter 修复都已合入。
- **0.2.2 改了什么**（详见 `CHANGELOG.md` 的 Unreleased）：
  - rmcp 2.1、PyO3 0.29 安全升级（连接器里的 MCP 服务端也换了）；
  - 一次 `update apply --yes` 更新 CLI 和 server 并重启 gateway；`meshy-dcc gateway restart`；窗口里的"立即更新"；
  - 超时的主线程调用不再晚执行（没开始的丢弃，已在跑的返回 `status: unknown`）；`doctor` 只读；
  - 匿名使用统计（默认开，`docs/telemetry.md`）；bug 报告（`meshy-dcc report`、窗口里的 Report a problem）；
  - 连接器：Maya sidecar 默认不开局域网端口、FreeCAD 子进程非零退出不写回用户文件、3ds Max `3dsmax-mesh-ops` 能加载、
    Blender `save_scene` 读回、Houdini 导入如实报告丢失的材质。
- 正式发布由 CI 在 `windows-latest` 上用 MSVC 原生构建（`.github/workflows/release.yml` → `scripts/release/build_release.sh`），
  发布前 CI 还会在 Windows runner 上从上一版升级一次（`scripts/release/upgrade_smoke.sh`）。本机验证同样原生构建，走和 CI 一样的路径。
- **目标**：找出并修掉 Windows 上的问题，给出"哪些 Windows × 宿主组合真机通过"的清单。只有真机通过的组合才进支持矩阵。

## 2. 规则（必须遵守）

- 本仓库的 `CLAUDE.md` 是总规则。另外：
- **不 push、不开 PR、不发布、不上传**，除非用户明确要求。代码修复在本地分支 `windows/<主题>` 上提交；core 和 adapters 的提交分开。
- 只碰测试自己建的工程和进程；**不打开、不修改用户已有的工程和软件偏好**。给进程发信号前核对 PID 和命令行。
- 隔离：测试时设 `MESHY_DCC_REGISTRY_DIR`、`MESHY_DCC_DATA_DIR`、`MESHY_DCC_GATEWAY_ADMIN_DB` 到测试目录；
  要用非默认端口时，`MESHY_DCC_GATEWAY_PORT`（gateway 监听）和 `MESHY_DCC_BASE_URL=http://127.0.0.1:<端口>`（CLI 连哪个 gateway）**两个都要设**。
  先确认 19765 上没有用户自己的 gateway（`Get-NetTCPConnection -LocalPort 19765 -State Listen`）。
- 宿主会把偏好写进用户目录的（Documents、`%APPDATA%`、`%LOCALAPPDATA%`），测前测后列清单对比，测试产生的东西列出来给用户确认后再删。
- **宿主的用户目录要用宿主自己的变量隔离，只改 `HOME`/`USERPROFILE` 不够**（2026-10-27 在 macOS 上，Blender 无视 HOME，
  连接器装进了用户真实的扩展目录）：Blender `BLENDER_USER_RESOURCES`、Houdini `HOUDINI_USER_PREF_DIR`、Maya `MAYA_APP_DIR`、
  FreeCAD `FREECAD_USER_HOME`/`FREECAD_USER_DATA`/`FREECAD_USER_TEMP`、Painter `MESHY_DCC_SUBSTANCE3D_PAINTER_PROFILE`。
  **安装前先用宿主自己的只读探测确认它会写到哪里**（例如 Blender：
  `blender --background --python-expr "import bpy; print(bpy.utils.resource_path('USER'))"`）。
- **不往线上发任何数据**：release 构建的匿名统计默认开启，测试环境一律设 `MESHY_DCC_TELEMETRY=0`；
  只在专门测统计时用 `MESHY_DCC_TELEMETRY_HOST=http://127.0.0.1:<端口>` 指向本地接收端。bug 报告用
  `MESHY_DCC_REPORT_URL=http://127.0.0.1:<端口>/` 指向本地接收端（发到线上会生成真实的 GitHub issue）。
- `MESHY_DCC_UPDATE_MANIFEST_URL` 指向非官方地址时不校验签名，只用于测试，不要写进任何文档给用户。
- 装软件（DCC、工具链）前先问用户。付费软件（Maya、3ds Max、Houdini、Cinema 4D、Substance、ZBrush）只在机器上已有时测。
- 不调用 Meshy API、不花 credits。

## 3. 环境

需要（缺的先问用户再装，`winget` 即可）：

| 工具 | 用途 |
|---|---|
| Git for Windows（含 Git Bash） | `build_release.sh` 在 Git Bash 里跑，和 CI 一致 |
| GitHub CLI `gh`（已登录、能读 `meshy-dev` 私有仓库） | 克隆两个仓库 |
| Visual Studio 2022 Build Tools：C++ 桌面开发 / 游戏开发 C++、MSVC v143、Windows 10/11 SDK | Rust MSVC 目标；Unreal 安装时编 C++ 模块 |
| rustup（仓库的 `rust-toolchain.toml` 固定 Rust 1.95） | core |
| Node.js 22 | 构建时打包 admin UI |
| Python 3.11 与 `uv` | wheel（maturin）、adapter 打包、测试脚本 |

```powershell
gh repo clone meshy-dev/meshy-dcc-core
gh repo clone meshy-dev/meshy-dcc-adapters
```

工作目录放短路径、不带空格（例如 `C:\mdcc\`）；另建一个**带空格和中文**的测试目录专测路径问题（例如 `C:\mdcc test\测试\`）。

## 4. 第一关：core 本身

在 Git Bash 里，和 CI 同一条命令：

```bash
cd meshy-dcc-core
scripts/release/build_release.sh out      # out/meshy-dcc.exe, out/meshy-dcc-server.exe, out/meshy_dcc_core-0.2.2-cp38-abi3-win_amd64.whl
cargo test -p meshy-dcc --no-fail-fast    # 记录 Windows 专有失败
```

检查（PowerShell，带 `--output json` 看结构化结果）：

- [ ] `out\meshy-dcc.exe --version`、`doctor`：能跑；SmartScreen / 杀毒有没有拦未签名程序（记录原话）。
- [ ] gateway：`meshy-dcc gateway start` 或任一需要 gateway 的命令会拉起它；`health`；`gateway stop`。防火墙对 loopback 是否弹窗。
- [ ] `meshy-dcc ui`：浏览器打开 `/admin`，四种语言切换、连接器页能列出宿主（experimental 有角标）。
- [ ] wheel：用一个**官方 MSVC 版 CPython 3.11**（python.org 或 `uv python install 3.11`）建 venv，装 wheel，`python -c "import meshy_dcc_core"`。
- [ ] `meshy-dcc agent-skill`（只看计划，不加 `--execute`；要写入用户的 agent 目录先问用户）。
- [ ] 路径：把 `out` 拷到带空格和中文的目录再跑一遍上面的命令。

一行安装（`scripts/install-cli.ps1`）要等正式发布后在真实 CDN 上验证，这里可选：只做 PowerShell 语法检查。

## 5. 第二关：连接器（有哪个宿主测哪个）

先列出机器上装了哪些 DCC（以及版本），和用户确认测试范围。免费、可直接装的：Blender、Godot、FreeCAD、OpenSCAD、Material Maker、
Roblox Studio（需登录）、Unity（Personal 许可）、Unreal（Epic Launcher，较重）。

**打包**（用第一关 `out` 里的 win_amd64 wheel；每个要测的宿主一条）：

```bash
cd meshy-dcc-adapters
uv run --python 3.11 --no-project python scripts/build_release.py blender --platform windows-x64 --core-wheel-dir ../meshy-dcc-core/out --out <builds>
```

这些打包脚本至今只在 macOS 上跑过（交叉打 Windows 包）；在 Windows 上跑出的问题也算本次要修的。

**本地 catalog 与包服务**（`local_test_catalog.py` 在 core 仓库）：

```powershell
uv run --python 3.11 --no-project --with pyyaml python scripts\release\local_test_catalog.py --builds <builds> --base-url http://127.0.0.1:8765/ --out <test>\catalog.json
python -m http.server 8765 --bind 127.0.0.1 --directory <builds>     # 另一个窗口
```

**每个宿主走一遍**（宿主专属说明先读 `skills/meshy-dcc/references/<host>.md`）：

1. `meshy-dcc --gateway local --no-auto-gateway --output json install --dcc-type <host> --catalog <test>\catalog.json [--project <测试工程>] [--dcc-path <exe>] --execute`
2. `meshy-dcc --output json start-instance --dcc-type <host> [--project <测试工程>] --wait-ready --yes`
3. 经 gateway：`list`、`load-skill`、`call` 一个只读工具、导入一个模型、读回（名字、几何、材质贴图）。
4. 保存 → `stop-instance`（有未保存时应被拒）→ 再 `start-instance` → 读回保存的内容。
5. `stop-instance --discard-unsaved`，确认宿主和 sidecar 都退出、端口释放。
6. 卸载（`references/connectors.md` 的 Remove 表）后，对比宿主用户目录的测前测后清单。

测试模型：用户给的 Meshy 模型（GLB/FBX）；没有就用 Khronos 的 `DamagedHelmet.glb`（glTF-Sample-Assets，带 PBR 贴图）。

**Windows 专项检查**（之前 macOS 上看不到的）：

- 通用：包内 `python\python.exe` 在没装 Python 的环境能起并导入 `_core.pyd`；sidecar 不弹控制台窗口；宿主退出后 sidecar 退出；路径带空格、`%`、`&`、中文。
- Blender：`--dcc-path` 指向 `blender.exe`；从磁盘装扩展。
- Unity：`.cmd` 安装与启动（`cmd.exe /d /s /c` 引号）；pid 文件记 `python.exe`；FBX 导入读回。
- Godot：设 `MESHY_DCC_GODOT_EXECUTABLE` 或 PATH 有 godot；插件直接起包内 `python.exe`；`--watch-pid` 退出。
- Unreal：VS 2022 能编出模块（带空格路径）；工程路径过长（约 90 字符）撞 MAX_PATH；编辑器开着时安装回滚并报 30；UE 自带 Python 能导入 `meshy_dcc_core`。
- FreeCAD：安装步骤用 `python\python.exe`（不用 `.cmd`）、服务用 `pythonw.exe`；`dcc_paths` 里 `FreeCAD.exe` 的大小写。
- OpenSCAD、Material Maker：Windows 默认安装位置未核实，找不到时用 `--dcc-path`，把实际位置记下来改 catalog 的 `dcc_paths`。
- Roblox Studio：Studio 在 `%LOCALAPPDATA%\Roblox\Versions\*\RobloxStudioBeta.exe`；插件写 `%LOCALAPPDATA%\Roblox\Plugins\MeshyDcc.lua`（测后删）；首启可能自更新导致超时（见 reference）。
- 3ds Max（只有 Windows，若机器上有）：启动脚本写 `%LOCALAPPDATA%\Autodesk\3dsMax\<年> - 64bit\<语言>\scripts\startup\`；该用户没启动过的版本会被拒；进程内 server 与 Qt timer 主线程派发。
- Maya / Houdini / Substance 3D Painter（若有）：用户 Documents 下的 modules / packages / python\startup，注意 OneDrive 重定向；Houdini 的 `HOME` 规则。

## 6. 第二轮：0.2.2 重点

按第 4、5 节走完构建、测试和连接器六步（所有能测的宿主都要重跑：连接器里的 MCP 服务端随 wheel 换成了 rmcp 2.1），另外逐项确认：

**更新：Windows 上第一次真实升级（最重要）**

1. 用官方安装脚本把**已发布的 0.2.1** 装进测试目录：
   `& ([scriptblock]::Create((irm https://cdn.meshy.ai/meshy-dcc-core/install.ps1))) -Version 0.2.1 -InstallDir <测试目录>`
   （其余 meshy-dcc 路径照第 2 节隔离，gateway 用非默认端口）。
2. 用第 4 节构建出的 0.2.2 做一份本地更新清单：`scripts/release/generate_update_manifest.py` 生成，下载地址改成本机 loopback 包服务，
   再把 `MESHY_DCC_UPDATE_MANIFEST_URL` 指向它（参考 `scripts/release/upgrade_smoke.sh` 和 release workflow 的 upgrade-smoke 任务）。
3. 在 Git Bash 里跑 `scripts/release/upgrade_smoke.sh --bin-dir <测试目录> --manifest-url <本地清单> --version 0.2.2 --port <端口>`，
   或者手动按它的步骤走。逐项确认：
   - `update apply --yes`（0.2.1 的代码）暂存 CLI 和 server；**之后的第一条命令有自己的输出和退出码**
     （relaunch 在 Windows 上是等待子进程加 job object，这是第一次实测）；
   - 0.2.1 不会重启 gateway：`meshy-dcc gateway restart` 后，`meshy-dcc --version`、`meshy-dcc-server --version` 和正在运行的 gateway
     （`/admin/api/health` 的 version）都是 0.2.2；
   - 运行中的 `meshy-dcc-server.exe` 被正确替换（Windows 不能覆盖运行中的 exe）；任务管理器里没有残留的旧进程；没有控制台窗口闪出。
4. 有时间的话，再在本地改出一个 0.2.3 标签的构建、做第二份清单，测 **0.2.2 自己的** 一步更新：一次 `update apply --yes` 应该同时
   更新两个程序并自动重启 gateway；窗口首页出现"立即更新"，点了以后显示"正在更新"，gateway 恢复后页面刷新为新版本。

**core 的新行为**

- `meshy-dcc gateway restart`（`gateway daemon restart` 的别名）；`gateway stop` 后立刻 `start` 不再等 30 秒。
- 超时的主线程调用：挑一个宿主（Blender 或 Maya），调用一个慢工具并给很短的超时，结果应是 `status: unknown`（`outcome-unknown`）；
  之后读回场景，确认场景状态和报告一致，超时后没有悄悄继续改动。
- `doctor` 只读：起一个宿主后强杀它，`doctor` 把它列在 `inventory.dead`，registry 文件前后字节一致（Windows 的只读锁探测第一次实测）。
- 窗口：侧边栏在窄窗口里不随长页面滚走；四种语言下新加的文案（更新、统计横幅和开关、Report a problem）没有溢出。

**连接器的修复**

- Maya：`MESHY_DCC_MAYA_SIDECAR=1` 且不设远程变量时，sidecar 命令行带 `--gateway-remote-port 0`，59765 没有监听；显式设了 host/port 才有。
- 3ds Max：`load-skill 3dsmax-mesh-ops` 注册 25 个工具；`create_box` 的长宽高读回正确。
- Houdini：导入 Meshy 导出的 USD/USDZ 和一个双材质 GLB，看 `materials_detected`、`materials_imported`、`materials_lost` 和 status 是否如实。
- FreeCAD：写操作返回 `exit_code: 0`，新的"非零退出不写回"没有误伤正常写入。
- Blender：`save_scene` 返回 `postcondition.verified: true`；保存到只读目录时报错且没有写出文件。

**统计与报告（都发到本地接收端）**

- 统计：首次运行的提示只出现一次；`meshy-dcc telemetry status|preview|off|on|reset`；窗口横幅（好的 / 关闭）和设置开关；
  `DO_NOT_TRACK=1` 生效且设置页说明原因。在宿主上调用过几个工具后看 `telemetry preview`：`tool_calls[].skill` 应是真实的技能名
  （例如 `blender-scene`），不应全是 `custom`（取决于连接器上报 `rankPathSource: bundled`）；预览里不能出现路径、工程名、实例 ID。
- 报告：`meshy-dcc report --summary "..."` 只预览；加 `--yes` 发到本地接收端；窗口里的 Report a problem 走同样流程；
  报告内容里没有本机路径、端口、PID、实例 ID、主机名。

## 7. 产出

- 问题：能修的在 `windows/<主题>` 分支修并加测试；修不了的写清复现步骤。
- 结果：在本文末尾"结果"一节追加（Windows 版本、每个宿主的版本与六步结果、专项检查结论、改了什么、待决定的事）。
  **不写本机用户名、绝对路径、实例 UUID**。
- 结束时告诉用户：哪些 Windows × 宿主组合可以进支持矩阵、有哪些分支待 push。

## 结果

### 第一轮（2026-10-18）

环境：Windows 11 Home China 10.0.26200 x64；VS 2022 Community 17.13（MSVC 14.43、Win SDK 22621）；Rust 1.95.0；
Node 22；官方 CPython 3.11.9；Git Bash。用户目录本身带中文；测试工程放在带空格和中文的目录。
隔离：registry、data、admin DB、gateway 端口都指到测试目录；Blender 用 `BLENDER_USER_RESOURCES`，Houdini 用 `HOUDINI_USER_PREF_DIR`。
包用本机构建的 win_amd64 wheel 打，经本地 catalog 与 loopback 包服务安装。

#### 第一关：core

| 检查 | 结果 |
|---|---|
| `build_release.sh`（Git Bash + MSVC） | ❌→✅ 三处 Windows 问题，修在 `windows/build`（见下） |
| `cargo test -p meshy-dcc` | ✅ 450 passed、9 ignored；两个自动起 gateway 的测试修复前各跑 600 s 以上（见下），修复后 3 s / 7 s |
| `--version`、`doctor` | ✅；本地构建无 MOTW，加 Zone.Identifier 后从终端运行不弹 SmartScreen；本机 Smart App Control 关闭，Defender 无检出 |
| gateway start / health / status / stop | ✅ 只绑 127.0.0.1（不触发防火墙）、无控制台窗口、stop 后进程与端口都释放、无残留进程 |
| `ui` | ✅ 打开默认浏览器 `/admin`；四种语言切换；连接器页列出 23 个宿主（发布前全部"即将推出"，experimental 角标在此状态下看不到） |
| wheel | ✅ 官方 MSVC CPython 3.11 venv 中 `import meshy_dcc_core` 加载 MSVC 版 `_core.pyd`；`pip install` 要加 `--no-deps`（元数据依赖未发布的 `meshy-dcc-server`，与平台无关） |
| `agent-skill`（只看计划） | ✅ 识别 8 个 agent；路径分隔符混用（`.agents/skills\meshy-dcc`），仅显示问题 |
| 带空格、中文、`&`、`%` 的目录 | ✅ 同目录找到 gateway、自动起 gateway、管道不挂、stop 干净 |
| `install-cli.ps1` | ✅ PowerShell 5.1 语法检查通过，纯 ASCII、无 BOM |

core 修复（分支 `windows/build`）：

- `ccdd2bb` 原生 Windows 构建：admin UI 的 `build.rs` 直接起 `npm`（Windows 上是 `npm.cmd`，构建在编译前就失败，CI 的 windows-latest 同样会失败）；
  `build_release.sh` 调 `python3`（Windows 的 Python 没有）；aws-lc 的 C 断言把 CARGO_HOME 绝对路径（含用户名，窄、宽字符串，且用 8.3 短路径编译）
  编进 exe 和 `_core.pyd`，改为给 MSVC 传 `/d1trimfile`；scrub 在 Windows 上也查 8.3 短路径和 UTF-16（原先 CI 的 `C:\Users\RUNNER~1` 会漏检）。
- `ae45b12` **自动起的 gateway、start-instance 起的 DCC 继承调用方的 stdout/stderr 管道**：Windows 子进程继承父进程所有可继承句柄，
  agent 读 `meshy-dcc list` 的输出要等到 gateway 退出（实测 10 分钟以上，agent 侧直接超时）。CLI 启动时把自己的标准句柄设为不可继承，
  detached 启动点（gateway-ensure、sidecar daemon）同样处理；回归测试在输出 60 s 不关时失败。
- 分支 `windows/doctor`：`doctor` 的 `--gateway-host`、`--gateway-port` 写死了默认值 127.0.0.1:19765，覆盖了按 `--base-url`、
  `MESHY_DCC_GATEWAY_PORT` 推出的网关，隔离测试时 `gateway.status` 报的是用户的 19765（与平台无关）。

#### 第二关：连接器

14 个 adapter 在 Windows 上都能用 `build_release.py --platform windows-x64` 打包；scrub 无本机路径。adapter 修复都在 `meshy-dcc-adapters` 的 `windows/<宿主>` 分支。

| 宿主（版本） | 1 安装 | 2 启动 | 3 调用、导入、读回 | 4 保存、停止、重开 | 5 停止 | 6 卸载、残留 |
|---|---|---|---|---|---|---|
| OpenSCAD 2021.01 | ✅ `dcc_paths` 命中 | ✅ 4 s | ✅ Meshy STL 写成 50 mm 圆底座、导出 STL、渲染 PNG | ✅ 文件即保存；重开 inspect_source | ✅ | ✅ 用户目录无残留 |
| FreeCAD 1.1.4 | ✅ glob 不区分大小写，命中 `freecad.exe` | ✅ 3 s，pythonw 无窗口 | ✅ 中文名文档、导入 Meshy GLB（9998 面，m→mm，Y-up→Z-up） | ✅ | ✅ | ✅ 无残留 |
| Material Maker 1.7（winget zip） | ✅ 需 `--dcc-path`（`DCC_PATH_REQUIRED` 提示清楚） | ✅ | ❌→✅ 每次写图都报 WinError 32、版本为 unknown（修复见下）；Meshy 底色 + 法线导出 Godot 2048² | ✅ | ✅ | ✅ Material Maker 自己写 `%APPDATA%\material_maker_2` |
| Blender 5.1 | ✅ `--dcc-path`（自定义目录） | ✅ 5 s | ✅ 中文路径 GLB；PBR 四路贴图都连上 | ✅ 中文 .blend；未保存时 stop 被拒（exit 7）；discard 后重开读回正确；`list` 的 scene 心跳后更新 | ✅ | ✅ 被占用的 `_core.pyd` 由 Blender 改名 `.~stale~`、下次启动删除；用户配置未动 |
| Godot 4.7.2（winget zip） | ⚠️ 需设 `MESHY_DCC_GODOT_EXECUTABLE`（Windows 可执行名永远不叫 godot）；preflight 原因被吞（已修） | ✅ 13 s；sidecar 由编辑器起、无窗口 | ❌→✅ 每个 session 调用与 safe stop 都失败（`D:\` 与 `D:/` 字符串比较，已修）；导入后 StandardMaterial3D 三路贴图 | ✅ 有未保存时 stop 被拒；discard；重开读回 | ✅ 2 s；宿主被杀后 sidecar 1 s 退出 | ⚠️ Remove 表漏了 `[autoload] MeshyDccRuntimePeer`（已补）；Godot 首启写 `%APPDATA%\Godot`、`%LOCALAPPDATA%\Godot` |
| Unity 2022.3.55f1c1（编辑器在另一盘、Hub 手动定位） | ❌→✅ 只找 Program Files（已修：读 Hub 的 editors-v2.json） | ✅ 27 s | ✅ FBX；Meshy 底色 + 法线（PNG）→ Standard 材质 verified | ✅ 中文场景名；dirty 时 stop 被拒；discard；重开读回 | ✅ | ✅ packages-lock 留一条 `ai.meshy` 记录（下次 resolve 清掉）；Unity 把工程加入最近列表 |
| Unreal 5.7.4 | ✅ VS 2022 编模块 88–90 s（带空格路径；112 字符工程路径正常）；⚠️ 中文工程路径 UBT/MSVC 编不过（Unreal 自身限制），安装器报 20 并附日志、已回滚 | ✅ 45 s | ✅ 中文源路径 GLB → Nanite StaticMesh + 材质 + 3 张贴图；inspect_model、capture_model | ❌→✅ 移动/挂接 actor 不置脏，safe stop 放行并丢掉了未保存的移动（已修）；修复后 stop 被拒、discard 后重开为已保存值 | ✅ 4 s | ✅；编辑器开着时装新文件 → 回滚 exit 30 ✅ |
| Houdini 21.0.440 FX（自定义目录） | ❌→✅ 路径里没有版本号（已修：读 houdini.exe 版本资源）；偏好隔离 | ✅ 26 s，`uiready.py` 自启，Houdini 自带 Python 加载 MSVC `_core.pyd` | ❌→✅ GLB 走 File SOP 根本读不了却报成功（已修：改用 GLTF SOP，读不了即失败）；MaterialX PBR 接 Meshy 贴图并 assign | ✅ 中文 .hip；有未保存时 stop 被拒、保存后放行；discard；重开读回几何与材质 | ✅ `hou.exit` 2 s 无保存框 | ✅ uninstall `removed`；用户 Documents 未动 |
| Roblox Studio 0.743（启动时从 0.710 自更新） | ✅ 插件写入 Studio 本地 Plugins | ⚠️ 首次自更新 + 登录超过等待（reference 已写）；之后 13–14 s | ✅ Meshy GLB → 9998 三角、四张贴图 | ✅ stop 总是 unsaved_changes（按设计）；discard 1–2 s；重开后导入的模型消失（按设计） | ✅ | ✅ `uninstall --plugin` 删除插件；无 AutoSaves |
| ZBrush 2026.2.1 | ⛔ 签名门禁按设计拒绝：本机 ZBrush.exe Authenticode `HashMismatch`（签名者 Maxon，内容被改过）；经用户同意，仅测试时替换签名探测（不改代码）后安装 | ⚠️ 首次启动 ZBrush 经 Explorer 自我重启、丢掉 `-script`，start-instance 超时；之后 16–66 s ✅ | ❌→✅ `get_session_info` 总是失败（已修）；Meshy OBJ 导入 9998 面；`list` 原为 zbrush@unknown（已修） | ✅ 中文 .zpr；再次保存 `FILE_EXISTS`；save_tool；不带 discard 的 stop 被拒（按设计）；open_project 读回 | ✅ 1 s | ✅ 只有工程的 `.meshy-dcc` |
| Maya 2027.2（adapter 声明 2025/2026） | ✅ `MAYA_APP_DIR` 隔离，`.mod` 写进它的 modules | ❌→✅ 卡在启动画面"Loading plugins"、一个核心 100%：userSetup 的看门狗在自己的 idle 回调里删自己（Maya 拒绝）；adapter 的 idleEvent 泵占满一核、饿死 lowestPriority 任务（均已修）；之后 34–63 s，空闲 CPU 约 0.02 s/10 s | ✅ `import_model` Meshy FBX → 9998 面；standardSurface 底色 sRGB、法线 Raw 经 bump2d | ❌→✅ `save_scene` 存 `.ma` 却写成 `.mb`（已修）；有未保存时 stop 被拒；discard；重开读回模型与贴图，未保存的立方体消失 | ❌→✅ safe stop 应答但 Maya 不退出（同一个泵，已修）；现 5 s | ✅ 卸载删 `.mod`；Maya 的 USD 插件不认 `MAYA_APP_DIR`，改写了用户 Documents 下 `maya\USDTemplates` 的 json；userSetup 的启动记录写到 `~\.meshy-dcc\receipts` |
| 3ds Max 2027（29.2） | ✅ 启动脚本与 6 个菜单宏写进用户的 CHS 目录 | ✅ 31–58 s | ❌→✅ 每个调用都因 core 加的 `_meta` 参数报 TypeError；`get_scene_status` 读单位报 MAXScript 类型错误；法线、粗糙度等数据贴图按 sRGB 读（2027 默认 OCIO，PNG/JPEG 规则是 sRGB）；法线强度停在 Physical 默认的 0.3（均已修）。Meshy FBX + Physical 材质，法线读回 raw、底色 sRGB、`bump_map_amt` 1.0 | ❌→✅ 工具的修改不置脏（pymxs 默认不记 undo），`open_scene` 不带 force 静默丢掉未保存内容（已修）；中文 `.max`；stop 被拒（`unsaved_state_unknown`）；discard；重开读回，色彩空间保留 | ❌→✅ safe stop 应答但 3ds Max 不退出：停止请求在服务线程上重启了主线程的 QTimer，泵从此停摆（已修）；现 4–7 s | ✅ 卸载删启动脚本和 6 个 `.mcr`，无残留 |
| Substance 3D Painter 12.1.5 | ✅ `dcc_paths` 命中；隔离：`MESHY_DCC_SUBSTANCE3D_PAINTER_PROFILE` 指测试 profile，启动时 `SUBSTANCE_PAINTER_PLUGINS_PATH=<测试 profile>\python` | ✅ 18–20 s，`ping` 在主线程 | ✅ Meshy GLB（中文名、OpenGL 法线、2048）建工程；五图 PBR 层（glTF 的 MR 图拆成粗糙度、金属度，AO 用白图）；Painter 12 工程是 OpenPBR 通道，探针层导出证实 Roughness/Metallic 落到 SpecularRoughness/BaseMetalness | ✅ 中文 `.spp`；有未保存时 stop 被拒；discard 6 s；重开读回层栈，未保存的层消失；glTF 导出与 Meshy 原图一致 | ✅ 6 s | ❌→✅ 卸载留下 Painter `pycache` 里 loader 与整个 payload 的 `.pyc`（已修）；Painter 自己在 Documents 的 `pycache` 缓存标准库，另写 WebEngine 缓存、预览、Sentry |
| Cinema 4D 2026.4 | ⛔ adapter 唯一用到的宿主进程 `c4dpy` 每次启动都崩（0xC0000005，`io_usd` 模块里的 `xzlib0.dll`；GUI 同日也崩过一次）；该 DLL 未签名、构建日期晚于同模块其余 Maxon 签名文件；把 APPDATA、TEMP、USERPROFILE 换成 ASCII 路径无效 | — | — | — | — | — |

adapter 修复（`meshy-dcc-adapters`，未 push）：

- `windows/material-maker` `32da5f3`：`mkstemp()` 的描述符没关，Windows 上无法删除/替换暂存文件（16 个已有测试在 Windows 上因此失败）；版本从 exe 版本资源读。
- `windows/godot` `456c2b3`（preflight 失败说明原因）、`09ae50f`（start-instance 工程路径按 Windows 规则比较）、`6c86fb6`（插件把 Godot 版本交给 sidecar，`list` 不再是 godot@unknown，与平台无关）。
- `windows/unity` `5ab4e43`：从 Unity Hub 记录找编辑器；修正一条假设构建机不是 Windows 的测试。
- `windows/houdini` `26ef06c`（从 houdini.exe 读版本）、`c601fc8`（glTF 用 GLTF SOP；读不了的文件算失败并清掉空容器）。
- `windows/unreal` `73cf335`：`set_actor_transform`、`attach_actor` 先 `modify()`，否则 safe stop 会丢掉未保存的修改（与平台无关）。
- `windows/zbrush` `38f0f51`：ZBrush 在 `-script` 返回后从脚本全局里删掉 `__file__`，Windows 上桥靠 UI 定时器继续服务，之后用到 `__file__` 的请求
  （`get_session_info`）都失败；改为加载时取路径。sidecar 导入不了 `zbrush.commands`，版本总是 unknown；改由桥启动 sidecar 时传入（与平台无关）。
- `windows/maya` `b189563`（userSetup 看门狗改为在 idle 回调之外删除自己）、`e877a4a`（主线程派发改用 Qt 单次定时器并遵守 core 的 tick 间隔，
  不再每次 idle 都跑）、`6136db1`（`save_scene` 按扩展名决定 `.ma`/`.mb`）。前两个与平台无关，Maya 2027 上必现。
- `windows/3dsmax` `a6a1524`（工具接受 core 注入的 `_meta`）、`2232f6e`（pymxs 的 name 值也是 callable，读单位不再调用它）、
  `aa49738`（只在主线程 poke 泵；从别的线程重启 QTimer 会让 Qt 丢掉定时器 id 而不停定时器，泵永久停摆）、
  `1ac9820`（非只读工具包在 pymxs undo 记录里执行，3ds Max 才会置脏；只读工具不包，空记录也会置脏）、
  `d28c248`（数据贴图用 OCIO 的数据色彩空间、gamma 1.0 打开；只在打开时设置有效）、`467278a`（Physical 材质上的法线强度设为 1.0，同 Send To 插件）。
  全部与平台无关（3ds Max 只有 Windows 版）。
- `windows/painter` `e31f266`：Painter 12 把 `sys.pycache_prefix` 设成 `<profile>\pycache`，卸载现在一并删掉 loader 与 payload 的字节码镜像；
  `d4936b6`：adapter 的 CLAUDE.md 记下真机隔离方法。

其他发现（未修）：

- Claude Code 等 agent 的进程环境带 `HOME`，Houdini 规则先认 `HOME`：agent 装的包文件落在 `~\houdiniX.Y`，从开始菜单启动的 Houdini 看不到
  （start-instance 起的 Houdini 继承同一 `HOME`，能用）。本次用户目录里出现的 `~\houdini21.0\houdini.env` 就是在这种环境里跑 Houdini 自带工具产生的。
- adapter 运行时在真实家目录建 `~\.meshy-dcc\<host>\skills`（空目录），不跟 `MESHY_DCC_DATA_DIR`；每次 CLI 运行在 `%APPDATA%\update\meshy-dcc\installations\` 写锁文件；
  `cargo test` 往真实 `%LOCALAPPDATA%\meshy-dcc` 写 admin DB 与更新缓存。
- Roblox 刚启动时 `list` 显示 `roblox@unknown`，首次 session 后才填上版本；
  Unity 重启后第一个调用报"可能被模态框挡住"，实为编辑器仍在忙；OpenSCAD 2021.01 两次导出的 STL 面序不同（大小相同）。
- 只在 Windows 构建机上失败的测试：Houdini（建符号链接要权限、macOS 包启动器无执行位）、Unreal（用 POSIX `Build.sh` 假装构建）、
  3ds Max 的 12 个安装器/打包测试（建符号链接要权限）；发布在 macOS runner 上打包，不受影响。
- Meshy 的 3ds Max Send To 插件用 `openBitMap gamma:1.0` 打开粗糙度、金属度、法线贴图；3ds Max 2024 起默认 OCIO，`gamma` 不再决定解释方式，
  这些贴图仍按 sRGB 读（本仓库之外）。
- `start-instance` 对按机器安装、不按工程的宿主（3ds Max、Maya、Painter）也要求 `--project`，宿主并不从它打开场景；3ds Max、Painter 的 reference 已写明。
- Painter 的测试 `test_default_server_starts_when_legacy_port_is_occupied` 自己要占 127.0.0.1:8765，和本文建议的本地包服务端口相同，
  包服务开着时这一个测试必然失败；关掉包服务后通过。
- 本机 Painter、3ds Max、ZBrush 的 exe 签名校验为 `HashMismatch`，按用户要求忽略；Cinema 4D 的 exe 签名有效，但 USD 模块里有一个未签名的 DLL（见上表）。

待决定：

1. 支持矩阵：上表 ✅ 的 Windows × 宿主组合（OpenSCAD、FreeCAD、Material Maker、Blender、Godot、Unity、Unreal、Houdini、Roblox Studio）可以进；
   其中 Material Maker、Godot、Unity、Unreal、Houdini 依赖上面的 adapter 修复，合并并重新打包、回填 catalog sha 之后才成立。Houdini 是第一次在真机上跑，可去掉 experimental。
   ZBrush 的功能链路在 Windows 上走通了，但本机 ZBrush.exe 签名不完好、安装靠测试替换；在一台签名完好的 ZBrush 上复跑安装一步之前，建议仍保持 experimental。
   Maya 2027、3ds Max 2027、Substance 3D Painter 12.1.5 也走通了六步，依赖上面 `windows/maya`、`windows/3dsmax`、`windows/painter` 的修复；
   三者都是第一次在真机上跑。Maya、3ds Max 的 adapter 声明支持 2025/2026，这次只测了 2027，进矩阵时按实测版本写。
   Cinema 4D 本机 `c4dpy` 起不来，adapter 没测到，不进。
2. Godot 在 Windows 上要不要加 `dcc_paths`（winget、Steam 位置）并在 project_steps 传 `{dcc_path}`；Material Maker 要不要加 winget 位置的 glob。
3. Houdini 在 agent 环境（`HOME` 由终端设置）里的安装位置：照 Houdini 规则（现状），还是 Windows 上只认用户持久环境里的 `HOME`，并让 launch 用同一规则。
4. 发布前给 Windows exe 做代码签名（Smart App Control 开着的机器会直接拦未签名程序；本机关闭，未能实测）。
5. Unreal：中文等非 ASCII 工程路径编不过模块，reference 是否写明。

### 第二轮（2026-10-28，0.2.2 候选）

环境同第一轮（同一台机器、同一批宿主版本；Roblox Studio 启动时自更新到 0.745）。core 为 main（含第一轮全部修复），adapters 为 main。
隔离：registry、data、admin DB、更新缓存、marketplace、settings/统计目录（`MESHY_DCC_SETTINGS_FILE`）、gateway 端口都指到测试目录；
宿主用各自的变量隔离（Blender、Maya、Houdini、Painter），测试工程在带空格和中文的目录。统计开着但指向本机接收端，bug 报告也发到本机接收端。

#### 第一关：core

| 检查 | 结果 |
|---|---|
| `build_release.sh`（Git Bash + MSVC） | ✅ 一次通过，scrub 无本机路径 |
| `cargo test -p meshy-dcc` | ❌→✅ 470 通过、1 失败、9 忽略。失败的是测试本身：`dir_snapshot` 读了测试进程自己独占加锁的 sentinel，Windows 的锁是强制的（os error 33）；改为锁文件只比长度后通过（core #38） |
| `--version`、`doctor` | ✅ doctor 1 s，只读 |
| gateway | ✅ 只绑 127.0.0.1、`--remote-port 0`、无窗口；`gateway restart` 3.4 s；stop 后立即 start 1.9 s（不再等 30 s） |
| `ui` | ✅ 四种语言在 1399 px 和 701 px 下新文案都不溢出；窄窗口侧边栏固定不动。❌→✅ 窄窗口 Agent 页：共享文件夹的长路径卡片撑出横向滚动（core #40） |
| wheel | ✅ 官方 CPython 3.11.9 venv 加载 `_core.pyd` |
| `agent-skill`（只看计划） | ✅ 8 个 agent、7 个目标；分隔符混用仍在（第一轮已记） |
| 带空格、中文、`&`、`%` 的目录 | ✅ |

#### 第二轮重点：更新

| 检查 | 结果 |
|---|---|
| 0.2.1 → 0.2.2（`upgrade_smoke.sh`，本地清单） | ❌→✅ `update apply` 正常暂存；之后第一条命令有自己的 JSON 和退出码 0；但 `gateway restart` 后运行中的 gateway 仍是 0.2.1（磁盘上已是 0.2.2）。修复后一次 restart 即为 0.2.2 |
| 0.2.2 → 0.2.3（一次 `update apply --yes`） | ❌→✅ 原版：exit 3，`gateway_restart: failed`（"still runs 0.2.2"），安装目录留下两个 `.meshy-dcc-backup`。修复版：第一条命令直接以新版重启，gateway 自动重启为 0.2.3，无残留 |
| 窗口"立即更新"（0.2.2 → 0.2.3） | ❌→✅ 原版：停在"正在更新…"约 3 分钟后提示"未能以 0.2.3 恢复运行，请运行 meshy-dcc gateway restart"。修复版：页面自动刷新为 0.2.3"已是最新" |
| 运行中的 exe 替换、残留进程 | ✅ 改名 + 移入新文件可行；无残留进程。暂存文件与提交记录写在真实的 `%APPDATA%\update`（不受 `MESHY_DCC_*` 隔离），测试后约 390 MB |

根因（core #37）：程序应用自己的暂存更新时，把正在运行的 exe 改名为 `.meshy-dcc-backup`、放入新文件，然后删除备份——
在 Windows 上那正是自己的运行映像，删除被拒（os error 5），于是一次已提交的替换被报成失败，进程不再重新拉起新版本（CLI 的第一条命令同理，打印
`failed to apply staged binary update` 后以旧版运行）。macOS 能删除运行中的文件，所以没暴露；release workflow 在 `windows-latest` 上的
upgrade-smoke 会因此失败、挡住 0.2.2 发布。修复：备份仍被占用时留到下次 apply 清理；另外 CLI 在拉起 gateway 前替它应用暂存的 server 更新
（这时没有进程占用该文件），让 0.2.1 的用户一次 `gateway restart` 就够。0.2.1 自己的 CLI 改不了，第一条命令的那条警告仍会出现（无害）。

#### 第二轮重点：core 的新行为

- `gateway restart`、stop 后立即 start：✅（见上）。
- `doctor` 只读：✅ 起 Blender 后强杀，`doctor` 把它列在 `inventory.dead`，registry 74 个文件前后字节一致。
- 超时的主线程调用：未能复现 `status: unknown`。Blender 5.1、Maya 2027 GUI 的工具调用都走 Rust `DccDispatcher::post`，不经过本次修的
  Python 调度器：40 s 的主线程调用不会在 30 s 截断，排在后面的调用照常执行。`call --timeout-secs` 只是 CLI 的请求超时：调用方 5 s 后收到
  `TIMEOUT`（exit 4，`retryable: true`），被排队的写操作约 15 s 后仍在 Blender 里执行了。已把 `call`/`call --batch` 的超时改为
  `retryable: false`、`details.reason: outcome_unknown`（core #39）；超时不传给宿主、排队的调用不会被丢弃，待决定。

#### 第二轮重点：统计与报告（都发到本机接收端）

- 统计：首次提示只出现一次；`telemetry status|preview|off|on|reset` 正常，状态落在隔离的 settings 目录；`DO_NOT_TRACK=1` 时设置页写明原因、开关置灰、
  横幅不出现；横幅"好"之后刷新不再出现；设置开关写入 settings。预览里只有版本、OS、连接器类型、按大版本的实例数、计数和匿名安装 ID，没有路径、名称、实例 ID。
- ❌ 13 个宿主共 222 次调用，`tool_calls[].skill` 全是 `custom`：连接器的内置技能上报 `rankPathSource: explicit_arg`（不是 `bundled`），
  而统计只认 `bundled`；另外"已知技能"只取发送那一刻在线实例的能力索引，每天发送时宿主多半已关。
- 报告：CLI `report` 只预览、`--yes` 发出，窗口 Report a problem 同样流程；两份报告里都没有主机名、用户名、本机路径、端口、PID、实例 ID 或安装 ID。

#### 第二关：连接器（全部用 0.2.2 wheel 重新打包，MCP 服务端为 rmcp 2.1）

| 宿主（版本） | 1 安装 | 2 启动 | 3 调用、导入、读回 | 4 保存、停止、重开 | 5 停止 | 6 卸载、残留 |
|---|---|---|---|---|---|---|
| Blender 5.1 | ✅ 19 s | ✅ 9 s / 6 s | ✅ 中文路径 GLB；BaseColor、Metallic/Roughness、Normal 都连上 | ✅ `save_scene` `postcondition.verified: true`；存到只读目录报错且未写文件；stop 被拒（`unsaved_state_unknown`）；discard 1.7 s；重开读回 | ✅ | ✅ `_core.pyd` 由 Blender 改名后下次启动删除；用户配置零差异 |
| Maya 2027.2 | ❌→✅ 包里缺 Python（见打包问题，已修）；重打后 7 s | ✅ 86 s / 65 s | ✅ FBX 9998 面，底色 sRGB、法线 Raw | ✅ `.ma` 为 Maya ASCII；stop 被拒；discard 7.5 s；重开读回 | ✅ 干净时不带 discard 6.4 s | ✅ 删 `.mod`；Maya USD 插件改写 Documents 下 `USDTemplates`（同第一轮） |
| 3ds Max 2027 | ✅ | ✅ 72 s | ❌→✅ `create_box` 的 depth 读回为 25（见下）；`3dsmax-mesh-ops` 25 个工具 ✅；FBX + Physical 材质：法线 raw、底色 sRGB、bump 1.0 | ✅ 中文 `.max`；dirty 正确；stop 被拒；discard 7.5 s；重开读回 | ✅（按设计总需 discard） | ✅ 启动脚本与宏删除 |
| Houdini 21.0.440 | ✅ 5 s | ✅ 62 s / 34 s | ✅ FBX + MaterialX；❌→✅ 导入材质的标量参数（metallic、roughness、opacity）从未设上（见下） | ✅ 中文 `.hip`；stop 被拒；discard 3.3 s；重开读回 | ✅ | ✅ 用户 Houdini 目录零差异 |
| FreeCAD 1.1.4 | ✅ | ✅ 1 s | ✅ 写操作都 `exit_code: 0`（非零退出保护未误伤）；中文 GLB → Mesh::Feature 9998 面 | ✅ 每次调用即保存；重开读回 | ✅ | ✅ |
| OpenSCAD 2021.01 | ✅ | ✅ 5 s | ✅ 打印底座、导出 STL、渲染 PNG；`up_axis: y` 时 OpenSCAD 2021 的 CGAL 求并断言失败，工具如实报错 | ✅ | ✅ | ✅ |
| Material Maker 1.7 | ✅ 需 `--dcc-path` | ✅ 3 s | ✅ 建图、接 Meshy 底色与法线、导出 Godot 2048² | ✅ 重开 `get_graph` 一致 | ✅ | ✅ |
| Godot 4.7.2 | ✅ | ✅ 11 s / 9 s | ✅ GLB → StandardMaterial3D 贴图 | ✅ 中文 `.tscn`；stop 被拒；discard；重开读回 | ✅ sidecar 随后退出 | ✅ Remove 表 |
| Unity 2022.3.55f1c1 | ✅ | ✅ 38 s / 17 s | ✅ FBX + Standard 材质 verified | ✅ 中文场景；stop 被拒；discard 2.3 s；重开读回（重启后第一个调用仍报"可能被模态框挡住"，第二次成功，同第一轮） | ✅ | ✅ |
| Unreal 5.7.4 | ✅ 165 s（编模块；工程路径无中文） | ✅ 51 s / 46 s | ✅ 中文路径 GLB → Nanite StaticMesh + 材质 | ✅ 移动 actor 置脏；stop 被拒；discard；重开为保存值 | ✅ | ✅ |
| Roblox Studio 0.745 | ✅ | ⚠️ 首次自更新后换了进程，超时（同第一轮）；之后 19 s | ✅ GLB → 9998 三角、四张贴图 | ✅ 按设计 | ✅ | ✅ 插件删除 |
| ZBrush 2026.2.1 | ⛔ 签名门禁按设计拒绝；测试替换探测后安装 | ❌ 每次启动都被"上次会话异常结束"提示框挡住，`-script` 不运行，超时（见下） | ✅ 关掉提示框后：OBJ 导入、中文 `.zpr` 保存 | ✅ stop 被拒（按设计） | ✅ 0.6 s | ✅ |
| Substance 3D Painter 12.1.5 | ✅（测试 profile） | ✅ 25 s | ✅ 中文 GLB 建工程、五图 PBR 层、中文 `.spp` | ✅ dirty 时 stop 被拒；discard 6.9 s；重开读回 | ✅ | ✅ 删 loader；Painter 往真实 Documents `pycache` 写测试路径的字节码（隔离测试特有，已记） |

0.2.2 连接器修复的逐项结论：Maya `MESHY_DCC_MAYA_SIDECAR=1` 时 sidecar 带 `--gateway-remote-host 127.0.0.1 --gateway-remote-port 0`、59765 无监听 ✅；
3ds Max `3dsmax-mesh-ops` 注册 25 个工具 ✅，`create_box` 读回 ❌→✅；Houdini 的 USD/USDZ 如实报 `carrier_unreadable`、丢失 2 个 ✅，
双材质 GLB 报 `imported 2 / lost 0` 却同时警告"材质没有导入" ❌→✅（根因是标量参数没设上）；FreeCAD ✅；Blender ✅。

修复（PR）：

- core #37：Windows 自更新（见上）。
- core #38：doctor 测试读自己持锁的 sentinel。
- core #40：窄窗口 Agent 卡片撑宽页面（CSS `min-width: 0` + Playwright 测试；用 mock 在 600 px 下验证，修复前溢出 68 px）。
- core #39：`call` 超时改为结果未知、不可重试。
- adapters #2：3ds Max 的 Box 没有 `depth`，pymxs 静默忽略，长度一直是默认 25；改用 `length` 并从节点读回尺寸。
- adapters #3：`_set_first_parm` 对标量先试 `parmTuple.set(tuple(value))`，`tuple(0.5)` 抛错后直接跳到下一个参数名，标量从未设上；测试替身对标量不返回 parmTuple，掩盖了问题，已改得和 Houdini 一致。
- adapters #4：Maya、Houdini、Painter 的打包脚本只要缓存目录存在就复用，空目录会打出不带 Python 的包（安装报"找不到路径"、exit 30）。

其他发现（未修）：

- ZBrush：适配器的安全停止就是 `TerminateProcess`，ZBrush 把会话记为异常结束，下次启动弹出模态 Note，`-script` 不运行，`start-instance` 必然超时；
  超时后 `stop-instance --operation-id` 还会拒绝（"operation has not registered an instance yet"），要改用 `--instance-id --dcc-type`。ZBrush SDK 没有退出命令。
- Maya sidecar 模式（`MESHY_DCC_MAYA_SIDECAR=1`，非默认）：找不到 `meshy-dcc-server` 时日志说回退到进程内 server，实际什么都没起，提示还让人
  `pip install meshy-dcc-server`（该包已不发布）；设了 `MESHY_DCC_SERVER_BIN` 后实例以 `maya@unknown`、sidecar 的 PID 注册，没有 operation/project，`start-instance` 认不出、超时。
- Houdini `import_to_scene`：多材质载体只把第一个材质绑到容器（其余建了但没绑，仍算"已导入"）；GLB 只重建标量、丢贴图，却报 `as_authored` 且无警告；
  解码器把缺省的 `metallicFactor` 当 0（glTF 缺省为 1）。USD 里贴图驱动的 UsdPreviewSurface 被说成"没有可读的 UsdPreviewSurface"。
- Unreal：大多数技能的 schema 没有 `additionalProperties: false`（`unreal-actors` 9 个工具都没有），传错参数名（如 `location` 而非 `location_x`）会返回成功、置脏、不动。
- Painter：`install --dcc-type substance3d-painter`（包名）可以装，但同名的 `start-instance`/`dcc-types` 说未安装、提示"重装"；`dcc-types` 列表里的 `substance3d_painter` 正常。
- 统计：适配器的单元测试套件会起真实 gateway，不关统计、不隔离家目录。在开发机上跑超过 10 分钟的测试（例如 3ds Max 适配器单测）时，
  测试进程里的 gateway 会向线上 PostHog 发一条匿名统计，并写入真实的 `~\.meshy-dcc`、`%LOCALAPPDATA%\meshy-dcc\log`；本轮已发生一次。
  跑适配器测试时应设 `MESHY_DCC_TELEMETRY=0`，测试套件最好自己关掉。
- Unity 重启后第一个调用的错误信息仍指向"模态框"并建议 `ui-control`，实为编辑器仍在忙。
- 打包时从 GitHub 下载 python-build-standalone 间歇性 TLS 校验失败（网络），godot、zbrush、roblox、openscad 的打包脚本每次都重新下载、没有缓存。
- 已发布的 `install.ps1` 末尾会对真实 agent 目录执行 `agent-skill --execute` 并改用户 PATH；本文第 6 节第 1 步原样照做会写用户的 agent 目录，
  本轮只跑了它的下载+校验部分。
- 只在 Windows 构建机上失败的适配器单测与第一轮相同（符号链接权限、执行位）。

待决定：

1. 支持矩阵：Blender、FreeCAD、OpenSCAD、Material Maker、Godot、Unity、Unreal、Houdini、Roblox Studio、Maya 2027、3ds Max 2027、
   Substance 3D Painter 12.1.5 在 Windows 上第二轮六步走通；3ds Max 与 Houdini 的结论依赖上面的修复。ZBrush 因启动被 Note 挡住，仍不建议进矩阵。
2. 0.2.2 发布前必须合入 core #37，否则 Windows 的 upgrade-smoke 失败、用户的一步更新也会失败。
3. 统计的技能名：让适配器把自带技能目录标成 `bundled`，还是统计接受适配器的路径来源；"已知技能"是否持久化（`state.json` 已有 `known_skills` 字段）。
4. 主线程调用的超时是否传给宿主，让排队未开始的调用在调用方超时后被丢弃。
5. ZBrush 安全停止的退出方式；Maya sidecar 模式要不要保留。
6. Houdini 多材质绑定与贴图；Unreal schema 是否统一加 `additionalProperties: false`。
