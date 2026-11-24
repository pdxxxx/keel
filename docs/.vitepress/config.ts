import { defineConfig } from "vitepress";

export default defineConfig({
  base: "/keel/",
  appearance: "dark",
  cleanUrls: true,
  lastUpdated: true,

  head: [
    ["link", { rel: "icon", href: "/keel/favicon.ico" }],
    ["link", { rel: "icon", type: "image/png", href: "/keel/keel.png" }],
    ["meta", { name: "theme-color", content: "#090d16" }],
  ],

  locales: {
    root: {
      label: "English",
      lang: "en-US",
      title: "keel",
      description:
        "Repo rules, task specs, session memory, and verification gates for coding agents.",
      themeConfig: {
        nav: [
          { text: "Guide", link: "/guide/getting-started", activeMatch: "/guide/" },
          { text: "CLI", link: "/guide/commands" },
          { text: "MCP", link: "/guide/mcp" },
          { text: "Changelog", link: "/changelog" },
        ],
        sidebar: {
          "/guide/": [
            {
              text: "Introduction",
              items: [
                { text: "Getting Started", link: "/guide/getting-started" },
                { text: "Commands Reference", link: "/guide/commands" },
                { text: "MCP Server", link: "/guide/mcp" },
              ],
            },
            {
              text: "Production & Migration",
              items: [
                {
                  text: "Existing Repos & Migration",
                  link: "/guide/brownfield-and-migration",
                },
                { text: "Architecture & Formats", link: "/guide/architecture" },
              ],
            },
          ],
        },
        outline: { label: "On this page" },
        footer: {
          message: "Released under the MIT License.",
          copyright: "Copyright © 2026 pdxxxx",
        },
      },
    },
    zh: {
      label: "简体中文",
      lang: "zh-CN",
      link: "/zh/",
      title: "keel",
      description:
        "面向 AI 编程的仓库规范、任务记录、会话检索与验证门禁工具。",
      themeConfig: {
        nav: [
          { text: "指南", link: "/zh/guide/getting-started", activeMatch: "/zh/guide/" },
          { text: "命令", link: "/zh/guide/commands" },
          { text: "MCP", link: "/zh/guide/mcp" },
          { text: "更新日志", link: "/zh/changelog" },
        ],
        sidebar: {
          "/zh/guide/": [
            {
              text: "快速上手",
              items: [
                { text: "安装与起步", link: "/zh/guide/getting-started" },
                { text: "命令参考", link: "/zh/guide/commands" },
                { text: "MCP 服务", link: "/zh/guide/mcp" },
              ],
            },
            {
              text: "存量项目与架构",
              items: [
                {
                  text: "老仓库接入与迁移",
                  link: "/zh/guide/brownfield-and-migration",
                },
                { text: "架构与磁盘格式", link: "/zh/guide/architecture" },
              ],
            },
          ],
        },
        outline: { label: "本页目录" },
        docFooter: { prev: "上一页", next: "下一页" },
        lastUpdated: { text: "更新于" },
        returnToTopLabel: "返回顶部",
        sidebarMenuLabel: "菜单",
        darkModeSwitchLabel: "主题",
        footer: {
          message: "基于 MIT 协议开源。",
          copyright: "Copyright © 2026 pdxxxx",
        },
      },
    },
  },

  themeConfig: {
    logo: "/keel.png",
    siteTitle: "keel",
    search: {
      provider: "local",
    },
    socialLinks: [
      { icon: "github", link: "https://github.com/pdxxxx/keel" },
    ],
  },
});
