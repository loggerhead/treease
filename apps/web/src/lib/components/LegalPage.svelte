<script lang="ts">
  import { assetUrl } from '$lib/assets';

  export let title: string
  export let html: string
</script>

<svelte:head>
  <title>{title} | Treease</title>
  <meta name="viewport" content="width=device-width, initial-scale=1" />
</svelte:head>

<div class="legal-page">
  <div class="legal-shell">
    <header class="legal-header">
      <a class="legal-brand" href="/" aria-label="Treease home">
        <img class="legal-brand__logo" src={assetUrl('/treease-logo.png')} alt="Treease logo" />
        <span class="legal-brand__copy">
          <span class="legal-brand__mark">Treease</span>
          <span class="legal-brand__note">Structured text workspace</span>
        </span>
      </a>

      <div class="legal-header__actions">
        <a class="legal-link" href="/">Home</a>
        <a class="legal-cta" href="/editor">Open Editor</a>
      </div>
    </header>

    <main class="legal-main">
      <article class="legal-card">
        <div class="legal-copy">
          {@html html}
        </div>
      </article>
    </main>

    <footer class="legal-footer">
      <span>© 2026 Treease</span>
      <div class="legal-footer__links">
        <a href="/terms">Terms</a>
        <a href="/privacy">Privacy</a>
      </div>
    </footer>
  </div>
</div>

<style>
  .legal-page {
    --accent: #2563eb;
    --accent-strong: #1d4ed8;
    --bg: #eff4fb;
    --bg-strong: #f7faff;
    --surface: rgba(255, 255, 255, 0.9);
    --ink: #0f172a;
    --muted: #4b5563;
    --line: rgba(15, 23, 42, 0.1);
    --shadow: 0 30px 80px rgba(15, 23, 42, 0.12);

    min-height: 100svh;
    background:
      radial-gradient(circle at 12% 10%, rgba(125, 211, 252, 0.2), transparent 30%),
      radial-gradient(circle at 88% 14%, rgba(37, 99, 235, 0.16), transparent 34%),
      linear-gradient(180deg, var(--bg-strong) 0%, var(--bg) 100%);
    color: var(--ink);
  }

  .legal-shell {
    box-sizing: border-box;
    width: min(1120px, 100%);
    margin: 0 auto;
    padding: 24px 24px 56px;
  }

  .legal-header {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    align-items: center;
    gap: 24px;
    min-height: 72px;
    margin-bottom: 28px;
    padding: 12px 18px;
    border: 1px solid var(--line);
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.7);
    backdrop-filter: blur(18px);
    box-shadow: 0 12px 40px rgba(15, 23, 42, 0.08);
  }

  .legal-brand {
    display: inline-flex;
    align-items: center;
    gap: 12px;
    min-width: 0;
    color: inherit;
    text-decoration: none;
  }

  .legal-brand__logo {
    width: 44px;
    height: 44px;
    flex: 0 0 auto;
    object-fit: contain;
  }

  .legal-brand__copy {
    display: inline-flex;
    flex-direction: column;
    min-width: 0;
  }

  .legal-brand__mark {
    font-size: 18px;
    font-weight: 700;
    letter-spacing: -0.04em;
  }

  .legal-brand__note {
    color: var(--muted);
    font-size: 14px;
    white-space: nowrap;
  }

  .legal-header__actions,
  .legal-footer,
  .legal-footer__links {
    display: inline-flex;
    align-items: center;
    gap: 18px;
  }

  .legal-link,
  .legal-footer a {
    color: var(--muted);
    font-size: 14px;
    font-weight: 600;
    text-decoration: none;
    transition: color 160ms ease;
  }

  .legal-link:hover,
  .legal-footer a:hover {
    color: var(--ink);
  }

  .legal-cta {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-height: 48px;
    padding: 0 22px;
    border-radius: 999px;
    background: var(--accent);
    color: #fff;
    font-size: 15px;
    font-weight: 700;
    text-decoration: none;
    box-shadow: 0 16px 30px rgba(37, 99, 235, 0.24);
    transition:
      transform 140ms ease,
      background-color 160ms ease;
  }

  .legal-cta:hover {
    background: var(--accent-strong);
  }

  .legal-cta:active {
    transform: translateY(1px) scale(0.99);
  }

  .legal-card {
    border: 1px solid var(--line);
    border-radius: 32px;
    background: var(--surface);
    box-shadow: var(--shadow);
    padding: 40px;
  }

  .legal-copy {
    max-width: 78ch;
    margin: 0 auto;
    color: var(--muted);
    font-size: 16px;
    line-height: 1.75;
  }

  .legal-copy :global(h1),
  .legal-copy :global(h2),
  .legal-copy :global(p),
  .legal-copy :global(ul) {
    margin: 0;
  }

  .legal-copy :global(h1) {
    color: var(--ink);
    font-size: clamp(2.4rem, 4vw, 3.4rem);
    line-height: 1;
    letter-spacing: -0.04em;
  }

  .legal-copy :global(h2) {
    margin-top: 28px;
    color: var(--ink);
    font-size: 1.2rem;
    line-height: 1.3;
    letter-spacing: -0.02em;
  }

  .legal-copy :global(p),
  .legal-copy :global(ul) {
    margin-top: 14px;
  }

  .legal-copy :global(ul) {
    padding-left: 22px;
  }

  .legal-copy :global(li + li) {
    margin-top: 10px;
  }

  .legal-copy :global(a) {
    color: var(--accent);
    font-weight: 600;
    text-decoration: none;
  }

  .legal-copy :global(a:hover) {
    text-decoration: underline;
  }

  .legal-footer {
    justify-content: space-between;
    margin-top: 20px;
    color: var(--muted);
    font-size: 13px;
  }

  @media (max-width: 720px) {
    .legal-shell {
      padding: 16px 16px 32px;
    }

    .legal-header {
      grid-template-columns: 1fr;
      justify-items: start;
      border-radius: 28px;
      padding: 18px;
    }

    .legal-brand {
      flex-wrap: wrap;
    }

    .legal-header__actions,
    .legal-footer {
      width: 100%;
      justify-content: space-between;
    }

    .legal-card {
      border-radius: 24px;
      padding: 22px;
    }

    .legal-footer {
      flex-direction: column;
      align-items: flex-start;
      gap: 10px;
    }
  }
</style>
