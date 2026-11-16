<script lang="ts">
  let {
    checked,
    onchange,
    disabled = false,
    label,
  }: { checked: boolean; onchange: (v: boolean) => void; disabled?: boolean; label: string } = $props();
</script>

<button
  type="button"
  role="switch"
  aria-checked={checked}
  aria-label={label}
  class="toggle"
  class:on={checked}
  {disabled}
  onclick={() => onchange(!checked)}
>
  <span class="knob"></span>
</button>

<style>
  .toggle {
    position: relative;
    width: 52px;
    height: 28px;
    border-radius: 14px;
    border: 1px solid var(--line-strong);
    background: var(--surface-1);
    cursor: pointer;
    transition: background 140ms var(--ease);
    flex: none;
  }
  .toggle.on {
    background: var(--focus);
    border-color: var(--focus);
  }
  .toggle:disabled {
    opacity: 0.45;
  }
  .knob {
    position: absolute;
    top: 3px;
    left: 3px;
    width: 20px;
    height: 20px;
    border-radius: 50%;
    background: var(--text);
    transition: transform 160ms var(--ease);
  }
  .on .knob {
    transform: translateX(24px);
    background: #06202c;
  }
</style>
