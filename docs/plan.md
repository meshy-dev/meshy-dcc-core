# Meshy DCC Core：Rust fork 计划

日期：2026-09-28。版本：0.1.0。本仓库是独立的 Meshy 产品，源码来自 `dcc-mcp/dcc-mcp-core`
（MIT）`v0.20.36`，不带上游 git 历史（见 `UPSTREAM.md`）。

## 1. 方向（已确认）

- **技术栈**：Rust。直接改造上游 workspace，不再用 Node 包装上游二进制。
  2026-09-22 至 28 的 Node.js 原型（0.3.30）只作为行为参考，归档在本地分支
  `archive/js-prototype-0.3.30`，不继续开发。
- **Agent 入口**：`meshy-dcc` CLI + `skills/meshy-dcc/SKILL.md`。不向 Agent 暴露 MCP
  connector。Meshy API、认证、计费和下载仍由外部已发布的 `meshy-cli` 负责。
- **改名范围**：只改产品面（二进制名、Meshy 自有状态目录与配置、Skill、文档）。
  内部 crate、Python 模块 `dcc_mcp_core`、宿主协议保持上游命名，这样上游 core 和
  adapter 的补丁都能继续套用。
- **DCC 范围**：
  - Meshy 自有 adapter 覆盖 Meshy 已有 Send To bridge 的 9 个 DCC。
  - 内置 catalog（`meshy-dcc-catalog.yml`）另外按 `upstream` 名单纳入一批上游 adapter，使用上游版本：
    Houdini、Cinema 4D、Marmoset、Substance 3D Painter / Designer、Mari、OpenUSD、Katana、
    MotionBuilder、SketchUp、FreeCAD、OpenSCAD、Material Maker、Photoshop。
  - Meshy 版未发布前，9 个 DCC 里上游已有 adapter 的先用上游版。
  - 其余上游 adapter 以后放进 Meshy marketplace，由用户自选安装，不进内置 catalog。
- **宿主 adapter 全部由 Meshy 自有**：
  - 有上游 `dcc-mcp-<host>` 的宿主，以其源码为起点独立导入，方式与 core 相同，不带上游历史。
  - adapter 内置本仓库构建的 `dcc_mcp_core` wheel，经 Meshy 渠道发布。
  - 上游没有 adapter 的宿主（Roblox、Omniverse）新做。
- **Send To 与 core 分开**：webapp 的 Send To 仍由现有 Meshy DCC 插件负责，core 不接管
  `/import` 和 5324–5332 端口。
  - 那些插件只做导入，没有 agent 可用的工具面，所以只把它们作为导入知识的来源
    （PBR 通道、色彩空间、宿主的坑），不复用代码。
  - 两者端口不冲突：core adapter 用动态端口和 9765，Send To 插件用 5324–5332。
    同一宿主内能否共存，在接入各宿主时验证。
- **Gateway 与 marketplace**：保留并改造为 Meshy 版本（见第 2 节）。

## 2. 上游可直接复用的部分

上游 CLI（本仓库 `crates/dcc-mcp-cli`）已具备本产品需要的主体能力：

| 能力 | 上游位置 |
|---|---|
| 读 FileRegistry 列出实例、直连宿主 `/mcp` 的 search / describe / load-skill / call | `application/local_instance.rs`、`local_control.rs` |
| 按项目启动宿主并等待就绪，同项目不重复启动 | `start-instance`，`application/instance_launch*` |
| 就绪等待、异步 job 轮询、安全停止 | `wait-ready`、`call --wait`、`stop-instance` |
| adapter 安装计划、诊断 | `install`、`doctor` |
| 宿主内主线程调度、嵌入式 HTTP server | `dcc-mcp-host`、`dcc-mcp-http` 及 Python wheel |

### Gateway：保留为 Meshy 本地 hub

上游宿主 adapter 默认就会选举 gateway：各宿主抢占 9765 端口，先抢到的那个在自己进程内
运行 gateway；它退出后其余宿主重新选举。删掉 daemon 并不能去掉 gateway，只会让它寄居
在某个随时可能关闭的 DCC 里。常驻 daemon 反而更稳定：

