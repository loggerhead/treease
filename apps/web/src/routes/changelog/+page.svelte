<svelte:head>
  <title>Treease Changelog | Structured Text Workspace</title>
  <meta
    name="description"
    content="Read the latest Treease product, editor, tutorial, and CLI updates."
  />
  <link rel="canonical" href="https://treease.com/changelog" />
</svelte:head>

<main class="changelog-page">
  <a href="/">← Treease home</a>
  <p class="eyebrow">Changelog</p>
  <h1>What is changing in Treease.</h1>
  <p class="lede">Product and documentation updates for the structured text workspace.</p>

  <article>
    <p class="date">2026-07-22</p>
    <h2>Search-ready tool guides</h2>
    <ul>
      <li>Added JSON viewer, formatter, and compare guides under <code>/tutorial</code>.</li>
      <li>Added direct examples for opening prepared Treease editor workflows.</li>
      <li>Connected related tutorials so users can move between viewing, formatting, and comparing.</li>
      <li>Added About, changelog, open-source, and CLI documentation links.</li>
    </ul>
  </article>
</main>

<style>
  .changelog-page { max-width: 760px; margin: 0 auto; padding: 56px 24px 80px; color: #10192a; }
  .changelog-page a { color: #1745b5; }
  .eyebrow { margin: 56px 0 12px; color: #2d63e2; font-weight: 700; letter-spacing: .12em; text-transform: uppercase; }
  h1 { margin: 0; font-size: clamp(2.4rem, 6vw, 4.5rem); line-height: 1.02; letter-spacing: -.05em; }
  h2 { margin: 10px 0 18px; font-size: 1.6rem; }
  p, li { line-height: 1.75; }
  .lede { color: #536273; font-size: 1.2rem; }
  .date { margin: 52px 0 0; color: #718196; font-weight: 700; }
</style>
