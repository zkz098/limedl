import type { APIRoute } from 'astro';
import { changelogRss } from '../lib/changelog-feed';

export const GET: APIRoute = () =>
  new Response(changelogRss('zh-CN'), {
    headers: { 'Content-Type': 'application/rss+xml; charset=utf-8' },
  });
