export const SITE_URL = 'https://limedl.zkz098.cn';
export const OG_IMAGE_URL = `${SITE_URL}/images/og-preview.png`;
export const GITHUB_URL = 'https://github.com/zkz098/limedl';

export function organizationJsonLd() {
  return {
    '@context': 'https://schema.org',
    '@type': 'Organization',
    name: 'limedl',
    url: SITE_URL,
    logo: `${SITE_URL}/logo.png`,
    sameAs: [GITHUB_URL],
  };
}

export function webSiteJsonLd() {
  return {
    '@context': 'https://schema.org',
    '@type': 'WebSite',
    name: 'limedl',
    url: SITE_URL,
    inLanguage: ['zh-CN', 'en'],
  };
}

export function softwareApplicationJsonLd(version: string) {
  return {
    '@context': 'https://schema.org',
    '@type': 'SoftwareApplication',
    name: 'limedl',
    applicationCategory: 'UtilitiesApplication',
    operatingSystem: 'Windows 10, macOS 11, Linux',
    softwareVersion: version,
    downloadUrl: `${GITHUB_URL}/releases/latest`,
    license: 'https://www.gnu.org/licenses/gpl-3.0.html',
    offers: {
      '@type': 'Offer',
      price: '0',
      priceCurrency: 'USD',
    },
    url: SITE_URL,
    sameAs: [GITHUB_URL],
  };
}

export function collectionPageJsonLd(name: string, description: string, url: string) {
  return {
    '@context': 'https://schema.org',
    '@type': 'CollectionPage',
    name,
    description,
    url,
    isPartOf: {
      '@type': 'WebSite',
      name: 'limedl',
      url: SITE_URL,
    },
  };
}

export function techArticleJsonLd(input: {
  headline: string;
  description: string;
  url: string;
  lang: string;
  section?: string;
}) {
  return {
    '@context': 'https://schema.org',
    '@type': 'TechArticle',
    headline: input.headline,
    description: input.description,
    inLanguage: input.lang,
    url: input.url,
    mainEntityOfPage: input.url,
    ...(input.section ? { articleSection: input.section } : {}),
    isPartOf: {
      '@type': 'WebSite',
      name: 'limedl',
      url: SITE_URL,
    },
    author: {
      '@type': 'Organization',
      name: 'limedl',
      url: SITE_URL,
    },
    publisher: {
      '@type': 'Organization',
      name: 'limedl',
      logo: {
        '@type': 'ImageObject',
        url: `${SITE_URL}/logo.png`,
      },
    },
  };
}

export function faqPageJsonLd(questions: { question: string; answer: string }[]) {
  return {
    '@context': 'https://schema.org',
    '@type': 'FAQPage',
    mainEntity: questions.map((item) => ({
      '@type': 'Question',
      name: item.question,
      acceptedAnswer: {
        '@type': 'Answer',
        text: item.answer,
      },
    })),
  };
}
