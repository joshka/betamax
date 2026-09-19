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
            { label: 'Quick start', slug: 'quick-start' },
            { label: 'Examples', slug: 'examples' },
          ],
        },
        {
          label: 'Authoring',
          items: [
            { label: 'Tape files', slug: 'authoring/tape-files' },
            { label: 'Input and keys', slug: 'authoring/input-and-keys' },
            { label: 'Outputs', slug: 'authoring/outputs' },
            { label: 'Presentation overlays', slug: 'authoring/presentation-overlays' },
            { label: 'Generated media storage', slug: 'authoring/generated-media' },
            { label: 'Themes and styling', slug: 'authoring/themes' },
          ],
        },
        {
          label: 'Testing',
          items: [
            { label: 'Terminal testing', slug: 'testing/terminal-testing' },
            { label: 'Feature field guide', slug: 'testing/feature-guide' },
            { label: 'State JSON', slug: 'testing/state-json' },
          ],
        },
        {
          label: 'Reference',
          items: [
            { label: 'Tape reference', slug: 'reference/tape-reference' },
            { label: 'Differences from VHS', slug: 'reference/vhs-differences' },
            { label: 'Roadmap', slug: 'reference/roadmap' },
            { label: 'Development', slug: 'reference/development' },
          ],
        },
      ],
    }),
    mdx({ gfm: true, optimize: true }),
  ],
});