- 跨实例搜索、异步 job、事件推送、平台管理 UI 都放在 gateway。
- 以后 Meshy 网页端或托管 Agent 需要访问本机时，gateway（配合 tunnel）是固定入口。
- Meshy adapter 内置的 core 与 daemon 同一兼容版本，选举结果可预期。

Agent 的 CLI 调用两条路径都保留：local profile 直连宿主，延迟最低；经 gateway
则可以使用聚合能力。

### Marketplace：改为 Meshy marketplace

上游 marketplace 的源就是一个 `marketplace.json` URL，包以 zip + SHA-256 校验，已实现
安装、升级、卸载和已安装状态。改造方式：

- 内置源改为托管在 `cdn.meshy.ai` 上的 Meshy catalog；包和 showcase 媒体也放 CDN，
  条目的主页链接回 meshy.ai 的对应页面。
- 首批包是 Meshy 官方 skill：各 DCC 的导入 / PBR 重建，以及"Meshy 资产 → 可玩场景"
  这类跨软件工作流。社区包经审核后发布到同一 catalog。
- 上游 `dcc-mcp/marketplace` 作为可选的第二个源，默认不启用，用户需要
  上游资产源时再添加。
- 上游对 dcc-mcp 发布流程身份的 sigstore 校验改为 Meshy 自己的签名身份；首版可以先只做
  SHA-256 固定。

## 3. 架构

```text
Agent → skills/meshy-dcc (SKILL.md)
      → meshy-dcc（单个 Rust 二进制，由 dcc-mcp-cli 改造）
          ├─ setup / doctor / repair          锁版本 + SHA-256 安装 adapter
          ├─ start-instance / list / wait-ready / stop-instance
          ├─ search / describe / load-skill / call   按实例路由，写操作须显式指定实例
          ├─ asset check / import             manifest / 字节 / SHA / 格式校验 + 读回
          ├─ marketplace …                    Meshy catalog（cdn.meshy.ai）上的 skill 包
          └─ meshy …                          透传外部 meshy-cli 的 JSON 输出
      → 宿主内 Meshy adapter（由上游 adapter 源码导入，内置本仓库 core wheel）
          └─ Meshy skill 包：导入、PBR 材质重建、读回
```

工作流层面的 Meshy 能力放在 skill 包里，这是上游原生的扩展点。adapter 本身缺少的基础
能力在 Meshy adapter 里补，例如 Unity 的模型导入与材质、Godot 的外部模型导入。PBR 通道
与色彩空间规则参考 Send To 插件仓库的 `docs/pipeline-contract.md` §6 和 Unity SUP-3544
等修复，只移植规则，不移植代码。

ORM 打包图要按 glTF 约定取通道：G = roughness，B = metallic。现有 Unity、Maya、Unreal
插件在这里有误。

## 4. 从 JS 原型保留的行为约束

以下约束已在真实宿主上验证，移植为 Rust 测试，不移植 JS 实现：

- 发布物按 `runtime-lock` 的 SHA-256 校验后再解包；安装写 receipt，半成品不算已安装。
- 启动前先写待核对的 launch 记录；宿主已启动但记录写入失败时，保留 PID，禁止同项目
  直接重启或修复。
- 写操作必须显式指定实例：用实例 slug 或 `--instance-id`，前缀须唯一且至少 4 位。实例失联时
  不改投同类实例。进程存活不等于就绪。
- Blender 原生 dirty 标记不可靠：默认 stop 在状态未知时拒绝退出，不发信号。
- Unity 通过 Editor API 退出并核对进程；断连时不回退到信号。
- 资产交接复制到宿主工程内新目录并复核 SHA；结果未知时保留副本和回执，不重放导入。
- Unity FBX 贴图只接受 manifest 明确列出的单组通道；Godot 导入就绪跟踪 reimport 信号。

## 5. DCC 覆盖

