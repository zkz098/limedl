import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';
import UnoCSS from 'unocss/astro';

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
      social: {
        github: 'https://github.com/zkz098/limedl',
      },
      sidebar: [
        {
          label: '快速上手',
          translations: { en: 'Getting Started' },
          items: [
            { label: '项目简介', translations: { en: 'Overview' }, slug: 'getting-started/overview' },
            { label: '安装指南', translations: { en: 'Installation' }, slug: 'getting-started/installation' },
          ],
        },
        {
          label: '核心功能',
          translations: { en: 'Features & Usage' },
          items: [
            { label: '任务管理与并发', translations: { en: 'Tasks & Concurrency' }, slug: 'guides/tasks' },
            { label: 'CDN 探针加速', translations: { en: 'CDN Acceleration' }, slug: 'guides/cdn' },
            { label: 'BitTorrent 调优', translations: { en: 'BitTorrent Tuning' }, slug: 'guides/bittorrent' },
          ],
        },
        {
          label: '生态与集成',
          translations: { en: 'Ecosystem & Integrations' },
          items: [
            { label: '浏览器接管插件', translations: { en: 'Browser Extensions' }, slug: 'ecosystem/browser' },
            { label: 'Aria2 RPC 对接', translations: { en: 'Aria2 RPC' }, slug: 'ecosystem/aria2-rpc' },
          ],
        },
        {
          label: '进阶与开发',
          translations: { en: 'Advanced & Dev' },
          items: [
            { label: '磁盘缓冲池机制', translations: { en: 'Disk Buffer Pool' }, slug: 'advanced/buffer-pool' },
            { label: '源码编译与架构', translations: { en: 'Build from Source' }, slug: 'advanced/development' },
          ],
        },
      ],
      customCss: ['./src/styles/starlight-custom.css'],
    }),
  ],
});
