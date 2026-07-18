import { error } from '@sveltejs/kit';
import { getTutorialArticle, tutorialArticles } from '$lib/content/tutorials';

export const prerender = true;

export function entries() {
  return tutorialArticles.map(({ slug }) => ({ slug }));
}

export function load({ params }) {
  const article = getTutorialArticle(params.slug);
  if (!article) {
    throw error(404, 'Tutorial not found');
  }

  return { article };
}
