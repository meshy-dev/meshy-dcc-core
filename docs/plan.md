# Meshy DCC Core 计划

日期：2026-09-28。版本：0.2.0。本仓库是独立的 Meshy 产品。代码最初 fork 自一个 MIT 项目，
0.2.0 起已完全分叉：不再跟进来源项目，不共享任何名字，也不与其运行时兼容。
署名只保留在 `LICENSE` 和 `NOTICE` 里。

## 1. 方向（已确认）

- **技术栈**：Rust。2026-09-22 至 28 的 Node.js 原型（0.3.30）只作为行为参考，归档在本地分支
  `archive/js-prototype-0.3.30`，不继续开发。
- **Agent 入口**：`meshy-dcc` CLI + `skills/meshy-dcc/SKILL.md`。不向 Agent 暴露 MCP
  connector。Meshy API、认证、计费和下载仍由外部已发布的 `meshy-cli` 负责。
- **全部 Meshy 命名**：
  - CLI：`meshy-dcc`（二进制、crate、安装脚本、release 资产、PyPI 包统一这一个名字）；其余 crate 为 `meshy-dcc-*`；
  - Python 包：`meshy_dcc_core`；
  - 环境变量：`MESHY_DCC_*`；
  - 注册表和状态目录：`meshy-dcc`；
  - gateway 端口：19765。
  - 这样与其他 DCC 运行时装在同一台机器上也不会冲突。
- **DCC 范围与 adapter**：
  - 每个宿主一个 Meshy 自有 adapter 仓库，都基于本仓库的 `meshy_dcc_core` 构建，经 Meshy 渠道发布。
  - 内置 catalog（`meshy-dcc-catalog.yml`）列出 23 个宿主：Blender、Unity、Godot、Unreal、ZBrush、
    Maya、3ds Max、Roblox、Omniverse、Houdini、Cinema 4D、Marmoset、Substance 3D Painter / Designer、
    Mari、OpenUSD、Katana、MotionBuilder、SketchUp、FreeCAD、OpenSCAD、Material Maker、Photoshop。
  - adapter 发布前，条目为 `not_available`。
  - 其他宿主以后作为 Meshy marketplace 的可选包提供，有需要时按同样方式制作，不进内置 catalog。
- **Send To 与 core 分开**：webapp 的 Send To 仍由现有 Meshy DCC 插件负责，core 不接管
  `/import` 和 5324–5332 端口。
  - 从那些插件只取导入规则（PBR 通道、色彩空间、宿主的坑）和宿主侧处理逻辑，
    做成 Meshy skill 包；不复用传输代码。
  - 两者端口不冲突：core adapter 用动态端口和 19765，Send To 插件用 5324–5332。
    同一宿主内能否共存，在接入各宿主时验证。
- **Gateway 与 marketplace**：保留，作为 Meshy 本地 hub 和 Meshy marketplace（见第 2 节）。

## 2. 已有能力

CLI（`crates/meshy-dcc`）已具备本产品需要的主体能力：

| 能力 | 位置 |
|---|---|
| 读 FileRegistry 列出实例、直连宿主 `/mcp` 的 search / describe / load-skill / call | `application/local_instance.rs`、`local_control.rs` |
| 按项目启动宿主并等待就绪，同项目不重复启动 | `start-instance`，`application/instance_launch*` |
| 就绪等待、异步 job 轮询、需授权的安全停止 | `wait-ready`、`call --wait`、`stop-instance --discard-unsaved` |
| adapter 安装（含 `host-package`）、诊断 | `install`、`doctor` |
| 宿主内主线程调度、嵌入式 HTTP server | `meshy-dcc-host`、`meshy-dcc-http` 及 Python wheel |

### Gateway：Meshy 本地 hub

宿主 adapter 会选举 gateway：各宿主抢占 19765 端口，先抢到的在自己进程内运行 gateway；
它退出后其余宿主重新选举。常驻 daemon 更稳定：

- 跨实例搜索、异步 job、事件推送、平台管理 UI 都放在 gateway。
- 以后 Meshy 网页端或托管 Agent 需要访问本机时，gateway（配合 tunnel）是固定入口。
- 所有组件同一版本线，选举只在 Meshy 自己的组件之间进行，结果可预期。

Agent 的 CLI 调用两条路径都保留：local profile 直连宿主，延迟最低；经 gateway
则可以使用聚合能力。

### Marketplace：Meshy marketplace

marketplace 的源是一个 `marketplace.json` URL，包以 zip + SHA-256 校验，已实现安装、升级、
卸载和已安装状态。

- 默认源为 `cdn.meshy.ai/meshy-dcc-core/marketplace/marketplace.json`，要求 Meshy 签名，
  未发布时安全失败；包和 showcase 媒体也放 CDN，条目主页链接回 meshy.ai。
- 首批包是 Meshy 官方 skill：各 DCC 的导入 / PBR 重建、Blender 3D 打印检查，
  以及"Meshy 资产 → 可玩场景"这类跨软件工作流。
- 可选 adapter 包也发布在这里。

## 3. 架构

