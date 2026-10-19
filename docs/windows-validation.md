# Windows 验证：给 Windows 上的 Claude Code session

这份文档让一台 Windows x64 机器上的新 Claude Code session 拿到背景、接着做 Windows 验证。
在 `meshy-dcc-core` 仓库根目录启动 Claude Code，第一句可以是：

> 读 `docs/windows-validation.md`，按它做 Windows 验证。先检查环境并告诉我缺什么，装软件前先问我。

## 1. 背景（一屏）

- **Meshy DCC Core**：一个本地 CLI（`meshy-dcc`）+ 常驻 gateway（`meshy-dcc-server`，端口 19765）+ 编进 CLI 的 agent Skill。
  每个 DCC 软件装一个 Meshy 连接器（adapter），实例注册到本机，agent 通过 CLI 点名实例调用工具。
  定位、架构、命令见 `README.md`、`docs/plan.md`、`skills/meshy-dcc/SKILL.md` 与 `skills/meshy-dcc/references/`。
- **两个仓库**（都在 `meshy-dev`，private）：
  - `meshy-dcc-core`（本仓库）：Rust core、Python wheel `meshy_dcc_core`、`meshy-dcc-catalog.yml`（内置安装目录）、release workflow。
  - `meshy-dcc-adapters`：14 个宿主的 adapter，一个宿主一个目录 `meshy-dcc-<host>/`，`scripts/build_release.py <host>` 打包。
- **现状**：版本 0.2.0，**还没发布**（npm、CDN、GitHub Release 都没有；catalog 里全部 `not_available`）。
  macOS arm64 上真机验证过的宿主：Blender、Unity、Godot、Unreal、OpenSCAD、Material Maker、FreeCAD、Roblox Studio
  （Roblox 有已知限制：导入的模型不能随 place 保存）。没在真软件里跑过、标 experimental 的：Maya、3ds Max、Houdini、
  Cinema 4D、Substance 3D Painter、ZBrush。
- **Windows 上什么都没真跑过**。之前的 Windows 包是 macOS 上用 mingw 交叉编译的；正式发布由 CI 在 `windows-latest` 上用
  MSVC 原生构建（`.github/workflows/release.yml` → `scripts/release/build_release.sh`）。**这次在 Windows 上原生构建，走和 CI 一样的路径。**
- **目标**：找出并修掉 Windows 上的问题，给出"哪些 Windows × 宿主组合真机通过"的清单。只有真机通过的组合才进支持矩阵。

## 2. 规则（必须遵守）

- 本仓库的 `CLAUDE.md` 是总规则。另外：
- **不 push、不开 PR、不发布、不上传**，除非用户明确要求。代码修复在本地分支 `windows/<主题>` 上提交；core 和 adapters 的提交分开。
- 只碰测试自己建的工程和进程；**不打开、不修改用户已有的工程和软件偏好**。给进程发信号前核对 PID 和命令行。
- 隔离：测试时设 `MESHY_DCC_REGISTRY_DIR`、`MESHY_DCC_DATA_DIR`、`MESHY_DCC_GATEWAY_ADMIN_DB` 到测试目录；
  要用非默认端口时，`MESHY_DCC_GATEWAY_PORT`（gateway 监听）和 `MESHY_DCC_BASE_URL=http://127.0.0.1:<端口>`（CLI 连哪个 gateway）**两个都要设**。
  先确认 19765 上没有用户自己的 gateway（`Get-NetTCPConnection -LocalPort 19765 -State Listen`）。
- 宿主会把偏好写进用户目录的（Documents、`%APPDATA%`、`%LOCALAPPDATA%`），测前测后列清单对比，测试产生的东西列出来给用户确认后再删。
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
scripts/release/build_release.sh out      # out/meshy-dcc.exe, out/meshy-dcc-server.exe, out/meshy_dcc_core-0.2.0-cp38-abi3-win_amd64.whl
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

## 6. 产出

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
