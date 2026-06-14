<script lang="ts">
  export let width = '100%';
  export let height = '10px';
  export let ariaHidden = true;
</script>

<span class="runtime-loading-bar" aria-hidden={ariaHidden} style:width style:height></span>

<style>
  .runtime-loading-bar {
    position: relative;
    display: block;
    overflow: hidden;
    border-radius: 999px;
    background: rgba(148, 163, 184, 0.18);
  }

  .runtime-loading-bar::after {
    position: absolute;
    inset: 0;
    background: linear-gradient(90deg, transparent, rgba(255, 255, 255, 0.82), transparent);
    content: "";
    transform: translateX(-100%);
    animation: runtime-loading-bar-shimmer 1400ms ease-in-out infinite;
  }

  @keyframes runtime-loading-bar-shimmer {
    100% {
      transform: translateX(100%);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .runtime-loading-bar::after {
      animation: none;
    }
  }
</style>