```text
Agent → skills/meshy-dcc (SKILL.md)
      → meshy-dcc（单个 Rust 二进制）
          ├─ install / uninstall / doctor      catalog 声明的安装步骤 + SHA-256
          ├─ start-instance / list / wait-ready / stop-instance
          ├─ search / describe / load-skill / call   按实例路由，写操作须显式指定实例
          ├─ asset check / import             manifest / 字节 / SHA / 格式校验 + 读回
          ├─ marketplace …                    Meshy marketplace（cdn.meshy.ai）上的包
          └─ meshy …                          透传外部 meshy-cli 的 JSON 输出
      → 宿主内 Meshy adapter（内置 meshy_dcc_core）
          └─ Meshy skill 包：导入、PBR 材质重建、分析处理、读回
```

工作流层面的 Meshy 能力放在 skill 包里。adapter 缺少的基础能力在 adapter 里补，例如 Unity 的
模型导入与材质、Godot 的外部模型导入。PBR 通道与色彩空间规则参考 Send To 插件仓库的
`docs/pipeline-contract.md` §6 和 Unity SUP-3544 等修复。

ORM 打包图要按 glTF 约定取通道：G = roughness，B = metallic。现有 Unity、Maya、Unreal
插件在这里有误。

## 4. 从 JS 原型保留的行为约束

以下约束已在真实宿主上验证，移植为 Rust 测试，不移植 JS 实现：

- 发布物按锁定的 SHA-256 校验后再安装；半成品不算已安装。
- 启动前先写待核对的 launch 记录；宿主已启动但记录写入失败时，保留 PID，禁止同项目
  直接重启或修复。
- 写操作必须显式指定实例：用实例 slug 或 `--instance-id`，前缀须唯一且至少 4 位。实例失联时
  不改投同类实例。进程存活不等于就绪。
- Blender 原生 dirty 标记不可靠：stop 默认拒绝，只有用户同意丢弃未保存内容
  （`--discard-unsaved`）才退出，且只停 start-instance 启动的实例。
- Unity 通过 Editor API 退出并核对进程；断连时不回退到信号。
- 资产交接复制到宿主工程内新目录并复核 SHA；结果未知时保留副本和回执，不重放导入。
- Unity FBX 贴图只接受 manifest 明确列出的单组通道；Godot 导入就绪跟踪 reimport 信号。

## 5. DCC 覆盖

| DCC | Meshy adapter 状态 | 主要改造 | 阶段 |
|---|---|---|---|
| Blender | `meshy-dcc-blender`，本地已建，真机验证过 | 基于 0.2.0 core 重建；Meshy 导入/PBR、3D 打印 skill | R1 首个 |
| Unity | `meshy-dcc-unity`，本地已建，真机验证过（6000.4.10f1） | 按工程安装、自带 Python 的 sidecar；Meshy 模型导入/PBR 材质 skill | R1 |
| Godot | `meshy-dcc-godot`，本地已建，真机验证过（4.7.2） | 按工程安装、自带 Python 的 sidecar；Meshy GLB 导入与导入就绪、PBR 读回 | R1 |
| ZBrush | 待建（需 2026.1+） | 补导入格式与保存 | R2 |
| Unreal | 待建 | macOS 本地编译插件；Meshy PBR 材质 | R2 |
| Roblox | 新做 | Studio 插件 + core 桥接（传输方式待验证）；资产走 Open Cloud | R2 |
| Maya、3ds Max、Houdini、Cinema 4D、Marmoset、Substance 3D Painter / Designer、Mari、OpenUSD、Katana、MotionBuilder、SketchUp、FreeCAD、OpenSCAD、Material Maker、Photoshop | 待建 | 改造为 Meshy adapter；本机无宿主的只做构建与单测 | R2+ |
| Omniverse | 新做 | Kit 扩展加载 wheel；参考 Send To 版 Kit 扩展 | 暂缓 |

只有真实 OS × 宿主 × adapter 验证通过的组合才进入支持矩阵。

工具返回错位一条（调用 N 拿到 N-1 的结果）的问题要在每个宿主接入时专门测；CLI 已按
request id 关联并拒绝错位的响应。

## 6. 阶段

| 阶段 | 交付 | 验收 |
|---|---|---|
| R0 骨架 | 产品面改名与发布渠道改向（0.1.0） | 真实 Blender：list → search → describe → call → 读回 |
| 0.2.0 分叉 | 全部 Meshy 命名、单一版本线、Meshy-only catalog | 全部测试通过；Blender adapter 基于新 core 真机复验 |
| R1 对等 | Meshy Blender / Unity / Godot adapter；平台开启/关闭；asset check/import、meshy 透传；Meshy marketplace 首批包 | 达到 0.3.30 的主矩阵验收；三个宿主导入、读回、保存重开 |
| R2 扩宿主 | ZBrush、Unreal、Roblox 及其余内置宿主 | 各宿主导入、读回、保存重开（有宿主的） |
| R3 评估 | 与既有 v2 对照同任务 | 单次调用延迟、turn 数、token、耗时；不夸大为质量结论 |

## 7. 版本

- 单一版本线：crate、Python wheel、`--version`、tag、Meshy 发布与自更新共用一个版本号，
  0.2.0 起。
- gateway 选举与 adapter 最低 core 版本检查只在 Meshy 组件之间比较这个版本。
- adapter 各自独立编号，在 catalog 里声明所需的最低 core 版本。

## 8. 验证

```sh
cargo build -p meshy-dcc
cargo test -p meshy-dcc
```

真实宿主检查只启动测试自己创建的工程和进程，不触碰用户已有项目。本地开发记录在
被忽略的 `docs/local/`，不写入本机路径、实例 UUID 或私人任务日志。
