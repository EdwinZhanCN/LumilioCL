// The source catalog. Facts here follow README.md and docs/ia/paths/; a
// feature the launcher does not have is not written here.
import { plannedTeasers } from "../data/roadmap";
export const zhCN = {
  lang: "zh-CN",
  meta: {
    title: "LumilioCL",
    description:
      "LumilioCL, 基于Rust的原生 Minecraft 启动器。高性能，低占用；自动管理 Java，发现模组，完全开源。",
  },

  nav: {
    label: "页面导航",
    home: "首页",
    proof: "实测",
    audiences: "为谁",
    stack: "技术",
    roadmap: "路线图",
    download: "下载",
    source: "GitHub 上的源代码",
  },

  hero: {
    // One entry per line: Chinese has no spaces to break at.
    title: ["原生、高性能、", "可扩展的", "Minecraft 启动器"],
    lede: "基于全新的GPU桌面UI渲染框架，你的最后一个下一代 Minecraft 启动器。",
    cta: "免费下载",
    source: "查看源代码",
    docs: "查看文档",
    plateLabel: "LumilioCL 的面板，每个格子都能按",
    idle: "碰碰我？",
    download: "下载",
    close: "关闭",
    prev: "上一个",
    next: "下一个",
    planned: "正在路上",
    shipped: "马上体验",
  },

  // The five black tiles: what the launcher does today.
  features: {
    library: {
      name: "游戏库",
      lede: "新建原版、Fabric 或 Quilt 游戏，或者把 MultiMC、Prism 和 .minecraft 里的游戏搬过来，原文件不动。",
      facts: ["导入 .mrpack 与 MultiMC / Prism 整合包", "收藏、合集和搜索", "完整备份，随时恢复"],
      shot: "游戏库页面的截图",
    },
    discover: {
      name: "发现",
      lede: "在启动器里浏览 Modrinth 上的模组、资源包、光影和整合包，按游戏版本和加载器筛选，装进当前游戏。",
      facts: ["模组、资源包、光影、整合包", "按版本与加载器筛选", "安装进度都在动态里"],
      shot: "发现页面的截图",
    },
    map: {
      name: "世界地图",
      lede: "从存档的 Region 文件画出地表，或者只凭种子算出结构的位置。路径点能和 Xaero 小地图互通。",
      facts: ["存档底图与种子地图", "结构、区块与 Region 图层", "复制坐标与路径点分享串"],
      shot: "世界地图的种子预测与结构图层截图",
    },
    schematic: {
      name: "投影预览",
      lede: "打开 Litematica 投影，看材料清单，或者用游戏自己的贴图在 3D 里转着看、飞进去看。",
      facts: ["材料清单导出为 CSV", "环绕查看与第一人称飞行", "用游戏自带的贴图绘制"],
      shot: "Litematica 投影的 3D 环绕预览截图",
    },
    diagnostics: {
      name: "崩溃诊断",
      lede: "游戏崩溃时，诊断页读取日志和崩溃报告，用一句话说明可能的原因，技术细节放在后面。",
      facts: ["实时输出、历史日志与崩溃报告", "可能的原因与修复建议", "复制脱敏后的完整分析"],
      shot: "崩溃分析弹窗的截图",
    },
  },

  // The three grey tiles: the roadmap (.agents/plans/).
  planned: plannedTeasers,

  launch: {
    stages: ["检查", "依赖库", "资源", "启动"],
    legend: "STAGE",
  },

  // Numbers come from src/data/benchmark.ts, measured, not written here.
  proof: {
    title: "同一台主机，内存占用降低约 {percent}%",
    lede: "厌烦了活动监视器里住满 “Chrome” ？用LumilioCL玩游戏，内存留给 Minecraft。",
    us: "LumilioCL",
    memory: "内存占用",
    memoryNote: "启动 30 秒后",
    processes: "进程数",
    processesNote: "包括网页内容、GPU 和网络等辅助进程",
    size: "安装后大小",
    sizeNote: "应用本身，不含游戏文件",
    method:
      "{date}，在 {machine}（{os}）上测量。LumilioCL {us} 与 {them} {themVersion} 各启动 {runs} 次，每次启动前先退出；用 footprint 读取启动 30 秒后的物理内存（与活动监视器的「内存」一致），{them} 计入它的 WebKit 辅助进程，取中位数。",
  },

  // Each point selects its own 16:10 screenshot through the existing keys.
  audiences: {
    title: "不管你玩了多久",
    lede: "LumilioCL 都已准备好，体验前所未有的功能丰富度和可扩展性",
    shot: "{name}的截图",
    groups: [
      {
        id: "newcomer",
        name: "新手",
        title: "第一次玩，也不折腾",
        lede: "装好就能玩。任何环境它来准备。",
        points: [
          { text: "自动检测 Java，也能下载推荐版本", shot: "/shots/java-runtimes.png" },
          { text: "Microsoft、离线账户与第三方登录", shot: "/shots/accounts.png" },
          { text: "配置下载源、镜像优先与同时下载数", shot: "/shots/download-settings.png" },
          { text: "从其他启动器导入游戏", shot: "/shots/import-games.png" },
          { text: "游戏崩溃时，分析可能的原因并给出建议", shot: "/shots/crash-analysis.png" },
        ],
        shot: "/shots/java-runtimes.png",
        shotName: "Java 管理页",
      },
      {
        id: "player",
        name: "进阶玩家",
        title: "模组、建筑与外观，都在一处",
        lede: "装模组、看地图、对投影，也能管理角色外观。",
        points: [
          { text: "浏览和安装 Modrinth 的模组、资源包、光影和整合包", shot: "/shots/discovery.png" },
          { text: "世界地图：种子预测、存档地图与结构图层", shot: "/shots/world-map.png" },
          { text: "Litematica 投影：完整 3D 预览", shot: "/shots/schematic-preview.png" },
          { text: "Litematica 投影：材料清单与 CSV 导出", shot: "/shots/schematic-materials.png" },
          { text: "预览和管理皮肤，查看披风与鞘翅外观", shot: "/shots/skins.png" },
        ],
        shot: "/shots/discovery.png",
        shotName: "发现页",
      },
      {
        id: "geek",
        name: "极客",
        title: "原生代码，开放到底",
        lede: "没有浏览器内核。每一帧由 GPU 绘制，每一行代码都在 GitHub 上。",
        points: [
          { text: "Rust 2024 与 GPUI，代码以 AGPL-3.0-only 开源", shot: "/shots/source.png" },
          { text: "自定义下载地址规则，官方源保留为回退", shot: "/shots/mirror-rules.png" },
          { text: "选择明暗主题，调整字体与界面缩放", shot: "/shots/appearance.png" },
          { text: "配置内置插件与 Discord 游戏状态", shot: "/shots/plugins.png" },
          { text: "创建快照、完整备份与导出整合包", shot: "/shots/backups.png" },
        ],
        shot: "/shots/source.png",
        shotName: "项目源码",
      },
    ],
  },

  stack: {
    title: "从界面到磁盘，每一层都是原生代码",
    lede: "LumilioCL 不嵌浏览器。界面、渲染、下载和存储，都是编译好的 Rust。",
    link: "在新标签页打开 {site}",
    layers: [
      { legend: "UI", name: "界面", tech: "GPUI · GPUI-Kit", note: "基于Rust的下一代原生桌面UI渲染技术" },
      { legend: "GPU", name: "图形", tech: "Metal / DirectX / Vulkan", note: "世界地图和投影 3D 预览由 WGPU 驱动" },
      { legend: "CORE", name: "核心", tech: "Rust 2024 · tokio", note: "下载、实例、账户和 Java 都是异步任务，界面从不等待" },
      { legend: "DATA", name: "存储", tech: "SQLite", note: "游戏库、启动历史和缓存，只存在一个本地轻量数据库里" },
    ],
  },

  roadmap: {
    details: "查看完整路线图",
    title: "接下来",
    lede: "版本号只往前走。每一步做什么，计划都在仓库里公开。",
    current: "已发布",
    planned: "计划中",
  },

  download: {
    title: "下载 LumilioCL",
    pending: "0.1.0 即将发布。发布后，这里会给出对应系统的安装包。",
    releases: "在 GitHub 上查看发布",
    for: "下载 {os} 版",
    get: "下载",
    github: "从 GitHub 下载",
    checksums: "校验文件 SHA256SUMS.txt",
    unsigned:
      "LumilioCL 没有购买开发者证书，第一次打开时系统会提醒「无法确认开发者」。这不代表软件有问题，处理一次就好。",
    unsignedLink: "怎么放行",
    platforms: {
      macos: { name: "macOS", needs: "Apple 芯片（M1 及更新），macOS 11 或更新" },
      windows: { name: "Windows", needs: "Windows 10 / 11，64 位" },
      linux: { name: "Linux", needs: "64 位，需要能用的 Vulkan 显卡驱动" },
    },
    files: {
      dmg: "磁盘映像",
      setup: "安装版（推荐）",
      portable: "免安装版",
      deb: "Debian、Ubuntu",
      tar: "其他发行版",
    },
  },

  notFound: {
    title: "这一页不存在",
    home: "回到首页",
  },

  footer: {
    changelog: "更新日志",
    docs: "文档与贡献规范",
    source: "源代码",
    releases: "发布",
    license: "许可证 AGPL-3.0-only",
    attributions: "致谢",
    fonts: "字体 Space Grotesk、JetBrains Mono 与 DSEG7，均以 SIL OFL 1.1 授权。",
    disclaimer: "LumilioCL 不是 Minecraft 官方产品，未经 Mojang 或 Microsoft 批准，也与它们没有关联。",
  },
};

export type Catalog = typeof zhCN;
