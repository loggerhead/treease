import { error } from '@sveltejs/kit';
import { getTutorialArticle } from '$lib/content/tutorials';

export function load({ params }) {
  const article = getTutorialArticle(params.slug);
  if (!article) {
    throw error(404, 'Tutorial not found');
  }

  return { article };
}
