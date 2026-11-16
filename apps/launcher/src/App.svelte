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

  let ready = $state(false);
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
      if (app.launcher?.game.status === 'complete' && !app.game.running) api.play().catch((e) => app.toast(String(e), 'err'));
      else app.page = app.launcher?.game.status === 'complete' ? 'home' : 'setup';
    },
  };

  onMount(async () => {
    await app.init();
    ready = true;
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
    <div class="body">
      <NavRail />
      <main bind:this={main} aria-label={tr('Content')}>
        {#key app.page}
          <div class="view">
            {#if app.page === 'home'}
              <Home />
            {:else if app.page === 'setup'}
              <Setup />
            {:else if app.page === 'pads'}
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
    <div class="loading"><span class="spinner"></span></div>
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
