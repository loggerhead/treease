import { error } from '@sveltejs/kit';
import { changelogEntries } from '$lib/content/changelog';
import type { EntryGenerator, PageLoad } from './$types';

export const entries: EntryGenerator = () => changelogEntries.map((entry) => ({ slug: entry.slug }));

export const load: PageLoad = ({ params }) => {
  const entry = changelogEntries.find((candidate) => candidate.slug === params.slug);
  if (!entry) error(404, 'Changelog entry not found');
  return { entry };
};
