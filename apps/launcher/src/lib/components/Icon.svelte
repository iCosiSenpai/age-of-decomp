<script lang="ts">
  // Line icons (24 × 24, stroke = currentColor), drawn for this launcher.
  let { name, size = 20 }: { name: string; size?: number } = $props();
  const paths: Record<string, string> = {
    play: 'M8 5.5v13l10.5-6.5z',
    setup: 'M4 6h10M4 12h16M4 18h7M17 4v4M14 16v4M20 10v4',
    display: 'M3 5h18v11H3zM8 20h8M12 16v4',
    picture: 'M4 5h16v14H4zM4 15l5-5 4 4 3-3 4 4M15.5 9a1.2 1.2 0 1 0 0-.01',
    game: 'M12 3l2.4 5.3 5.6.6-4.2 3.8 1.2 5.6L12 15.4 7 18.3l1.2-5.6L4 8.9l5.6-.6z',
    audio: 'M4 9v6h4l5 4V5L8 9zM16.5 8.5a5 5 0 0 1 0 7M19 6a8.5 8.5 0 0 1 0 12',
    controls: 'M6 9h12a4 4 0 0 1 3.9 4.8l-.8 3.6a2.2 2.2 0 0 1-3.8 1L15 16H9l-2.3 2.4a2.2 2.2 0 0 1-3.8-1l-.8-3.6A4 4 0 0 1 6 9zM8 11v4M6 13h4M16 12.5h.01M18 14.5h.01',
    pads: 'M7 4h10v16H7zM10 8h4M12 13a1.6 1.6 0 1 0 0 .01M9 17h6',
    performance: 'M4 18a8 8 0 1 1 16 0M12 18l4.5-6',
    diagnostics: 'M3 12h4l2-6 4 12 2-6h6',
    system: 'M12 8.5a3.5 3.5 0 1 0 0 7 3.5 3.5 0 0 0 0-7zM19.4 13a7.6 7.6 0 0 0 0-2l2-1.5-2-3.4-2.3.9a7.6 7.6 0 0 0-1.7-1L15 3.6h-4l-.4 2.4a7.6 7.6 0 0 0-1.7 1l-2.3-.9-2 3.4 2 1.5a7.6 7.6 0 0 0 0 2l-2 1.5 2 3.4 2.3-.9c.5.4 1.1.7 1.7 1l.4 2.4h4l.4-2.4c.6-.3 1.2-.6 1.7-1l2.3.9 2-3.4z',
    check: 'M5 12.5l4.5 4.5L19 7.5',
    cross: 'M6 6l12 12M18 6L6 18',
    warn: 'M12 4l9 16H3zM12 10v4M12 17h.01',
    folder: 'M3 6h6l2 2h10v11H3z',
    file: 'M6 3h8l4 4v14H6zM14 3v4h4',
    refresh: 'M20 12a8 8 0 1 1-2.3-5.7M20 4v5h-5',
    min: 'M5 12h14',
    max: 'M5 5h14v14H5z',
    close: 'M6 6l12 12M18 6L6 18',
    arrow: 'M5 12h14M13 6l6 6-6 6',
    back: 'M19 12H5M11 6l-6 6 6 6',
    bolt: 'M13 3L5 13h6l-1 8 8-10h-6z',
    info: 'M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18zM12 11v6M12 7.5h.01',
    disk: 'M4 6h16v12H4zM4 13h16M8 16h.01',
    chip: 'M7 7h10v10H7zM9 3v4M15 3v4M9 17v4M15 17v4M3 9h4M3 15h4M17 9h4M17 15h4',
    vibrate: 'M8 4h8v16H8zM4 8v8M20 8v8',
    globe: 'M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18zM3 12h18M12 3c3 3.2 3 14.8 0 18M12 3c-3 3.2-3 14.8 0 18',
  };
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill={name === 'play' ? 'currentColor' : 'none'}
  stroke="currentColor"
  stroke-width="1.7"
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
>
  <path d={paths[name] ?? paths.info} />
</svg>
