<script lang="ts">
  import { Github, LoaderCircle, Mail, ShieldCheck } from 'lucide-svelte';
  import { toast } from 'svelte-sonner';
  import { Dialog, DialogContent, DialogHeader, DialogTitle } from './ui/dialog';
  import { Button } from './ui/button';
  import { sendEmailOtp, signInWithProvider, verifyEmailOtp } from '../auth/supabase-auth';
  import { workspaceHost } from '../workspace-host';

  export let open = false;

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
  <DialogContent aria-label="Log in" data-testid="login-dialog" class="max-w-[500px] rounded-[20px] border-[#dce5f0] bg-white p-8 shadow-[0_18px_60px_rgba(15,23,42,0.16)] sm:p-10">
    <DialogHeader class="items-center">
      <DialogTitle class="text-center text-[38px] font-bold tracking-[-0.04em] text-[#071126]">Log in</DialogTitle>
    </DialogHeader>

    <div class="mt-4 flex flex-col gap-4">
      <Button
        variant="outline"
        class="h-12 w-full justify-start rounded-[11px] border-[#dce5f0] px-5 text-[18px] font-semibold shadow-[0_2px_4px_rgba(15,23,42,0.05)] hover:bg-[#f8fbff]"
        data-testid="login-google-button"
        disabled={busy}
        on:click={() => handleProvider('google')}
      >
        <span class="mr-5 grid h-6 w-6 place-items-center text-[22px] font-bold" aria-hidden="true"><span class="text-[#4285f4]">G</span></span>
        Login with Google
      </Button>
      <Button
        variant="outline"
        class="h-12 w-full justify-start rounded-[11px] border-[#dce5f0] px-5 text-[18px] font-semibold shadow-[0_2px_4px_rgba(15,23,42,0.05)] hover:bg-[#f8fbff]"
        data-testid="login-github-button"
        disabled={busy}
        on:click={() => handleProvider('github')}
      >
        <Github size={25} strokeWidth={2.4} class="mr-5 text-[#071126]" />
        Login with GitHub
      </Button>

      <div class="my-4 flex items-center gap-4 text-[18px] font-medium text-[#61738f]" aria-hidden="true">
        <span class="h-px flex-1 bg-[#dce5f0]"></span><span>OR</span><span class="h-px flex-1 bg-[#dce5f0]"></span>
      </div>

      {#if !otpSent}
        <label class="flex flex-col gap-2 text-[13px] font-medium text-[#61738f]" for="login-email">
          Email address
          <div class="flex h-12 items-center gap-3 rounded-[11px] border border-[#dce5f0] px-4 focus-within:border-[var(--accent)] focus-within:ring-2 focus-within:ring-[var(--accent)]/15">
            <Mail size={18} class="text-[#61738f]" />
            <input id="login-email" bind:value={email} type="email" autocomplete="email" placeholder="you@example.com" class="min-w-0 flex-1 bg-transparent text-[15px] text-[#071126] outline-none placeholder:text-[#9aa9bd]" on:keydown={(event) => event.key === 'Enter' && handleEmailSubmit()} />
          </div>
        </label>
        <Button class="h-12 w-full rounded-[11px] bg-[#2879f6] text-[16px] font-semibold text-white hover:bg-[#1768e5]" disabled={busy} data-testid="login-email-button" on:click={handleEmailSubmit}>
          {#if busy}<LoaderCircle size={16} class="mr-2 animate-spin" />{/if}
          Continue with email
        </Button>
      {:else}
        <label class="flex flex-col gap-2 text-[13px] font-medium text-[#61738f]" for="login-otp">
          Verification code sent to {email}
          <input id="login-otp" bind:value={otp} type="text" inputmode="numeric" autocomplete="one-time-code" maxlength="8" placeholder="Enter 6-digit code" class="h-12 rounded-[11px] border border-[#dce5f0] px-4 text-[18px] tracking-[0.2em] text-[#071126] outline-none focus:border-[var(--accent)] focus:ring-2 focus:ring-[var(--accent)]/15" on:keydown={(event) => event.key === 'Enter' && handleOtpSubmit()} />
        </label>
        <div class="flex gap-3">
          <Button variant="outline" class="h-11 flex-1 rounded-[11px]" on:click={() => (otpSent = false)}>Change email</Button>
          <Button class="h-11 flex-1 rounded-[11px] bg-[#2879f6] text-white hover:bg-[#1768e5]" disabled={busy} data-testid="login-verify-button" on:click={handleOtpSubmit}>
            {#if busy}<LoaderCircle size={16} class="mr-2 animate-spin" />{/if}Verify code
          </Button>
        </div>
      {/if}

      {#if error}
        <p class="rounded-[9px] border border-red-200 bg-red-50 px-3 py-2 text-[13px] text-red-700" role="alert">{error}</p>
      {/if}

      <p class="mt-5 flex items-start gap-2 text-[14px] leading-6 text-[#61738f]">
        <ShieldCheck size={17} class="mt-0.5 shrink-0 text-[#2879f6]" />
        By continuing you agree to our <a class="text-[#2879f6] hover:underline" href="https://treease.io/terms" on:click={(event) => void openLegalPage(event, '/terms')}>Terms of Service</a> and <a class="text-[#2879f6] hover:underline" href="https://treease.io/privacy" on:click={(event) => void openLegalPage(event, '/privacy')}>Privacy Policy</a>.
      </p>
    </div>
  </DialogContent>
</Dialog>
