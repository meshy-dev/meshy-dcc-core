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

（Windows session 在这里追加。）
