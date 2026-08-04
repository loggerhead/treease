<script lang="ts">
  import { onMount } from 'svelte';
  import { LoaderCircle, RefreshCw, Trash2, Upload, X } from 'lucide-svelte';
  import { toast } from 'svelte-sonner';
  import { Button } from './ui/button';
  import Tooltip from './Tooltip.svelte';
  import { authUser, observeAuthUser } from '../auth/auth-user-store';
  import { getFeedbackConsoleLogs } from '../feedback/console-log-buffer';

  import { submitFeedback } from '../services/treease-server';

  type Category = 'bug' | 'feature' | 'question';

  export let open = false;

  let category: Category = 'bug';
  let description = '';
  let email = '';
  let includeScreenshot = true;
  let sendConsoleLogs = true;
  let screenshot = '';
  let screenshotBusy = false;
  let submitBusy = false;
  let errorMessage = '';
  let prepared = false;
  let uploadInput: HTMLInputElement | null = null;
  let captureFrame: number | null = null;
  let captureGeneration = 0;
  let emailAutofilled = false;

  onMount(() => observeAuthUser());

  $: if (open && !prepared) {
    prepared = true;
    if ($authUser?.email) {
      email = $authUser.email;
      emailAutofilled = true;
    }
    scheduleScreenshot();
  }

  $: if (open && !emailAutofilled && $authUser?.email) {
    email = $authUser.email;
    emailAutofilled = true;
  }

  $: if (!open && prepared) {
    resetForm();
  }

  async function prepareScreenshot(): Promise<void> {
    const generation = ++captureGeneration;
    screenshotBusy = true;
    errorMessage = '';
    try {
      const nextScreenshot = await capturePage();
      if (open && generation === captureGeneration) screenshot = nextScreenshot;
    } catch {
      if (open && generation === captureGeneration) {
        errorMessage = 'Automatic screenshot failed. You can still upload an image and submit.';
      }
    } finally {
      if (open && generation === captureGeneration) screenshotBusy = false;
    }
  }

  function scheduleScreenshot(): void {
    if (captureFrame !== null) cancelAnimationFrame(captureFrame);
    captureFrame = requestAnimationFrame(() => {
      captureFrame = requestAnimationFrame(() => {
        captureFrame = null;
        if (open) void prepareScreenshot();
      });
    });
  }

  async function capturePage(): Promise<string> {
    const selectedTarget = document.querySelector('.app-split-layout');
    const target = selectedTarget instanceof HTMLElement ? selectedTarget : document.body;
    const { captureFeedbackScreenshot } = await import('../feedback/screenshot-capture.runtime');
    return captureFeedbackScreenshot(target, node => {
      if (!(node instanceof HTMLElement)) return true;
      return node.id !== 'bugdrop-host' && !node.closest('[data-testid="feedback-dialog"]');
    });
  }

  function resetForm(): void {
    if (captureFrame !== null) {
      cancelAnimationFrame(captureFrame);
      captureFrame = null;
    }
    captureGeneration += 1;
    prepared = false;
    category = 'bug';
    description = '';
    email = '';
    emailAutofilled = false;
    includeScreenshot = true;
    sendConsoleLogs = true;
    screenshot = '';
    screenshotBusy = false;
    submitBusy = false;
    errorMessage = '';
  }

  function removeScreenshot(): void {
    screenshot = '';
    includeScreenshot = false;
  }

  function requestScreenshotReplacement(): void {
    uploadInput?.click();
  }

  async function handleScreenshotUpload(event: Event): Promise<void> {
    const input = event.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    input.value = '';
    if (!file) return;
    if (!file.type.startsWith('image/')) {
      errorMessage = 'Please choose an image file.';
      return;
    }
    if (file.size > 5 * 1024 * 1024) {
      errorMessage = 'Images must be smaller than 5 MB.';
      return;
    }
    screenshot = await readFileAsDataUrl(file);
    includeScreenshot = true;
    errorMessage = '';
  }

  async function submit(): Promise<void> {
    if (!description.trim()) {
      errorMessage = 'Please describe your feedback.';
      return;
    }

    const normalizedEmail = email.trim();
    if (normalizedEmail && !normalizedEmail.includes('@')) {
      errorMessage = 'Please enter a valid email address.';
      return;
    }

    submitBusy = true;
    errorMessage = '';
    try {
      const issueUrl = await submitFeedback({ category, description: description.trim(), email: normalizedEmail || null, screenshot: includeScreenshot ? screenshot : '', consoleLogs: sendConsoleLogs ? JSON.stringify(getFeedbackConsoleLogs(), null, 2) : '' });
      toast.success('Feedback submitted', {
        description: 'Your feedback has been sent to GitHub.',
        ...(issueUrl
          ? {
              action: {
                label: 'View Issue',
                onClick: () => window.open(issueUrl, '_blank', 'noopener,noreferrer'),
              },
            }
          : {}),
      });
      open = false;
    } catch (error) {
      errorMessage = error instanceof Error ? error.message : 'Unable to submit feedback.';
    } finally {
      submitBusy = false;
    }
  }

  function readFileAsDataUrl(file: File): Promise<string> {
    return new Promise((resolve, reject) => {
      const reader = new FileReader();
      reader.onload = () => (typeof reader.result === 'string' ? resolve(reader.result) : reject(new Error('Unable to read the image.')));
      reader.onerror = () => reject(new Error('Unable to read the image.'));
      reader.readAsDataURL(file);
    });
  }


  $: descriptionPlaceholder = {
    bug: 'What went wrong? Include the steps to reproduce, what you expected, and what actually happened.',
    feature: 'What would you like Treease to do? Describe the workflow, use case, and why it would be useful.',
    question: 'What are you trying to accomplish? Include the context and where you got stuck.',
  }[category];
