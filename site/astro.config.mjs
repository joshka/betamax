import { defineConfig } from 'astro/config';
import mdx from '@astrojs/mdx';
import starlight from '@astrojs/starlight';

const base = '/betamax';
const canonicalBasePathScript = [
  `if (location.pathname === '${base}') {`,
  `  location.replace('${base}/' + location.search + location.hash);`,
  '}',
].join('\n');

export default defineConfig({
  site: 'https://www.joshka.net',
  base,
  integrations: [
    starlight({
      title: 'Betamax',
      description: 'Rust-first terminal captures, GIFs, screenshots, and terminal snapshots.',
      customCss: ['./src/styles/custom.css'],
      routeMiddleware: './src/starlightRouteData.ts',
      head: [
        {
          tag: 'script',
          content: canonicalBasePathScript,
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
            { label: 'Overview', slug: 'overview' },
            { label: 'Quick Start', slug: 'quick-start' },
            { label: 'Examples', slug: 'examples' },
          ],
        },
        {
          label: 'Authoring',
          items: [
            { label: 'Tape Files', slug: 'authoring/tape-files' },
            { label: 'Outputs', slug: 'authoring/outputs' },
            { label: 'Generated Media Storage', slug: 'authoring/generated-media' },
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
    mdx({ gfm: true, optimize: true }),
  ],
});
