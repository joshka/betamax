import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';

export default defineConfig({
  site: 'https://www.joshka.net',
  base: '/betamax',
  integrations: [
    starlight({
      title: 'Betamax',
      description: 'Rust-first terminal captures, GIFs, screenshots, and terminal snapshots.',
      customCss: ['./src/styles/custom.css'],
      head: [
        {
          tag: 'meta',
          attrs: {
            property: 'og:image',
            content: 'https://www.joshka.net/betamax/og.png',
          },
        },
        {
          tag: 'meta',
          attrs: {
            property: 'og:image:width',
            content: '1200',
          },
        },
        {
          tag: 'meta',
          attrs: {
            property: 'og:image:height',
            content: '630',
          },
        },
        {
          tag: 'meta',
          attrs: {
            name: 'twitter:image',
            content: 'https://www.joshka.net/betamax/og.png',
          },
        },
      ],
      editLink: {
        baseUrl: 'https://github.com/joshka/betamax/edit/main/',
      },
      social: [
        {
          icon: 'github',
          label: 'GitHub',
          href: 'https://github.com/joshka/betamax',
        },
      ],
      sidebar: [
        {
          label: 'Start',
          items: [
            { label: 'Overview', slug: 'index' },
            { label: 'Quick Start', slug: 'quick-start' },
            { label: 'Examples', slug: 'examples' },
          ],
        },
        {
          label: 'Authoring',
          items: [
            { label: 'Tape Files', slug: 'authoring/tape-files' },
            { label: 'Outputs', slug: 'authoring/outputs' },
            { label: 'Themes And Styling', slug: 'authoring/themes' },
          ],
        },
        {
          label: 'Testing',
          items: [
            { label: 'Terminal Testing', slug: 'testing/terminal-testing' },
            { label: 'State JSON', slug: 'testing/state-json' },
          ],
        },
        {
          label: 'Reference',
          items: [
            { label: 'Tape Reference', slug: 'reference/tape-reference' },
            { label: 'Differences From VHS', slug: 'reference/vhs-differences' },
            { label: 'Roadmap', slug: 'reference/roadmap' },
            { label: 'Development', slug: 'reference/development' },
          ],
        },
      ],
    }),
  ],
});
