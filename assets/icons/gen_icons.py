#!/usr/bin/env python3
"""LumilioCL 图标生成器（开发期工具，产物提交进仓库；CI 不依赖它）。

依赖: pip install cairosvg pillow
用法: python3 gen_icons.py [APP_ID]

输入（唯一事实来源，只改这些）:
  src/tile/tile-light.svg        Windows / Linux 大尺寸母版 (>=64px)
  src/tile/tile-light-small.svg  20–48px 母版（无螺丝孔）
  src/tile/tile-{16,20,24,32}.svg 像素对齐母版（这几个尺寸精确命中时使用）
  src/macos/AppIcon-layers/*.svg Icon Composer 图层（手动导入一次，产出 AppIcon.icon）
输出（生成物，不要手改）:
  windows/LumilioCL.ico
  linux/hicolor/<N>x<N>/apps/<APP_ID>.png, linux/hicolor/scalable/apps/<APP_ID>.svg
"""
import io, shutil, sys
from pathlib import Path
import cairosvg
from PIL import Image

APP_ID = sys.argv[1] if len(sys.argv) > 1 else "app.lumilio.LumilioCL"
ROOT = Path(__file__).resolve().parent
SRC = ROOT / "src" / "tile"

# Windows 11 各缩放下标题栏/任务栏/资源管理器会请求的尺寸 + 256 大图标
ICO_SIZES = [16, 20, 24, 30, 32, 36, 40, 48, 60, 64, 72, 80, 96, 128, 256]
# freedesktop hicolor 常用尺寸
LINUX_SIZES = [16, 22, 24, 32, 48, 64, 128, 256, 512]


def master_for(size: int) -> Path:
    hinted = SRC / f"tile-{size}.svg"
    if hinted.exists():
        return hinted
    if size <= 48:
        return SRC / "tile-light-small.svg"
    return SRC / "tile-light.svg"


def render(size: int) -> Image.Image:
    png = cairosvg.svg2png(url=str(master_for(size)), output_width=size, output_height=size)
    return Image.open(io.BytesIO(png)).convert("RGBA")


def build_ico(out: Path) -> None:
    frames = {s: render(s) for s in ICO_SIZES}
    out.parent.mkdir(parents=True, exist_ok=True)
    base = frames[max(ICO_SIZES)]
    base.save(out, format="ICO", sizes=[(s, s) for s in ICO_SIZES],
              append_images=[frames[s] for s in ICO_SIZES if s != max(ICO_SIZES)])


def build_linux(out_root: Path) -> None:
    if out_root.exists():
        shutil.rmtree(out_root)
    for s in LINUX_SIZES:
        d = out_root / f"{s}x{s}" / "apps"
        d.mkdir(parents=True, exist_ok=True)
        render(s).save(d / f"{APP_ID}.png", optimize=True)
    d = out_root / "scalable" / "apps"
    d.mkdir(parents=True, exist_ok=True)
    shutil.copy(SRC / "tile-light.svg", d / f"{APP_ID}.svg")


if __name__ == "__main__":
    build_ico(ROOT / "windows" / "LumilioCL.ico")
    build_linux(ROOT / "linux" / "hicolor")
    print(f"done: APP_ID={APP_ID}")
