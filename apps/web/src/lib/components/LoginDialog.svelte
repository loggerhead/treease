<script lang="ts">
  import { Github, LoaderCircle, Mail } from 'lucide-svelte';
  import { toast } from 'svelte-sonner';
  import { Dialog, DialogContent, DialogHeader, DialogTitle } from './ui/dialog';
  import { Button } from './ui/button';
  import { sendEmailOtp, signInWithProvider, verifyEmailOtp } from '../auth/supabase-auth';
  import { workspaceHost } from '../workspace-host';

  export let open = false;
  export let onAuthenticated: () => void = () => {};

  let email = '';
  let otp = '';
  let otpSent = false;
  let busy = false;
  let error = '';

  $: if (!open) {
    email = '';
    otp = '';
    otpSent = false;
    busy = false;
    error = '';
  }

  async function handleProvider(provider: 'google' | 'github') {
    busy = true;
    error = '';
    try {
      await signInWithProvider(provider);
    } catch (cause) {
      error = cause instanceof Error ? cause.message : 'Unable to start login.';
      busy = false;
    }
  }

  async function handleEmailSubmit() {
    const normalizedEmail = email.trim();
    if (!normalizedEmail || !normalizedEmail.includes('@')) {
      error = 'Enter a valid email address.';
      return;
    }
    busy = true;
    error = '';
    try {
      await sendEmailOtp(normalizedEmail);
      otpSent = true;
      toast.success('A verification code was sent to your email.');
    } catch (cause) {
      error = cause instanceof Error ? cause.message : 'Unable to send the verification code.';
    } finally {
      busy = false;
    }
  }

  async function handleOtpSubmit() {
    const normalizedEmail = email.trim();
    const normalizedOtp = otp.trim();
    if (normalizedOtp.length < 6) {
      error = 'Enter the verification code from your email.';
      return;
    }
    busy = true;
    error = '';
    try {
      const session = await verifyEmailOtp(normalizedEmail, normalizedOtp);
      if (session?.refresh_token && (await workspaceHost).surface === 'desktop') {
        await (await workspaceHost).storeRefreshToken(session.refresh_token);
      }
      open = false;
      onAuthenticated();
      toast.success('You are now logged in.');
    } catch (cause) {
      error = cause instanceof Error ? cause.message : 'The verification code is invalid.';
    } finally {
      busy = false;
    }
  }

  async function openLegalPage(event: MouseEvent, path: '/terms' | '/privacy'): Promise<void> {
    event.preventDefault();
    if ((await workspaceHost).surface !== 'desktop') {
      window.location.assign(path);
      return;
    }
    await (await workspaceHost).openExternal(new URL(path, 'https://treease.io'));
  }
</script>