| DCC | Meshy adapter 起点（2026-09 上游版本） | 主要改造 | 阶段 |
|---|---|---|---|
| Blender | `dcc-mcp-blender` 0.2.11 | 换内置 core wheel；Meshy 导入/PBR skill | R1 首个 |
| Unity | `dcc-mcp-unity` 0.13.0（C#） | 补模型导入设置、材质、资产实例化（上游均没有） | R1 |
| Godot | `dcc-mcp-godot` 0.8.1 | 补外部 glb/fbx 导入与导入就绪；超时后不再提交动作 | R1 |
| ZBrush | `dcc-mcp-zbrush` 0.2.26（需 2026.1+） | 补导入格式与保存 | R2 |
| Unreal | `dcc-mcp-unreal` 0.3.8 | macOS 本地编译插件；Meshy PBR 材质 | R2 |
| Roblox | 无 | 新做 Studio 插件 + core 桥接（传输方式待验证）；资产走 Open Cloud | R2 |
| Maya | `dcc-mcp-maya` 0.9.31 | – | 暂缓（无验证宿主） |
| 3ds Max | `dcc-mcp-3dsmax` 0.2.14 | 仅 Windows | 暂缓 |
| Omniverse | 无（`dcc-mcp-openusd` 为 headless USD） | 新做 Kit 扩展加载 wheel；参考 Send To 版 Kit 扩展 | 暂缓 |

只有真实 OS × 宿主 × adapter 验证通过的组合才进入支持矩阵。

在 Meshy 版发布之前，Blender、Unity、Godot、Unreal、ZBrush、Maya、3ds Max 用内置 catalog 里的上游条目安装，
版本是上游 v0.20.36 catalog 锁定的版本；Roblox、Omniverse 上游没有 adapter，仍是占位。

上游 core #2417 仍未修复：经 CLI 的工具返回会错位一条。每个宿主接入时都要专门测这一项，
需要时在 core 里加 request id 关联。

## 6. 阶段

| 阶段 | 交付 | 验收 |
|---|---|---|
| R0 fork 骨架 | 经确认的改名与上游地址改向；锁定 Blender adapter | 真实 Blender：list → search → describe → call → 读回 |
| R1 对等 | Meshy Blender / Unity / Godot adapter；Meshy catalog 与平台开启/关闭；asset check/import、meshy 透传、stop 保护；Meshy marketplace 首批包 | 达到 0.3.30 的主矩阵验收；三个宿主导入、读回、保存重开 |
| R2 扩宿主 | ZBrush、Unreal、Roblox | 各宿主导入、读回、保存重开 |
| R3 评估 | 与既有 v2 对照同任务 | 单次调用延迟、turn 数、token、耗时；不夸大为质量结论 |

### R0 改造原则

- **默认不删除上游内容。** 没有被我们二进制依赖的 crate 不参与编译，保留没有成本；删除反而会
  在合并上游时产生冲突。只发布需要的二进制，而不是删减源码。
- 可能的改动只有两类：产品面改名，以及把写死的上游地址改指向 Meshy。每一项先说明用途和
  影响，经用户逐条确认后再改。
- 已核查并保留：`dcc-mcp-tunnel-*`（云端到本机的中继，可支撑 Meshy 网页端或托管 Agent
  远程驱动本地 DCC，也是浏览器访问 localhost 受限时的备选通道）、`dcc-mcp-semantic`
  （可选的向量检索 skill，marketplace 包变多后有用）、gateway daemon、marketplace。

## 7. 与上游的关系与版本

- 仓库与上游无 git 关联。上游修复以补丁方式引入（在本机单独的上游 clone 上生成两个 tag 之间的 diff，
  `git apply -3` 后审查测试），整版同步时更新 `UPSTREAM.md` 的基线。
- **产品版本**独立编号，从 0.1.0 开始，用于 `--version`、tag、Meshy 发布与自更新。
- **内部兼容版本**：crate 版本保持导入基线（0.20.36）。gateway 选举与 adapter 最低 core 版本检查会比较
  它，宿主里内嵌的同一运行时也参与比较，所以不跟随产品版本。
- 保留上游 MIT LICENSE 与版权声明；重新分发上游 adapter 或其依赖前逐一核对许可证。

## 8. 验证

```sh
cargo build -p dcc-mcp-cli
cargo test -p dcc-mcp-cli
```

真实宿主检查只启动测试自己创建的工程和进程，不触碰用户已有项目。本地开发记录在
被忽略的 `docs/local/`，不写入本机路径、实例 UUID 或私人任务日志。
