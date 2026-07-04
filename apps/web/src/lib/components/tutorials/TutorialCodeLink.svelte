<script lang="ts">
  export let href: string;
  export let label: string;
  export let ariaLabel: string | undefined = undefined;
</script>

<pre class="tutorial-code-link"><a href={href} aria-label={ariaLabel}>{label}</a></pre>

<style>
  .tutorial-code-link {
    margin: 0;
    overflow-x: auto;
    padding: 12px 14px;
    border: 1px solid rgba(45, 99, 226, 0.14);
    border-radius: 14px;
    background: rgba(235, 243, 255, 0.8);
    white-space: pre-wrap;
    word-break: break-all;
  }

  .tutorial-code-link a {
    color: #1745b5;
    font-family: var(--font-mono, "SF Mono", "SFMono-Regular", Menlo, Consolas, monospace);
    font-size: 12px;
    font-weight: 700;
    line-height: 1.6;
    text-decoration: none;
  }

  .tutorial-code-link a:hover {
    color: #0f3591;
    text-decoration: underline;
  }

  .tutorial-code-link a:focus-visible {
    outline: 2px solid #1745b5;
    outline-offset: 3px;
    border-radius: 4px;
  }
</style>