<Dialog bind:open>
  <DialogContent aria-label="Log in" data-testid="login-dialog" style="max-width: 400px" class="gap-0 rounded-[10px] border-[#e2e8f0] bg-white p-6 shadow-[0_18px_54px_rgba(15,23,42,0.14)]">
    <DialogHeader class="items-center">
      <DialogTitle class="text-center text-[22px] font-semibold tracking-[-0.025em] text-[#0f172a]">Log in</DialogTitle>
    </DialogHeader>

    <div class="mt-6 flex flex-col gap-2">
      <Button
        variant="outline"
        class="h-10 w-full justify-start gap-4 rounded-[6px] border-[#e2e8f0] px-4 text-[14px] font-medium text-[#0f172a] shadow-none hover:bg-[#f8fafc]"
        data-testid="login-google-button"
        disabled={busy}
        on:click={() => handleProvider('google')}
      >
        <svg class="h-5 w-5" viewBox="0 0 24 24" aria-hidden="true">
          <path fill="#4285F4" d="M21.6 12.23c0-.71-.06-1.4-.18-2.07H12v3.92h5.38a4.6 4.6 0 0 1-2 3.02v2.55h3.24c1.9-1.75 2.98-4.33 2.98-7.42Z" />
          <path fill="#34A853" d="M12 22c2.7 0 4.97-.9 6.62-2.35l-3.24-2.55c-.9.6-2.05.96-3.38.96-2.6 0-4.81-1.76-5.6-4.13H3.05v2.63A10 10 0 0 0 12 22Z" />
          <path fill="#FBBC05" d="M6.4 13.93A6 6 0 0 1 6.08 12c0-.67.12-1.32.32-1.93V7.44H3.05A10 10 0 0 0 2 12c0 1.64.39 3.19 1.05 4.56l3.35-2.63Z" />
          <path fill="#EA4335" d="M12 5.94c1.47 0 2.79.51 3.83 1.5l2.87-2.88A9.65 9.65 0 0 0 12 2a10 10 0 0 0-8.95 5.44l3.35 2.63C7.19 7.7 9.4 5.94 12 5.94Z" />
        </svg>
        Login with Google
      </Button>
      <Button
        variant="outline"
        class="h-10 w-full justify-start gap-4 rounded-[6px] border-[#e2e8f0] px-4 text-[14px] font-medium text-[#0f172a] shadow-none hover:bg-[#f8fafc]"
        data-testid="login-github-button"
        disabled={busy}
        on:click={() => handleProvider('github')}
      >
        <Github size={20} strokeWidth={2.2} class="text-[#0f172a]" />
        Login with GitHub
      </Button>

      <div class="my-3 flex items-center gap-2 text-[11px] font-medium text-[#94a3b8]" aria-hidden="true">
        <span class="h-px flex-1 bg-[#e2e8f0]"></span><span>OR</span><span class="h-px flex-1 bg-[#e2e8f0]"></span>
      </div>

      {#if !otpSent}
        <label class="flex flex-col gap-2 text-[13px] font-medium text-[#61738f]" for="login-email">
          Email address
          <div class="flex h-10 items-center gap-3 rounded-[6px] border border-[#e2e8f0] px-3 focus-within:border-[var(--accent)] focus-within:ring-2 focus-within:ring-[var(--accent)]/15">
            <Mail size={18} class="text-[#61738f]" />
            <input id="login-email" bind:value={email} type="email" autocomplete="email" placeholder="you@example.com" class="min-w-0 flex-1 bg-transparent text-[15px] text-[#071126] outline-none placeholder:text-[#9aa9bd]" on:keydown={(event) => event.key === 'Enter' && handleEmailSubmit()} />
          </div>
        </label>
        <Button class="h-10 w-full rounded-[6px] text-[14px] font-medium" style="background-color: #2563eb; color: #ffffff" disabled={busy} data-testid="login-email-button" on:click={handleEmailSubmit}>
          {#if busy}<LoaderCircle size={16} class="mr-2 animate-spin" />{/if}
          Continue with email
        </Button>
      {:else}
        <label class="flex flex-col gap-2 text-[13px] font-medium text-[#61738f]" for="login-otp">
          Verification code sent to {email}
          <input id="login-otp" bind:value={otp} type="text" inputmode="numeric" autocomplete="one-time-code" maxlength="8" placeholder="Enter 6-digit code" class="h-12 rounded-[11px] border border-[#dce5f0] px-4 text-[17px] font-medium tabular-nums tracking-[0.12em] text-[#071126] outline-none placeholder:text-[15px] placeholder:font-normal placeholder:tracking-normal placeholder:text-[#9aa9bd] focus:border-[var(--accent)] focus:ring-2 focus:ring-[var(--accent)]/15" on:keydown={(event) => event.key === 'Enter' && handleOtpSubmit()} />
        </label>
        <div class="flex gap-3">
          <Button variant="outline" class="h-11 flex-1 rounded-[11px]" on:click={() => (otpSent = false)}>Change email</Button>
          <Button class="h-11 flex-1 rounded-[11px]" style="background-color: #2563eb; color: #ffffff" disabled={busy} data-testid="login-verify-button" on:click={handleOtpSubmit}>
            {#if busy}<LoaderCircle size={16} class="mr-2 animate-spin" />{/if}Verify code
          </Button>
        </div>
      {/if}

      {#if error}
        <p class="rounded-[9px] border border-red-200 bg-red-50 px-3 py-2 text-[13px] text-red-700" role="alert">{error}</p>
      {/if}

      <p class="mt-4 text-[12px] leading-5 text-[#64748b]">
        By continuing you are agreeing to our <a class="text-[#2563eb] hover:underline" href="https://treease.io/terms" on:click={(event) => void openLegalPage(event, '/terms')}>Terms of Service</a> and <a class="text-[#2563eb] hover:underline" href="https://treease.io/privacy" on:click={(event) => void openLegalPage(event, '/privacy')}>Privacy Policy</a>.
      </p>
    </div>
  </DialogContent>
</Dialog>
