<script lang="ts">
  import { assetUrl, r2Assets } from '$lib/assets';
  import AccountMenu from './AccountMenu.svelte';

  export let navItems: Array<{ href: string; label: string }> = [];
  export let ctaHref = '/editor';
  export let ctaLabel = 'Editor';
  export let onLogin: () => void = () => {};
  export let onLogout: () => Promise<void> = async () => {};
</script>

<header class="site-header">
  <a class="brand" href="/" aria-label="Treease home">
    <img class="brand-logo" src={assetUrl(r2Assets.treeaseLogo)} alt="Treease logo" />
    <span class="brand-copy">
      <span class="brand-mark">Treease</span>
      <span class="brand-note">Structured text workspace</span>
    </span>
  </a>

  <nav class="site-nav" aria-label="Primary">
    {#each navItems as item}
      <a href={item.href}>{item.label}</a>
    {/each}
  </nav>

  <div class="header-actions">
    <AccountMenu variant="landing" {onLogin} {onLogout} />
    <a class="header-cta" href={ctaHref}>{ctaLabel}</a>
  </div>
</header>

<style>
  .site-header {
    display: grid;
    align-items: center;
    grid-template-columns: minmax(0, 1fr) auto auto;
    gap: 24px;
    min-height: 68px;
    margin-bottom: 42px;
    padding: 0 0 18px;
    border-bottom: 1px solid var(--line);
  }

  .brand {
    display: inline-flex;
    align-items: center;
    gap: 12px;
    min-width: 0;
    color: inherit;
    text-decoration: none;
  }

  .brand-logo {
    width: 44px;
    height: 44px;
    flex: 0 0 auto;
    object-fit: contain;
    filter: drop-shadow(0 10px 18px rgba(45, 99, 226, 0.16));
  }

  .brand-copy {
    display: inline-flex;
    flex-direction: column;
    min-width: 0;
  }

  .brand-mark {
    font-family: var(--font-display);
    font-size: 18px;
    font-weight: 700;
    letter-spacing: -0.03em;
  }

  .brand-note {
    color: var(--muted-soft);
    font-size: 13px;
    letter-spacing: 0.02em;
    white-space: nowrap;
  }

  .site-nav {
    display: inline-flex;
    align-items: center;
    gap: 18px;
  }

  .site-nav a {
    color: var(--muted);
    font-size: 14px;
    font-weight: 600;
    text-decoration: none;
    transition: color 160ms ease;
  }

  .site-nav a:hover {
    color: var(--accent-strong);
  }

  .header-cta {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-height: 48px;
    padding: 0 22px;
    border-radius: 999px;
    font-size: 15px;
    font-weight: 700;
    text-decoration: none;
    transition:
      transform 140ms ease,
      border-color 160ms ease,
      background-color 160ms ease,
      color 160ms ease,
      box-shadow 160ms ease;
    color: #ffffff;
    background: linear-gradient(135deg, var(--accent) 0%, var(--accent-strong) 100%);
    box-shadow: 0 16px 34px rgba(23, 69, 181, 0.22);
  }

  .header-cta:hover {
    transform: translateY(-1px);
  }

  .header-actions {
    display: inline-flex;
    align-items: center;
    gap: 14px;
  }

  @media (max-width: 900px) {
    .site-header {
      grid-template-columns: 1fr;
      gap: 16px;
      justify-items: start;
    }

    .site-nav {
      flex-wrap: wrap;
    }
  }
</style>
