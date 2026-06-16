import { defineRouteMiddleware } from '@astrojs/starlight/route-data';

import ogImage from './assets/og.png';

export const onRequest = defineRouteMiddleware((context) => {
  // Importing the image keeps it in Astro's asset graph, so the URL changes when the image does.
  const ogImageUrl = new URL(ogImage.src, context.site ?? 'https://www.joshka.net');

  context.locals.starlightRoute.head.push(
    {
      tag: 'meta',
      attrs: {
        property: 'og:image',
        content: ogImageUrl.href,
      },
    },
    {
      tag: 'meta',
      attrs: {
        property: 'og:image:width',
        content: String(ogImage.width),
      },
    },
    {
      tag: 'meta',
      attrs: {
        property: 'og:image:height',
        content: String(ogImage.height),
      },
    },
    {
      tag: 'meta',
      attrs: {
        name: 'twitter:image',
        content: ogImageUrl.href,
      },
    },
  );
});
