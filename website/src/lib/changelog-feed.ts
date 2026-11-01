import changelog from '../data/changelog.json';

const SITE = 'https://limedl.com';

const labels = {
  'zh-CN': {
    title: 'limedl 更新日志',
    description: 'limedl 稳定版本的新功能、问题修复、性能优化与重构。',
    features: '新功能',
    fixes: '问题修复',
    performance: '性能优化',
    refactoring: '重构',
    page: '/changelog/',
    feed: '/changelog.xml',
  },
  en: {
    title: 'limedl changelog',
    description: 'Features, fixes, performance work, and refactoring in stable limedl releases.',
    features: 'Features',
    fixes: 'Bug Fixes',
    performance: 'Performance',
    refactoring: 'Refactoring',
    page: '/en/changelog/',
    feed: '/en/changelog.xml',
  },
} as const;

function escapeXml(value: string) {
  return value
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll("'", '&apos;');
}

export function changelogRss(lang: 'zh-CN' | 'en') {
  const text = labels[lang];
  const releases = changelog.releases.slice(0, 20);

  const items = releases
    .map((release) => {
      const link = `${SITE}${text.page}#${release.tag}`;
      const body = release.groups
        .map((group) => {
          const heading = text[group.id as keyof typeof text] ?? group.id;
          const commits = group.commits
            .map((commit) => {
              const scope = commit.scope ? `${commit.scope}: ` : '';
              return `- ${scope}${commit.subject} (${commit.shortHash})`;
            })
            .join('\n');
          return `## ${heading}\n${commits}`;
        })
        .join('\n\n');

      return [
        '    <item>',
        `      <title>${escapeXml(`${release.tag} (${release.date})`)}</title>`,
        `      <link>${escapeXml(link)}</link>`,
        `      <guid isPermaLink="false">limedl-${release.tag}</guid>`,
        `      <pubDate>${new Date(`${release.date}T00:00:00Z`).toUTCString()}</pubDate>`,
        `      <description>${escapeXml(body)}</description>`,
        '    </item>',
      ].join('\n');
    })
    .join('\n');

  return `<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0" xmlns:atom="http://www.w3.org/2005/Atom">
  <channel>
    <title>${escapeXml(text.title)}</title>
    <link>${SITE}${text.page}</link>
    <description>${escapeXml(text.description)}</description>
    <language>${lang}</language>
    <atom:link href="${SITE}${text.feed}" rel="self" type="application/rss+xml" />
${items}
  </channel>
</rss>
`;
}
