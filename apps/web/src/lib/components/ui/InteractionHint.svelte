<script lang="ts">
  import { Keyboard, MousePointer2, X } from 'lucide-svelte';

  type InteractionHintToken = string | { key: string };

  export let icon: 'keyboard' | 'pointer';
  export let label: string;
  export let tokens: InteractionHintToken[];
  export let fading = false;
  export let testId: string | undefined = undefined;
  export let dismissLabel: string;
  export let onDismiss: () => void;
</script>

<div
  class="interaction-hint"
  class:interaction-hint--fading={fading}
  data-testid={testId}
  role="status"
  aria-live="polite"
>
  <span class="interaction-hint__icon" aria-hidden="true">
    {#if icon === 'keyboard'}
      <Keyboard size={15} strokeWidth={2} />
    {:else}
      <MousePointer2 size={15} strokeWidth={2} />
    {/if}
  </span>
  <span class="interaction-hint__copy">
    <span class="interaction-hint__label">{label}</span>
    <span class="interaction-hint__message">
      {#each tokens as token}
        {#if typeof token === 'string'}
          {token}
        {:else}
          <kbd>{token.key}</kbd>
        {/if}
      {/each}
    </span>
  </span>
  <button type="button" aria-label={dismissLabel} on:click={onDismiss}><X size={13} strokeWidth={2.2} /></button>
</div>

<style>
  .interaction-hint {
    display: flex;
    min-width: 0;
    max-width: min(560px, calc(100vw - 48px));
    align-items: center;
    gap: 9px;
    border: 1px solid rgb(123 224 244 / 74%);
    border-radius: 10px;
    padding: 7px 8px 7px 7px;
    color: #f4fbff;
    background:
      linear-gradient(135deg, rgb(17 50 73 / 98%), rgb(24 77 103 / 98%)),
      #16364d;
    box-shadow:
      0 12px 28px rgb(16 47 68 / 26%),
      0 0 0 1px rgb(231 252 255 / 10%) inset,
      0 1px 0 rgb(255 255 255 / 18%) inset;
    font-size: 11px;
    line-height: 1.35;
    transition: opacity 180ms ease-out, transform 180ms ease-out;
  }

  .interaction-hint--fading {
    opacity: 0;
    transform: translateY(-5px);
    pointer-events: none;
  }

  .interaction-hint__icon {
    display: inline-grid;
    width: 28px;
    height: 28px;
    flex: 0 0 auto;
    place-items: center;
    border: 1px solid rgb(220 251 255 / 44%);
    border-radius: 7px;
    color: #11374e;
    background: linear-gradient(135deg, #bff5ff, #77d7ec);
    box-shadow: 0 1px 0 rgb(255 255 255 / 58%) inset;
  }

  .interaction-hint__copy {
    display: grid;
    min-width: 0;
    gap: 1px;
  }

  .interaction-hint__label {
    color: #8ee8f6;
    font: 700 9px/1.1 ui-monospace, SFMono-Regular, Menlo, monospace;
    letter-spacing: .12em;
    text-transform: uppercase;
  }

  .interaction-hint__message { color: #f4fbff; }

  .interaction-hint kbd {
    display: inline-flex;
    min-width: 16px;
    height: 17px;
    align-items: center;
    justify-content: center;
    margin: 0 1px;
    border: 1px solid rgb(194 243 252 / 52%);
    border-radius: 4px;
    padding: 0 4px;
    color: #fff;
    background: rgb(5 30 47 / 48%);
    box-shadow: 0 1px 0 rgb(255 255 255 / 14%) inset;
    font: 10px ui-monospace, SFMono-Regular, Menlo, monospace;
  }

  .interaction-hint button {
    display: inline-flex;
    width: 22px;
    height: 22px;
    flex: 0 0 auto;
    align-items: center;
    justify-content: center;
    border: 0;
    border-radius: 5px;
    color: #b3dce5;
    background: transparent;
    cursor: pointer;
  }

  .interaction-hint button:hover { color: #fff; background: rgb(225 251 255 / 14%); }
  .interaction-hint button:focus-visible { outline: 2px solid #a8effa; outline-offset: 1px; }
</style>
