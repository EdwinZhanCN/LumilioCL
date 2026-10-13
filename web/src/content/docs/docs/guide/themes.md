---
title: 主题
description: 选择内置主题，或用 JSON 文件制作自己的主题。
---

主题决定启动器的配色、输入控件和对话框的圆角，以及界面使用的字体。
浅色和深色各用一套主题。

## 选择主题

1. 打开 **设置 › 外观**。
2. 在 **浅色主题** 一行点 **编辑**，在对话框里选一套主题。
3. 在 **深色主题** 一行点 **编辑**，在对话框里选一套主题。

选择立即生效并保存。
**明暗** 设为 **跟随系统** 时，启动器随系统的明暗在两套主题之间切换。
要回到内置主题，在对话框里点 **恢复默认**。

启动器自带以下主题：

| 主题 | 明暗 | 来源 |
| --- | --- | --- |
| Aluminium | 浅色 | 启动器默认 |
| Night | 深色 | 启动器默认 |
| Catppuccin Latte | 浅色 | [Catppuccin](https://github.com/catppuccin/catppuccin)，MIT 许可 |
| Catppuccin Mocha | 深色 | [Catppuccin](https://github.com/catppuccin/catppuccin)，MIT 许可 |
| Rosé Pine Dawn | 浅色 | [Rosé Pine](https://github.com/rose-pine/rose-pine-theme)，MIT 许可 |
| Rosé Pine | 深色 | [Rosé Pine](https://github.com/rose-pine/rose-pine-theme)，MIT 许可 |

## 添加本地主题

1. 在 **设置 › 外观 › 本地主题** 一行点打开文件夹的按键。
   macOS 上它叫 **在访达中显示**，Windows 上叫 **在文件资源管理器中显示**，Linux 上叫 **在文件管理器中显示**。
   文件夹不存在时，启动器会先创建它。
2. 把 `.json` 主题文件放进这个 `themes` 文件夹。
3. 点 **重新读取**。
4. 在 **浅色主题** 或 **深色主题** 中选择新主题。

`themes` 文件夹的默认位置：

| 系统 | 位置 |
| --- | --- |
| macOS | `~/Library/Application Support/LumilioCL/themes` |
| Windows | `%APPDATA%\LumilioCL\themes` |
| Linux | `$XDG_DATA_HOME/lumilio/themes`；没有设置 `XDG_DATA_HOME` 时为 `~/.local/share/lumilio/themes` |

设置了 `LUMILIO_HOME` 环境变量时，文件夹在 `$LUMILIO_HOME/themes`。
**本地主题** 一行显示实际使用的路径。

启动器只读取这个文件夹第一层的 `.json` 文件。
主页壁纸的图片也存在这里，不要删除它。
无法使用的文件或主题会列在 **本地主题** 下方，并写明原因。
同一文件里一套主题出错，不影响其他主题。

## 文件格式

主题文件是 JSON，结构参照 [Zed](https://zed.dev) 的主题文件。
一个文件有一个 `themes` 数组，可以放多套主题。

下面的文件定义了一套浅色主题。
它只写了几项颜色，其余颜色由内置的 Aluminium 补齐。

```json
{
  "themes": [
    {
      "name": "Paper",
      "appearance": "light",
      "source": "My own palette",
      "colors": {
        "page": "#f4f1ea",
        "panel": "#faf8f3",
        "ink": "#1f1d1a",
        "orange": "#2f6fdb",
        "orange_text": "#1f56b3"
      },
      "radius": 6,
      "radius_lg": 10,
      "fonts": { "sans": "Inter" }
    }
  ]
}
```

完整的例子见启动器自带的 [rose-pine.json](https://github.com/EdwinZhanCN/LumilioCL/blob/main/crates/lumilio-ui/themes/rose-pine.json) 和 [catppuccin.json](https://github.com/EdwinZhanCN/LumilioCL/blob/main/crates/lumilio-ui/themes/catppuccin.json)。

### 主题字段

| 字段 | 必填 | 说明 |
| --- | --- | --- |
| `name` | 是 | 主题名，显示在主题列表里 |
| `appearance` | 是 | `"light"` 或 `"dark"`。决定主题出现在哪个列表，也决定缺少的颜色从哪套内置主题补齐 |
| `source` | 否 | 配色的来源和许可证。改编他人配色时写明 |
| `colors` | 否 | 颜色，见下表。省略的颜色由同一明暗的内置主题补齐 |
| `radius` | 否 | 输入框、下拉框和弹出菜单的圆角，单位为像素，默认 `4`。启动器自己的按键和面板圆角固定，不受影响 |
| `radius_lg` | 否 | 对话框和通知的圆角，单位为像素，默认 `6` |
| `fonts` | 否 | 字体族名：`sans` 是界面字体，`mono` 是等宽字体，`cjk` 是中文字体 |
| `window_background` | 否 | 窗口背后的材质：`"opaque"`（默认）、`"transparent"` 或 `"blurred"`。颜色带透明度时才看得到 |

### 颜色

颜色写成 `#rrggbb`，或带透明度的 `#rrggbbaa`；`#` 可以省略。
颜色名沿用默认配色的叫法，例如 `orange` 是强调色，值可以是任何颜色。

| 颜色 | 用在哪里 |
| --- | --- |
| `page` | 页面底色 |
| `panel` | 面板、列表块和输入框 |
| `hairline` | 分隔线和边框 |
| `ink` | 正文文字 |
| `muted` | 说明文字和次要数值 |
| `key_white` | 白色按键：次要按键、分段按键 |
| `key_black` | 黑色按键：“更多”、画面上的次要按键 |
| `key_grey` | 灰色按键：未选中的标签页、禁用的按键、普通标签 |
| `on_white` | 白色按键上的文字 |
| `on_black` | 黑色按键上的文字 |
| `orange` | 强调色填充：主按键、当前标签页、亮起的指示灯 |
| `orange_text` | 作为文字的强调色 |
| `on_orange` | 强调色填充上的文字 |
| `danger` | 危险操作的确认按键 |
| `danger_text` | 危险菜单项和失败说明的文字 |
| `led_off` | 熄灭的指示灯 |
| `ok` | 表示成功的指示灯 |
| `display` | 显示窗的底色，例如游戏封面所在的窗口 |
| `display_ink` | 显示窗里亮起的字段和进度条 |
| `display_dim` | 显示窗里未亮的字段 |
| `display_label` | 显示窗上的说明和单位 |

## 规则与限制

- 圆角取 `0` 到 `16` 像素；超过 `16` 时按 `16` 处理。
- 在 **设置 › 外观** 里选的字体优先于主题里的字体。
- 主题指定的字体没有安装时，启动器改用自带字体。
- 本地主题与已有主题同名、明暗也相同时，不会被读入。
- 文件里不认识的字段会被忽略。
  主题缺少颜色时，设置页提示“这套主题缺少 N 项颜色，缺的已用默认补齐”；颜色名写错时也会出现这条提示。
- `"blurred"` 需要系统支持模糊；不支持时窗口保持原样。
- 修改主题文件后，点 **重新读取** 才会生效。
- 删除正在使用的主题并点 **重新读取** 后，启动器回到同一明暗的内置主题。
