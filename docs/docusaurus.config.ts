import {readFileSync} from 'node:fs';
import path from 'node:path';
import {themes as prismThemes} from 'prism-react-renderer';
import type {Config} from '@docusaurus/types';
import type * as Preset from '@docusaurus/preset-classic';

const releasedVersions = JSON.parse(
  readFileSync(path.join(__dirname, 'versions.json'), 'utf8'),
) as string[];
const latestReleasedVersion = releasedVersions[0] ?? 'current';

const config: Config = {
  title: 'OTVI',
  tagline: 'Open TV Interface – YAML-driven television streaming framework',
  favicon: 'img/favicon.ico',

  future: {
    v4: true,
  },

  url: 'https://otvi.rbls.eu.org',
  baseUrl: '/',

  organizationName: 'rabilrbl',
  projectName: 'otvi',

  onBrokenLinks: 'throw',
  markdown: {
    hooks: {
      onBrokenMarkdownLinks: 'throw',
    },
  },

  i18n: {
    defaultLocale: 'en',
    locales: ['en'],
  },

  presets: [
    [
      'classic',
      {
        docs: {
          sidebarPath: './sidebars.ts',
          editUrl: 'https://github.com/rabilrbl/otvi/tree/main/docs/',
          editCurrentVersion: true,
          showLastUpdateTime: true,
          lastVersion: latestReleasedVersion,
          versions: {
            current: {
              label: 'Next',
            },
          },
        },
        blog: {
          routeBasePath: 'blogs',
          showReadingTime: true,
          editUrl: 'https://github.com/rabilrbl/otvi/tree/main/docs/',
          blogTitle: 'OTVI Blog',
          blogDescription: 'Release notes, documentation updates, and project announcements for OTVI.',
          blogSidebarTitle: 'Recent posts',
          blogSidebarCount: 10,
          feedOptions: {
            type: ['rss', 'atom'],
            xslt: true,
          },
          onInlineTags: 'warn',
          onInlineAuthors: 'warn',
          onUntruncatedBlogPosts: 'warn',
        },
        theme: {
          customCss: './src/css/custom.css',
        },
      } satisfies Preset.Options,
    ],
  ],

  themeConfig: {
    colorMode: {
      respectPrefersColorScheme: true,
    },
    navbar: {
      title: 'OTVI',
      items: [
        {
          type: 'docSidebar',
          sidebarId: 'docsSidebar',
          position: 'left',
          label: 'Docs',
        },
        {
          type: 'docsVersionDropdown',
          position: 'right',
        },
        {
          to: '/blogs',
          label: 'Blogs',
          position: 'left',
        },
        {
          href: 'https://github.com/rabilrbl/otvi',
          label: 'GitHub',
          position: 'right',
        },
      ],
    },
    footer: {
      style: 'dark',
      links: [
        {
          title: 'Documentation',
          items: [
            {label: 'Getting Started', to: '/docs/getting-started'},
            {label: 'Provider Guide', to: '/docs/providers/overview'},
            {label: 'API Reference', to: '/docs/api-reference/overview'},
          ],
        },
        {
          title: 'More',
          items: [
            {
              label: 'Blog',
              to: '/blogs',
            },
            {
              label: 'GitHub',
              href: 'https://github.com/rabilrbl/otvi',
            },
          ],
        },
      ],
      copyright: `Copyright © ${new Date().getFullYear()} OTVI. Licensed under CC BY-NC-SA 4.0. Built with Docusaurus.`,
    },
    prism: {
      theme: prismThemes.github,
      darkTheme: prismThemes.dracula,
      additionalLanguages: ['bash', 'yaml', 'rust', 'json', 'toml', 'docker'],
    },
  } satisfies Preset.ThemeConfig,
};

export default config;
