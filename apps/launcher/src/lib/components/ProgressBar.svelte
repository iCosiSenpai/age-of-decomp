<script lang="ts">
  let { value, tone = 'focus', label }: { value: number; tone?: 'focus' | 'ok' | 'err' | 'gold'; label: string } = $props();
  let pct = $derived(Math.max(0, Math.min(100, value * 100)));
</script>

<div class="bar" role="progressbar" aria-label={label} aria-valuemin="0" aria-valuemax="100" aria-valuenow={Math.round(pct)}>
  <div class="fill {tone}" style:width="{pct}%"></div>
</div>

<style>
  .bar {
    position: relative;
    height: 12px;
    border-radius: 6px;
    background: var(--surface-1);
    border: 1px solid var(--line);
    overflow: hidden;
  }
  .fill {
    height: 100%;
    border-radius: 6px;
    transition: width 300ms var(--ease);
    position: relative;
    overflow: hidden;
  }
  .fill::after {
    content: '';
    position: absolute;
    inset: 0;
    background: linear-gradient(90deg, transparent, rgba(255, 255, 255, 0.28), transparent);
    animation: sheen 1.8s linear infinite;
  }
  .focus {
    background: linear-gradient(90deg, #1d8fc0, var(--focus));
  }
  .ok {
    background: var(--ok);
  }
  .ok::after,
  .err::after {
    display: none;
  }
  .err {
    background: var(--err);
  }
  .gold {
    background: linear-gradient(90deg, var(--gold), var(--gold-strong));
  }
  @keyframes sheen {
    from {
      transform: translateX(-100%);
    }
    to {
      transform: translateX(100%);
    }
  }
</style>
