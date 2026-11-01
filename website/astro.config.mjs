import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';
import UnoCSS from 'unocss/astro';

// Sidebar as data: [zh label, en label, [[zh label, en label, slug], …]].
// Starlight wants `{ label, translations: { en }, slug }` for every page and the
// same shape wrapped in a group, so writing it out longhand repeated four keys and
// one wrapper per entry. The builders below expand this table into that shape.
const sidebarGroups = [
  ['快速上手', 'Getting Started', [
    ['项目简介', 'Overview', 'getting-started/overview'],
    ['安装指南', 'Installation', 'getting-started/installation'],
    ['配置手册', 'Configuration', 'getting-started/configuration'],
    ['平台支持', 'Platform Support', 'getting-started/platform-support'],
  ]],
  ['使用场景', 'Use Cases', [
    ['场景总览', 'Overview', 'use-cases'],
    ['NAS / 软路由离线下载', 'NAS / Soft-Router Downloads', 'use-cases/nas-offline-download'],
    ['跨境与镜像加速', 'Cross-Border Mirrors', 'use-cases/cross-border-mirrors'],
    ['浏览器一键接管', 'Browser Handoff', 'use-cases/browser-handoff'],
  ]],
  ['核心功能', 'Features & Usage', [
    ['任务管理与并发', 'Tasks & Concurrency', 'guides/tasks'],
    ['CDN 探针加速', 'CDN Acceleration', 'guides/cdn'],
    ['URL 重写与自定义 CDN', 'URL Rewrite & Custom CDN', 'guides/url-rewrite-and-cdn-providers'],
    ['BitTorrent 调优', 'BitTorrent Tuning', 'guides/bittorrent'],
    ['命令行与协议', 'CLI & Protocol', 'guides/cli'],
    ['常见问题与排错', 'Troubleshooting', 'guides/troubleshooting'],
  ]],
  ['生态与集成', 'Ecosystem & Integrations', [
    ['浏览器接管插件', 'Browser Extensions', 'ecosystem/browser'],
    ['Aria2 RPC 对接', 'Aria2 RPC', 'ecosystem/aria2-rpc'],
    ['无头服务端 (NAS / 软路由)', 'Headless Daemon (NAS / Server)', 'ecosystem/server-daemon'],
  ]],
  ['安全与帮助', 'Security & Help', [
    ['安全与验签', 'Security & Verification', 'security'],
    ['常见问题', 'FAQ', 'faq'],
  ]],
  ['进阶与内幕', 'Advanced & Internals', [
    ['系统架构与技术内幕', 'System Architecture', 'advanced/architecture'],
    ['磁盘缓冲池机制', 'Disk Buffer Pool', 'advanced/buffer-pool'],
    ['磁盘介质与缓冲调优', 'Disk Media & Buffer Tuning', 'advanced/disk-media-and-buffer-tuning'],
    ['Metalink 与镜像选择', 'Metalink & Mirror Selection', 'advanced/metalink-and-mirrors'],
    ['自更新通道与回滚', 'Self-Update Channels & Rollback', 'advanced/self-update-channels'],
    ['专家参数与 HTTP/3', 'Expert Settings & HTTP/3', 'advanced/expert-settings-and-http3'],
    ['数据备份与 Aria2 迁移', 'Data Backup & Aria2 Import', 'guides/data-backup-and-migration'],
    ['性能基准', 'Benchmarks', 'advanced/benchmarks'],
    ['源码编译与开发', 'Build & Contributing', 'advanced/development'],
  ]],
];

const sidebar = sidebarGroups.map(([label, en, pages]) => ({
  label,
  translations: { en },
  items: pages.map(([pageLabel, pageEn, slug]) => ({
    label: pageLabel,
    translations: { en: pageEn },
    slug,
  })),
}));

export default defineConfig({
  site: 'https://limedl.zkz098.cn',
  integrations: [
    UnoCSS({
      injectReset: false,
    }),
    starlight({
      title: 'limedl',
      favicon: '/favicon.png',
      logo: {
        src: './src/assets/logo.png',
      },
      defaultLocale: 'root',
      locales: {
        root: {
          label: '简体中文',
          lang: 'zh-CN',
        },
        en: {
          label: 'English',
          lang: 'en',
        },
      },
      social: [
        { icon: 'github', label: 'GitHub', href: 'https://github.com/zkz098/limedl' },
      ],
      sidebar,
      components: {
        Head: './src/components/starlight/Head.astro',
      },
      customCss: ['./src/styles/starlight-custom.css'],
    }),
  ],
});
