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
  ]],
  ['核心功能', 'Features & Usage', [
    ['任务管理与并发', 'Tasks & Concurrency', 'guides/tasks'],
    ['CDN 探针加速', 'CDN Acceleration', 'guides/cdn'],
    ['BitTorrent 调优', 'BitTorrent Tuning', 'guides/bittorrent'],
    ['命令行与协议', 'CLI & Protocol', 'guides/cli'],
    ['常见问题与排错', 'Troubleshooting', 'guides/troubleshooting'],
  ]],
  ['生态与集成', 'Ecosystem & Integrations', [
    ['浏览器接管插件', 'Browser Extensions', 'ecosystem/browser'],
    ['Aria2 RPC 对接', 'Aria2 RPC', 'ecosystem/aria2-rpc'],
  ]],
  ['进阶与内幕', 'Advanced & Internals', [
    ['系统架构与技术内幕', 'System Architecture', 'advanced/architecture'],
    ['磁盘缓冲池机制', 'Disk Buffer Pool', 'advanced/buffer-pool'],
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
  site: 'https://limedl.com',
  integrations: [
    UnoCSS({
      injectReset: false,
    }),
    starlight({
      title: 'limedl',
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
      customCss: ['./src/styles/starlight-custom.css'],
    }),
  ],
});
