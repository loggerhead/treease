export type TutorialExample = {
  label: string;
  href: string;
  displayHref?: string;
  description: string;
};

export type TutorialParameter = {
  name: string;
  purpose: string;
  href: string;
  displayHref?: string;
  note?: string;
};

export type TutorialSection = {
  title: string;
  summary?: string;
  paragraphs?: string[];
  bullets?: string[];
  examples?: TutorialExample[];
  parameters?: TutorialParameter[];
};

export type TutorialFaqItem = {
  question: string;
  answer: string;
};

export type TutorialArticle = {
  slug: string;
  title: string;
  description: string;
  eyebrow: string;
  lede: string;
  keywords: string[];
  readingMinutes: number;
  updatedOn: string;
  sections: TutorialSection[];
  faq: TutorialFaqItem[];
  relatedSlugs: string[];
};
