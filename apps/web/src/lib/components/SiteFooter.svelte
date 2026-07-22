<script lang="ts">
  export let links: Array<{ href: string; label: string }> = [
    { href: '/about', label: 'About' },
    { href: '/changelog', label: 'Changelog' },
    { href: 'https://docs.rs/treease-cli', label: 'CLI docs' },
    { href: '/terms', label: 'Terms' },
    { href: '/privacy', label: 'Privacy' },
  ];
</script>

<footer class="site-footer" aria-label="Site footer">
  <span class="site-footer__brand">© 2026 Treease</span>
  <nav class="site-footer__links" aria-label="Site resources">
    {#each links as item}
      <a href={item.href} rel={item.href.startsWith('http') ? 'me' : undefined}>{item.label}</a>
    {/each}
  </nav>
</footer>

<style>
  .site-footer, .site-footer__links { display: inline-flex; align-items: center; gap: 18px; }
  .site-footer { justify-content: space-between; margin-top: 34px; padding: 12px 0 0; border-top: 1px solid var(--line); color: var(--muted-soft, #718196); font-size: 13px; }
  .site-footer__links a { color: inherit; font-weight: 600; text-decoration: none; transition: color 160ms ease; }
  .site-footer__links a:hover { color: var(--accent-strong); }
  @media (max-width: 640px) {
    .site-footer { flex-direction: column; align-items: flex-start; gap: 10px; }
  }
</style>
