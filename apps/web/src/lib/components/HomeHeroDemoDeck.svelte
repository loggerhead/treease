<script lang="ts">
  import { assetUrl } from '$lib/assets';
  import { onMount, tick } from 'svelte';

  type DemoItem = {
    id: string;
    label: string;
    title: string;
    description: string;
    poster: string;
    video: string;
    accent: string;
  };

  const demos: DemoItem[] = [
    {
      id: 'graph',
      label: 'Graph',
      title: 'Graph View',
      description: 'Trace paths, hierarchy, and node relationships in the graph.',
      poster: assetUrl('/landing/hero-demo-graph.png'),
      video: assetUrl('/landing/hero-demo-graph.mp4'),
      accent: '#2563eb'
    },
    {
      id: 'compare',
      label: 'Compare',
      title: 'Compare',
      description: 'Review structured diffs and text fallback in the same workspace.',
      poster: assetUrl('/landing/hero-demo-compare.png'),
      video: assetUrl('/landing/hero-demo-compare.mp4'),
      accent: '#f97316'
    },
    {
      id: 'preview',
      label: 'Editor Preview',
      title: 'Editor Preview',
      description: 'Preview hover values and export results right beside the editor.',
      poster: assetUrl('/landing/hero-demo-preview.png'),
      video: assetUrl('/landing/hero-demo-preview.mp4'),
      accent: '#14b8a6'
    }
  ];

  let activeDemo = 0;
  let deckPaused = false;
  let loadRequested = demos.map((_, index) => index === 0);
  let videoRefs: Array<HTMLVideoElement | null> = demos.map(() => null);
  let deckStage: HTMLDivElement | null = null;

  function getLayer(index: number): number {
    return (index - activeDemo + demos.length) % demos.length;
  }

  function setActiveDemo(index: number): void {
    activeDemo = index;
  }

  async function ensureVideo(index: number): Promise<HTMLVideoElement | null> {
    if (!loadRequested[index]) {
      loadRequested = loadRequested.map((value, currentIndex) =>
        currentIndex === index ? true : value
      );
      await tick();
    }

    const video = videoRefs[index];
    if (!video) return null;
    video.load();
    return video;
  }

  async function playDemo(index: number): Promise<void> {
    const video = await ensureVideo(index);
    if (!video) return;
    video.currentTime = 0;
    try {
      await video.play();
    } catch {
      // Ignore autoplay failures on non-hover surfaces.
    }
  }

  function pauseAll(): void {
    for (const video of videoRefs) {
      video?.pause();
    }
  }

  function handleCardEnter(index: number): void {
    if (index !== activeDemo) return;
    deckPaused = true;
    void playDemo(index);
  }

  function handleCardSelect(index: number): void {
    deckPaused = true;
    pauseAll();
    setActiveDemo(index);
  }

  function handleDeckLeave(): void {
    deckPaused = false;
    pauseAll();
  }

  onMount(() => {
    deckStage?.addEventListener('mouseleave', handleDeckLeave);

    const interval = window.setInterval(() => {
      if (deckPaused) return;
      activeDemo = (activeDemo + 1) % demos.length;
    }, 4200);

    return () => {
      deckStage?.removeEventListener('mouseleave', handleDeckLeave);
      pauseAll();
      window.clearInterval(interval);
    };
  });
</script>