</script>

<section aria-labelledby="feedback-dialog-title" data-testid="feedback-dialog" class="feedback-panel flex max-h-[calc(100vh-16px)] flex-col gap-4 overflow-y-auto">
    <header>
      <h2 id="feedback-dialog-title" class="text-[16px] leading-none font-semibold tracking-[-0.01em]">Send Feedback</h2>
    </header>

    <div class="flex flex-col gap-4">
        <fieldset class="flex flex-col gap-2 pb-1">
          <legend class="text-[12px] font-semibold">Category</legend>
          <div class="grid grid-cols-3 gap-2">
            {#each [['bug', '🐛 Bug'], ['feature', '✨ Feature'], ['question', '❓ Question']] as option}
              <label class="flex min-h-7 cursor-pointer items-center gap-2 rounded-[var(--control-radius)] border border-[var(--border-muted)] px-2 py-1.5 text-[13px] has-[:checked]:border-[var(--accent)] has-[:checked]:bg-[var(--accent-soft)]">
                <input type="radio" name="feedback-category" value={option[0]} bind:group={category} />
                {option[1]}
              </label>
            {/each}
          </div>
        </fieldset>

        <label class="flex flex-col gap-2 text-[12px] font-semibold">
          Description
          <textarea bind:value={description} class="min-h-24 resize-y rounded-[var(--control-radius)] border border-[var(--border-muted)] bg-[var(--panel-bg)] px-3 py-2 font-normal outline-none focus:border-[var(--accent)] focus:ring-2 focus:ring-[var(--accent)]/15" placeholder={descriptionPlaceholder}></textarea>
        </label>

        <label class="flex flex-col gap-2 text-[12px] font-semibold" for="feedback-email">
          <span>Email <span class="font-normal text-[var(--text-muted)]">(optional, for follow-up)</span></span>
          <input id="feedback-email" bind:value={email} type="email" autocomplete="email" placeholder="you@example.com" class="h-8 rounded-[var(--control-radius)] border border-[var(--border-muted)] bg-[var(--panel-bg)] px-3 font-normal outline-none focus:border-[var(--accent)] focus:ring-2 focus:ring-[var(--accent)]/15" on:input={() => (emailAutofilled = true)} />
        </label>

        <div class="rounded-[var(--surface-radius)] border border-[var(--border-muted)] bg-[var(--panel-bg-alt)] p-3">
          <div class="mb-2 flex items-center justify-between gap-3">
            <label class="flex items-center gap-2 text-[12px] font-semibold">
              <input type="checkbox" bind:checked={includeScreenshot} />
              Include screenshot
            </label>
            <input bind:this={uploadInput} class="hidden" type="file" accept="image/*" on:change={handleScreenshotUpload} />
            <Button size="sm" on:click={requestScreenshotReplacement}>
              <Upload size={14} class="mr-1.5" />{screenshot ? 'Replace image' : 'Upload image'}
            </Button>
          </div>
          {#if screenshotBusy}
            <div class="flex h-36 items-center justify-center rounded-[9px] border border-dashed border-[var(--border-muted)] text-[12px] text-[var(--text-muted)]">
              <LoaderCircle size={16} class="mr-2 animate-spin" />Generating screenshot…
            </div>
          {:else if screenshot}
            <div class="relative overflow-hidden rounded-[9px] border border-[var(--border-muted)] bg-white">
              <img src={screenshot} alt="Screenshot preview" class="max-h-56 w-full object-contain" />
              <div class="absolute right-2 top-2 flex gap-1.5">
                <Tooltip content="Retake screenshot" side="top"><Button size="xs" iconOnly={true} aria-label="Retake screenshot" on:click={scheduleScreenshot}><RefreshCw size={13} /></Button></Tooltip>
                <Tooltip content="Delete screenshot" side="top"><Button size="xs" iconOnly={true} aria-label="Delete screenshot" on:click={removeScreenshot}><Trash2 size={13} /></Button></Tooltip>
              </div>
            </div>
          {:else}
            <div class="flex h-36 items-center justify-center rounded-[9px] border border-dashed border-[var(--border-muted)] text-[12px] text-[var(--text-muted)]">
              <button type="button" class="flex h-full w-full flex-col items-center justify-center gap-2 text-[12px] text-[var(--text-muted)] transition-colors hover:text-[var(--accent)]" on:click={requestScreenshotReplacement} aria-label="Upload screenshot">
                <Upload size={18} />
                <span>Click to upload a screenshot</span>
              </button>
            </div>
          {/if}
        </div>

        <label class="flex items-center gap-2 text-[12px] text-[var(--text-muted)]">
          <input type="checkbox" bind:checked={sendConsoleLogs} />
          Send console logs
        </label>

        {#if errorMessage}
          <p class="rounded-[8px] bg-red-50 px-3 py-2 text-[12px] text-red-700">{errorMessage}</p>
        {/if}
    </div>
    <footer class="flex flex-col-reverse gap-2 sm:flex-row sm:justify-end">
      <Button size="sm" variant="outline" on:click={() => (open = false)}><X size={14} class="mr-1.5" />Cancel</Button>
      <Button size="sm" disabled={submitBusy || screenshotBusy} on:click={submit}>
        {#if submitBusy}<LoaderCircle size={14} class="mr-1.5 animate-spin" />{/if}
        Submit feedback
      </Button>
    </footer>
</section>
