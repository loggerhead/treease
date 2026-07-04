import type { TutorialArticle } from './types';

const REMOTE_JSON_URL =
  'https://raw.githubusercontent.com/loggerhead/treease-testdata/refs/heads/main/json/complex.1.json';

function encodeEditorHref(params: Record<string, string>): string {
  const search = new URLSearchParams();
  for (const [key, value] of Object.entries(params)) {
    search.set(key, value);
  }
  return `/editor?${search.toString()}`;
}

function displayEditorHref(params: Record<string, string>): string {
  return `/editor?${Object.entries(params)
    .map(([key, value]) => `${key}=${value}`)
    .join('&')}`;
}

const tutorials: TutorialArticle[] = [
  {
    slug: 'url-parameters',
    title: 'Treease URL Parameters Tutorial',
    description:
      'Learn what each Treease editor URL parameter does, including text, textUrl, rightText, rightTextUrl, lang, ui, command, yq, nest, and autoFormat.',
    eyebrow: 'Tutorial',
    lede:
      'Treease can open the editor in a prepared state directly from the URL. This guide explains what each supported parameter controls and shows the shortest practical example for each one.',
    keywords: [
      'treease url parameters',
      'treease editor url preset',
      'json editor query parameters',
      'json editor texturl',
      'json compare url preset',
    ],
    readingMinutes: 8,
    updatedOn: '2026-07-04',
    sections: [
      {
        title: 'What the editor URL preset is for',
        paragraphs: [
          'Treease uses query parameters on the editor route to prefill source text, open a right-side preview, trigger common commands, and adjust the visible shell around the editor. This makes the editor easier to share in documentation, support links, internal runbooks, and SEO landing pages.',
          'The URL preset is especially useful when you want to show a user a concrete starting point instead of asking them to paste data by hand. You can send a link that opens Treease with source text already present, a comparison view already visible, or a preset yq expression ready to run.',
        ],
      },
      {
        title: 'Parameters that set source text',
        summary: 'These parameters control what appears in the main source editor.',
        parameters: [
          {
            name: 'text',
            purpose: 'Sets the source editor text directly from the query string.',
            href: encodeEditorHref({ lang: 'json', text: '{"hello":"world"}' }),
            displayHref: displayEditorHref({ lang: 'json', text: '{"hello":"world"}' }),
            note: 'Use this when the payload is short enough to live comfortably in the URL itself.',
          },
          {
            name: 'textUrl',
            purpose: 'Fetches source text from a URL and loads the fetched response into the source editor.',
            href: encodeEditorHref({ textUrl: REMOTE_JSON_URL }),
            displayHref: displayEditorHref({ textUrl: REMOTE_JSON_URL }),
            note: 'This is best when the source document is too large or too dynamic to inline in the URL.',
          },
          {
            name: 'lang',
            purpose: 'Sets the editor language used for the imported source text.',
            href: encodeEditorHref({ lang: 'yaml', text: 'service: api\nport: 8080' }),
            displayHref: displayEditorHref({ lang: 'yaml', text: 'service: api\nport: 8080' }),
            note: 'Use this when the file format should be explicit instead of inferred from context.',
          },
        ],
      },
      {
        title: 'Parameters that control the right-side preview',
        summary: 'These parameters prepare the preview pane for compare or reference workflows.',
        parameters: [
          {
            name: 'rightText',
            purpose: 'Sets the text shown in the right-side preview editor.',
            href: encodeEditorHref({ lang: 'json', text: '{"left":1}', rightText: '{"right":2}' }),
            displayHref: displayEditorHref({ lang: 'json', text: '{"left":1}', rightText: '{"right":2}' }),
            note: 'Use this when you want a second document visible immediately for manual inspection or compare.',
          },
          {
            name: 'rightTextUrl',
            purpose: 'Fetches preview text from a URL and shows the fetched response in the right-side preview.',
            href: encodeEditorHref({ rightTextUrl: REMOTE_JSON_URL }),
            displayHref: displayEditorHref({ rightTextUrl: REMOTE_JSON_URL }),
            note: 'This works well for linking a live or hosted baseline file into a compare workflow.',
          },
        ],
      },
      {
        title: 'Parameters that run editor actions',
        summary: 'These parameters ask Treease to do work after the editor opens.',
        parameters: [
          {
            name: 'command',
            purpose: 'Runs a built-in editor action such as format, minify, sort, escape, unescape, or compare.',
            href: encodeEditorHref({ ui: 'viewer', text: '{"b":2,"a":1}', command: 'format' }),
            displayHref: displayEditorHref({ ui: 'viewer', text: '{"b":2,"a":1}', command: 'format' }),
            note: 'This is useful for task-oriented landing pages like “format JSON online” or “compare JSON online”.',
          },
          {
            name: 'yq',
            purpose: 'Runs a yq preview expression against the current source document.',
            href: encodeEditorHref({ text: '{"service":{"port":8080}}', yq: '.service' }),
            displayHref: displayEditorHref({ text: '{"service":{"port":8080}}', yq: '.service' }),
            note: 'This is helpful when the tutorial needs to demonstrate extracting or transforming a structured subset.',
          },
        ],
      },
      {
        title: 'Parameters that shape the editor shell',
        summary: 'These parameters control how much of the interface is visible and how the parser behaves.',
        parameters: [
          {
            name: 'ui',
            purpose: 'Controls whether the editor pane, viewer pane, top bar, and bottom bar are visible.',
            href: encodeEditorHref({ ui: 'editor,viewer,topbar,bottombar' }),
            displayHref: displayEditorHref({ ui: 'editor,viewer,topbar,bottombar' }),
            note: 'Use this to create focused landing pages, such as viewer-only format pages or cleaner embedded docs demos.',
          },
          {
            name: 'nest',
            purpose: 'Toggles nested JSON parsing behavior in the parser settings.',
            href: encodeEditorHref({ text: '{"payload":"{\\"ok\\":true}"}', nest: 'true' }),
            displayHref: displayEditorHref({ text: '{"payload":"{\\"ok\\":true}"}', nest: 'true' }),
            note: 'This is relevant when the source includes JSON serialized inside a string field.',
          },
          {
            name: 'autoFormat',
            purpose: 'Controls whether smart formatting stays enabled for the prepared editor session.',
            href: encodeEditorHref({ text: '{"compact":true}', autoFormat: 'false' }),
            displayHref: displayEditorHref({ text: '{"compact":true}', autoFormat: 'false' }),
            note: 'Use this when a tutorial should preserve exact source layout instead of normalizing it automatically.',
          },
        ],
      },
      {
        title: 'Example landing-page patterns',
        bullets: [
          'Use `text` plus `command=format` for a “format JSON online” landing page.',
          'Use `text` plus `command=minify` for a “minify JSON” landing page.',
          'Use `text` plus `rightText` plus `command=compare` for a “compare JSON” landing page.',
          'Use `textUrl` for docs or support pages that should load a maintained remote sample instead of a hard-coded payload.',
          'Use `ui=viewer` when the page should feel like a focused tool flow rather than a full workspace.',
        ],
      },
    ],
    faq: [
      {
        question: 'What does textUrl do in Treease?',
        answer:
          'The `textUrl` parameter fetches text from a URL and inserts the fetched response into the source editor when the page opens.',
      },
      {
        question: 'What does rightTextUrl do in Treease?',
        answer:
          'The `rightTextUrl` parameter fetches text from a URL and places the fetched response in the right-side preview pane.',
      },
      {
        question: 'What does lang do in Treease?',
        answer:
          'The `lang` parameter sets the editor language for the prepared session so the imported text is treated as JSON, YAML, TOML, or another supported format.',
      },
    ],
    relatedSlugs: ['format-json-online', 'compare-json-structurally', 'url-to-json-editor'],
  },
  {
    slug: 'format-json-online',
    title: 'How to Format JSON Online with Treease',
    description:
      'Format JSON online while keeping the source text attached to a visual graph view. Learn how Treease prepares JSON for review and export.',
    eyebrow: 'Tutorial',
    lede:
      'Formatting JSON is most useful when it improves review, debugging, and handoff. Treease formats the current document in place, while keeping the graph and source text attached to the same working state.',
    keywords: ['format json online', 'json formatter online', 'pretty print json', 'treease format json'],
    readingMinutes: 5,
    updatedOn: '2026-07-04',
    sections: [
      {
        title: 'Why format JSON before reviewing it',
        paragraphs: [
          'Dense JSON hides structure. Formatting adds spacing, indentation, and line breaks so nested objects and arrays become easier to scan.',
          'Treease is useful here because formatting does not disconnect the source text from the structure view. You can format the source and still inspect the graph, trace a field path, or compare the result before exporting it.',
        ],
      },
      {
        title: 'Ways to open a formatting workflow',
        examples: [
          {
            label: 'Inline JSON formatting',
            href: encodeEditorHref({ ui: 'viewer', text: '{"b":2,"a":1}', command: 'format' }),
            displayHref: displayEditorHref({ ui: 'viewer', text: '{"b":2,"a":1}', command: 'format' }),
            description: 'Opens Treease with compact JSON and runs the format action immediately.',
          },
          {
            label: 'Load a remote sample then review it',
            href: encodeEditorHref({ textUrl: REMOTE_JSON_URL }),
            displayHref: displayEditorHref({ textUrl: REMOTE_JSON_URL }),
            description: 'Loads hosted JSON into the source editor so the document can be formatted and reviewed.',
          },
        ],
      },
      {
        title: 'What makes this useful for SEO landing pages',
        bullets: [
          'The page can open straight into a formatting flow.',
          'The same landing page can show structure, not only plain pretty-printed text.',
          'Related actions like sort, compare, preview export, and graph inspection remain one click away.',
        ],
      },
    ],
    faq: [
      {
        question: 'Can Treease format JSON from a URL?',
        answer:
          'Yes. Use `textUrl` to load hosted JSON into the source editor, then apply formatting in the same editor session.',
      },
    ],
    relatedSlugs: ['url-parameters', 'compare-json-structurally', 'url-to-json-editor'],
  },
  {
    slug: 'compare-json-structurally',
    title: 'How to Compare JSON Structurally',
    description:
      'Compare JSON structurally in Treease by preparing a left document and a right preview document, then running compare inside the editor workflow.',
    eyebrow: 'Tutorial',
    lede:
      'Text diffs alone can be noisy when two JSON documents differ mostly in formatting or key order. Treease starts from the structured document view, then brings compare into the same workspace.',
    keywords: ['compare json structurally', 'json compare online', 'json semantic diff', 'treease compare json'],
    readingMinutes: 5,
    updatedOn: '2026-07-04',
    sections: [
      {
        title: 'Why structural comparison matters',
        paragraphs: [
          'Two JSON documents can look different as text while still representing nearly the same structure. Structural comparison reduces the noise from line wrapping, indentation, and layout differences.',
          'Treease keeps the source document and the right-side comparison text in one editor route, which makes it suitable for support docs, migration guides, and comparison landing pages.',
        ],
      },
      {
        title: 'A prepared compare link',
        examples: [
          {
            label: 'Inline compare example',
            href: encodeEditorHref({
              text: '{"service":{"port":8080}}',
              rightText: '{"service":{"port":9090}}',
              command: 'compare',
            }),
            displayHref: displayEditorHref({
              text: '{"service":{"port":8080}}',
              rightText: '{"service":{"port":9090}}',
              command: 'compare',
            }),
            description: 'Opens both sides and runs compare in the prepared session.',
          },
          {
            label: 'Remote baseline compare',
            href: encodeEditorHref({
              textUrl: REMOTE_JSON_URL,
              rightTextUrl: REMOTE_JSON_URL,
              command: 'compare',
            }),
            displayHref: displayEditorHref({
              textUrl: REMOTE_JSON_URL,
              rightTextUrl: REMOTE_JSON_URL,
              command: 'compare',
            }),
            description: 'Loads two hosted documents into the comparison workflow.',
          },
        ],
      },
      {
        title: 'When to use this article',
        bullets: [
          'For “compare JSON online” landing pages.',
          'For migration guides that show a before-and-after payload.',
          'For docs that want to explain Treease compare parameters without explaining internal precedence details.',
        ],
      },
    ],
    faq: [
      {
        question: 'How do I prefill both sides of compare in Treease?',
        answer:
          'Use `text` or `textUrl` for the left document and `rightText` or `rightTextUrl` for the right-side preview, then open the page with `command=compare`.',
      },
    ],
    relatedSlugs: ['url-parameters', 'format-json-online', 'url-to-json-editor'],
  },
  {
    slug: 'url-to-json-editor',
    title: 'How to Load URL Content into a JSON Editor',
    description:
      'Use Treease textUrl and rightTextUrl parameters to load hosted JSON or other structured text into the editor and preview panes.',
    eyebrow: 'Tutorial',
    lede:
      'Sometimes the best editor link is not a pasted sample but a hosted document. Treease supports URL-backed editor presets so landing pages and support docs can open live or maintained samples directly in the editor.',
    keywords: ['url to json editor', 'load json from url', 'json editor from url', 'treease texturl'],
    readingMinutes: 4,
    updatedOn: '2026-07-04',
    sections: [
      {
        title: 'When URL-backed input is useful',
        paragraphs: [
          'URL-backed input works well when the sample is maintained elsewhere, when the payload is too large for a practical query string, or when the same landing page should always reflect a current example document.',
          'Treease supports source-editor loading and right-side preview loading through dedicated URL parameters so the same mechanism can power import, reference, and compare tutorials.',
        ],
      },
      {
        title: 'Source and preview URL parameters',
        parameters: [
          {
            name: 'textUrl',
            purpose: 'Loads fetched content into the source editor.',
            href: encodeEditorHref({ textUrl: REMOTE_JSON_URL }),
            displayHref: displayEditorHref({ textUrl: REMOTE_JSON_URL }),
          },
          {
            name: 'rightTextUrl',
            purpose: 'Loads fetched content into the right-side preview pane.',
            href: encodeEditorHref({ rightTextUrl: REMOTE_JSON_URL }),
            displayHref: displayEditorHref({ rightTextUrl: REMOTE_JSON_URL }),
          },
        ],
      },
      {
        title: 'Practical use cases',
        bullets: [
          'Hosted sample payloads in product documentation.',
          'Support links that open the editor with a known reference document.',
          'Comparison landing pages that point to a baseline file and a current file.',
        ],
      },
    ],
    faq: [
      {
        question: 'Which Treease parameter loads JSON from a URL?',
        answer:
          'Use `textUrl` to load fetched content into the source editor. Use `rightTextUrl` to load fetched content into the right-side preview pane.',
      },
    ],
    relatedSlugs: ['url-parameters', 'compare-json-structurally', 'format-json-online'],
  },
];

const tutorialMap = new Map(tutorials.map((article) => [article.slug, article]));

export const tutorialArticles = tutorials;

export function getTutorialArticle(slug: string): TutorialArticle | null {
  return tutorialMap.get(slug) ?? null;
}

export function getRelatedTutorialArticles(article: TutorialArticle): TutorialArticle[] {
  return article.relatedSlugs
    .map((slug) => tutorialMap.get(slug))
    .filter((candidate): candidate is TutorialArticle => candidate != null);
}