<section class="hero-demo-deck" aria-label="Treease feature demos">
  <div class="deck-stage" bind:this={deckStage}>
    {#each demos as demo, index}
      {@const layer = getLayer(index)}
      <article
        class:demo-card--active={index === activeDemo}
        class="demo-card"
        style={`--accent:${demo.accent}; --layer:${layer}; --z:${demos.length - layer};`}
      >
        <button
          type="button"
          class="demo-card__button"
          aria-label={`${demo.label} demo`}
          on:mouseenter={() => handleCardEnter(index)}
          on:focus={() => handleCardEnter(index)}
          on:click={() => handleCardSelect(index)}
        >
          <div class="demo-card__media">
            <img
              src={demo.poster}
              alt={demo.title}
              loading={index === 0 ? 'eager' : 'lazy'}
            />

            {#if loadRequested[index]}
              <video
                bind:this={videoRefs[index]}
                class:demo-card__video--visible={index === activeDemo}
                class="demo-card__video"
                muted
                loop
                playsinline
                preload="none"
                poster={demo.poster}
              >
                <source src={demo.video} type="video/mp4" />
              </video>
            {/if}

            <div class="demo-card__glow" aria-hidden="true"></div>
          </div>

          <div class="demo-card__caption">
            <div>
              <strong>{demo.title}</strong>
              <p>{demo.description}</p>
            </div>
            <span class="demo-card__index">0{index + 1}</span>
          </div>
        </button>
      </article>
    {/each}
  </div>
</section>

<style>
  .hero-demo-deck {
    position: relative;
  }

  .deck-stage {
    position: relative;
    min-height: 500px;
    padding: 0 72px 12px 0;
  }

  .demo-card {
    position: absolute;
    inset: auto 0 0 auto;
    width: min(100%, 560px);
    z-index: var(--z);
    transform:
      translate3d(calc(var(--layer) * 54px), calc(var(--layer) * 28px), 0)
      scale(calc(1 - (var(--layer) * 0.04)));
    transform-origin: top right;
    opacity: calc(1 - (var(--layer) * 0.12));
    transition:
      transform 420ms cubic-bezier(0.2, 0.9, 0.2, 1),
      opacity 240ms ease,
      filter 240ms ease;
    filter: saturate(calc(1 - (var(--layer) * 0.16)));
  }

  .demo-card--active {
    filter: none;
    opacity: 1;
  }

  .demo-card__button {
    display: flex;
    flex-direction: column;
    width: 100%;
    padding: 0;
    appearance: none;
    -webkit-appearance: none;
    border: 1px solid rgba(255, 255, 255, 0.35);
    border-radius: 30px;
    background:
      linear-gradient(180deg, rgba(255, 255, 255, 0.84), rgba(232, 239, 251, 0.94));
    box-shadow:
      0 32px 80px rgba(15, 23, 42, 0.2),
      inset 0 1px 0 rgba(255, 255, 255, 0.7);
    overflow: hidden;
    text-align: left;
    cursor: pointer;
    backdrop-filter: blur(20px);
    outline: none;
  }

  .demo-card__caption {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 14px;
    padding: 16px 18px;
  }

  .demo-card__index {
    font-size: 12px;
    font-weight: 800;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  .demo-card__index {
    color: var(--muted-soft, #6b7280);
  }

  .demo-card__media {
    position: relative;
    aspect-ratio: 12 / 8.6;
    overflow: hidden;
    isolation: isolate;
    background: #f8fbff;
    pointer-events: none;
  }

  .demo-card__media img,
  .demo-card__video {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
    background: #f8fbff;
    pointer-events: none;
  }

  .demo-card__video {
    opacity: 0;
    transition: opacity 220ms ease;
  }

  .demo-card__button:focus-visible {
    border-color: color-mix(in srgb, var(--accent) 38%, rgba(255, 255, 255, 0.35));
    box-shadow:
      0 32px 80px rgba(15, 23, 42, 0.2),
      inset 0 1px 0 rgba(255, 255, 255, 0.7),
      0 0 0 3px color-mix(in srgb, var(--accent) 22%, transparent);
  }

  .demo-card__video--visible {
    opacity: 1;
  }

  .demo-card__glow {
    position: absolute;
    inset: auto -12% -24% 24%;
    height: 48%;
    border-radius: 999px;
    background: radial-gradient(circle, color-mix(in srgb, var(--accent) 40%, transparent) 0%, transparent 70%);
    filter: blur(28px);
    opacity: 0.9;
    pointer-events: none;
  }

  .demo-card__caption strong {
    display: block;
    color: var(--ink, #0f172a);
    font-size: 17px;
    letter-spacing: -0.03em;
  }

  .demo-card__caption p {
    margin: 6px 0 0;
    color: var(--muted, #4b5563);
    font-size: 14px;
    line-height: 1.55;
  }

  @media (max-width: 1080px) {
    .deck-stage {
      min-height: 452px;
      padding-right: 54px;
    }
  }

  @media (max-width: 860px) {
    .deck-stage {
      min-height: auto;
      padding: 0;
    }

    .demo-card {
      position: relative;
      width: 100%;
      margin-top: -42px;
      transform: none;
      opacity: 1;
      filter: none;
    }

    .demo-card:first-child {
      margin-top: 0;
    }

    .demo-card__button {
      border-radius: 26px;
    }
  }

  @media (prefers-color-scheme: dark) {
    .demo-card__button {
      border-color: rgba(148, 163, 184, 0.16);
      background:
        linear-gradient(180deg, rgba(11, 20, 36, 0.88), rgba(15, 23, 42, 0.94));
      box-shadow:
        0 28px 70px rgba(2, 6, 23, 0.42),
        inset 0 1px 0 rgba(255, 255, 255, 0.04);
    }

    .demo-card__media {
      background: #0f1b31;
    }
  }
</style>
