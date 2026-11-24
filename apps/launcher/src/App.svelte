<script lang="ts">
  import { onMount } from 'svelte';
  import { api, isTauri } from './lib/api';
  import { tr } from './lib/i18n.svelte';
  import { installKeyboardNav, installWebGamepadNav, moveFocus, Repeater, type NavButton, type NavHandlers } from './lib/nav';
  import { app, NAV } from './lib/state.svelte';
  import TitleBar from './lib/components/TitleBar.svelte';
  import NavRail from './lib/components/NavRail.svelte';
  import HintBar from './lib/components/HintBar.svelte';
  import Toasts from './lib/components/Toasts.svelte';
  import Home from './lib/pages/Home.svelte';
  import Setup from './lib/pages/Setup.svelte';
  import SettingsPage from './lib/pages/SettingsPage.svelte';
  import Pads from './lib/pages/Pads.svelte';
  import System from './lib/pages/System.svelte';
  import Updates from './lib/components/Updates.svelte';
  import Icon from './lib/components/Icon.svelte';

  let ready = $state(false);
  // The first read of the launcher's state failed: shown with a way to try again, instead of loading for ever.
  let initError = $state<string | null>(null);
  let sdlNav = $state(false);
  let main: HTMLElement | undefined = $state();

  const handlers: NavHandlers = {
    back: () => {
      const active = document.activeElement as HTMLElement | null;
      if (main?.contains(active)) {
        document.querySelector<HTMLElement>('.rail .item.active')?.focus();
      } else if (app.page !== 'home') {
        app.page = 'home';
      }
    },
    tab: (delta) => {
      const pages = NAV.filter((n) => !n.advanced || app.showAdvanced);
      const i = pages.findIndex((p) => p.id === app.page);
      app.page = pages[(i + delta + pages.length) % pages.length].id;
      requestAnimationFrame(() => moveFocus('down'));
    },
    start: () => {
      // Start plays when Play on the home page would; otherwise it goes where the next step is.
      const game = app.launcher?.game;
      if (game?.status === 'complete' && game.nup_present && !app.game.running) api.play().catch((e) => app.toast(String(e), 'err'));
      else app.page = game?.status === 'complete' ? 'home' : 'setup';
    },
  };

  onMount(async () => {
    try {
      await app.init();
      ready = true;
    } catch (e) {
      initError = String(e);
      requestAnimationFrame(() => moveFocus('down'));
    }
    installKeyboardNav(handlers);
    installWebGamepadNav(handlers, () => {}, () => !sdlNav);
    if (isTauri) {
      // The backend reads every controller through SDL 3 (Switch 2 pads included) and sends logical buttons.
      const { listen } = await import('@tauri-apps/api/event');
      const rep = new Repeater(handlers);
      await listen<{ button: NavButton; down: boolean }>('pad-nav', (e) => {
        sdlNav = true;
        rep.set(e.payload.button, e.payload.down);
      });
      const loop = () => {
        rep.tick();
        requestAnimationFrame(loop);
      };
      requestAnimationFrame(loop);
    }
  });

  $effect(() => {
    app.page;
    main?.scrollTo({ top: 0 });
  });
</script>

<div class="shell">
  <TitleBar version={app.launcher?.version ?? ''} />
  {#if ready}
    <Updates startup />
    {#if app.launcher?.diagnostics_available && app.launcher.diagnostics === null}
      <aside class="consent" aria-label={tr('Anonymous diagnostics')}>
        <div>
          <strong>{tr('Help improve Age of Decomp')}</strong>
          <p>{tr('Share crash categories, build failures and rounded frame rates with the developer. Personal files and full logs stay on this PC. You can turn this off in System.')}</p>
        </div>
        <button class="btn" onclick={() => app.setDiagnostics(false)}>{tr('Keep disabled')}</button>
        <button class="btn" onclick={() => app.setDiagnostics(true)}>{tr('Share diagnostics')}</button>
      </aside>
    {/if}
    <div class="body">
      <NavRail />
      <main bind:this={main} aria-label={tr('Content')}>
        {#key app.page}
          <div class="view">
            {#if app.page === 'home'}
              <Home />
            {:else if app.page === 'setup'}
              <Setup />
            {:else if app.page === 'controls' || app.page === 'pads'}
              <Pads />
            {:else if app.page === 'system'}
              <System />
            {:else}
              <SettingsPage group={app.page} />
            {/if}
          </div>
        {/key}
      </main>
    </div>
    <HintBar />
  {:else}
    <div class="loading">
      {#if initError !== null}
        <div class="failed">
          <p class="error" role="alert"><Icon name="warn" size={20} /> {tr('Age of Decomp could not start.')} <span class="faint">{tr(initError)}</span></p>
          <button class="btn primary" data-autofocus onclick={() => location.reload()}>{tr('Try again')}</button>
        </div>
      {:else}
        <span class="spinner"></span>
      {/if}
    </div>
  {/if}
  <Toasts />
</div>

<style>
  .shell {
    height: 100%;
    display: flex;
    flex-direction: column;
    background:
      radial-gradient(ellipse at 100% 0%, rgba(63, 198, 255, 0.06), transparent 40%),
      radial-gradient(ellipse at 0% 100%, rgba(231, 183, 90, 0.05), transparent 45%),
      var(--bg);
  }
  .body {
    flex: 1;
    display: flex;
    min-height: 0;
  }
  .consent {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px 24px;
    border-bottom: 1px solid var(--line);
    background: var(--surface-2);
  }
  .consent div { flex: 1; }
  .consent p { margin: 4px 0 0; color: var(--text-faint); }
  main {
    flex: 1;
    overflow-y: auto;
    padding: 26px 32px 40px;
  }
  .view {
    animation: enter 220ms var(--ease);
  }
  @keyframes enter {
    from {
      opacity: 0;
      transform: translateY(6px);
    }
    to {
      opacity: 1;
      transform: none;
    }
  }
  .loading {
    flex: 1;
    display: grid;
    place-items: center;
  }
  .failed {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12px;
  }
  .error {
    color: var(--err);
    max-width: 680px;
    overflow-wrap: anywhere;
    text-align: center;
  }
  .spinner {
    width: 34px;
    height: 34px;
    border-radius: 50%;
    border: 3px solid var(--surface-4);
    border-top-color: var(--focus);
    animation: spin 0.8s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
