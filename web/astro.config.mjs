import { defineConfig } from "astro/config";
import react from "@astrojs/react";
import starlight from "@astrojs/starlight";
import mdx from "@astrojs/mdx";

export default defineConfig({
  site: "https://launcher.lumilio.org",
  integrations: [react(), starlight({
    title: "LumilioCL / Docs",
    description: "Install LumilioCL. Learn how to change and test the project.",
    logo: { light: "./public/icons/tile-light.svg", dark: "./public/icons/tile-dark.svg", replacesTitle: false },
    favicon: "/icons/tile-32.svg",
    locales: { root: { label: "简体中文", lang: "zh-CN" }, en: { label: "English", lang: "en" } },
    disable404Route: true,
    customCss: ["./src/styles/docs.css"],
    social: [{ icon: "github", label: "Source code", href: "https://github.com/EdwinZhanCN/LumilioCL" }],
    editLink: { baseUrl: "https://github.com/EdwinZhanCN/LumilioCL/edit/main/web/" },
    sidebar: [
      { label: "Start", translations: { "zh-CN": "开始" }, items: [
        { label: "Documentation", translations: { "zh-CN": "文档首页" }, slug: "docs" },
        { label: "Install LumilioCL", translations: { "zh-CN": "安装 LumilioCL" }, slug: "docs/install" },
      ] },
      { label: "Project", translations: { "zh-CN": "项目进展" }, items: [
        { label: "Roadmap", translations: { "zh-CN": "路线图" }, slug: "docs/roadmap" },
        { label: "Changelog", translations: { "zh-CN": "更新日志" }, slug: "docs/changelog" },
      ] },
      { label: "Contribute", translations: { "zh-CN": "参与贡献" }, items: [
        { label: "Contribution rules", translations: { "zh-CN": "贡献规则" }, slug: "docs/contribute" },
        { label: "Set up development", translations: { "zh-CN": "开发环境" }, slug: "docs/contribute/setup" },
        { label: "Choose a work method", translations: { "zh-CN": "选择工作方式" }, slug: "docs/contribute/work-methods" },
        { label: "Change and review", translations: { "zh-CN": "修改与评审" }, slug: "docs/contribute/change-and-review" },
        { label: "Write documentation", translations: { "zh-CN": "编写文档" }, slug: "docs/contribute/writing" },
      ] },
      { label: "Reference", translations: { "zh-CN": "参考" }, items: [
        { label: "Features", translations: { "zh-CN": "功能文档" }, slug: "docs/features" },
        { label: "WASM plugins", translations: { "zh-CN": "WASM 插件" }, slug: "docs/wasm" },
      ] },
      { label: "Launcher website", translations: { "zh-CN": "启动器官网" }, link: "/" },
    ],
  }), mdx()],
  devToolbar: { enabled: false },
  // Starlight owns document language routing. The marketing page selects
  // its Chinese catalog separately and retains its own HTML language.
});
