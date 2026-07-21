export type ChangelogSection = {
  heading: string;
  paragraphs: string[];
};

export type ChangelogTutorialLink = {
  href: string;
  label: string;
};

export type ChangelogEntry = {
  slug: string;
  title: string;
  date: string;
  isoDate: string;
  summary: string;
  tags: string[];
  author: string;
  featured?: boolean;
  sections?: ChangelogSection[];
  tutorialLinks?: ChangelogTutorialLink[];
};
export { generatedChangelog as changelogEntries } from './changelog.generated';
