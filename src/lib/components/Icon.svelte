<script lang="ts" module>
  // The handful of icons the app needs, drawn on a 24px grid with a 1.75 stroke.
  const PATHS = {
    dashboard: 'M4 4h7v9H4zM13 4h7v5h-7zM13 11h7v9h-7zM4 15h7v5H4z',
    osc: 'M12 12h.01M8.5 8.5a5 5 0 0 0 0 7M15.5 8.5a5 5 0 0 1 0 7M5.6 5.6a9 9 0 0 0 0 12.8M18.4 5.6a9 9 0 0 1 0 12.8',
    mqtt: 'M14.5 5.5a2.5 2.5 0 1 1-5 0 2.5 2.5 0 0 1 5 0zM8 18.5a2.5 2.5 0 1 1-5 0 2.5 2.5 0 0 1 5 0zM21 18.5a2.5 2.5 0 1 1-5 0 2.5 2.5 0 0 1 5 0zM12 8v4M12 12l-5 4.5M12 12l5 4.5',
    settings:
      'M4 6h9M17 6h3M4 12h3M11 12h9M4 18h11M19 18h1M13 4v4M7 10v4M15 16v4',
    info: 'M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18zM12 11v6M12 7.5h.01',
    refresh: 'M20 11a8 8 0 0 0-14.6-3.5L4 9M4 4v5h5M4 13a8 8 0 0 0 14.6 3.5L20 15M20 20v-5h-5',
    plus: 'M12 5v14M5 12h14',
    minus: 'M5 12h14',
    trash: 'M4 7h16M10 11v6M14 11v6M6 7l1 13h10l1-13M9 7V4h6v3',
    chevron: 'm6 9 6 6 6-6',
    terminal: 'M3 5h18v14H3zM7 10l3 2.5L7 15M12.5 15H17',
    close: 'M6 6l12 12M18 6 6 18',
    check: 'm5 12.5 4.5 4.5L19 7.5',
    copy: 'M9 9h11v11H9zM5 15H4V4h11v1',
    alert: 'M12 4 2.5 20h19zM12 10v5M12 17.5h.01',
    eye: 'M2 12s3.6-7 10-7 10 7 10 7-3.6 7-10 7S2 12 2 12zM12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6z',
    external: 'M14 4h6v6M20 4l-9 9M18 14v6H4V6h6',
    folder: 'M3 6h6l2 2h10v11H3z',
    save: 'M5 4h11l3 3v13H5zM8 4v5h7V4M8 20v-6h8v6',
    download: 'M12 4v11M7 10l5 5 5-5M5 20h14',
    learn: 'M12 12h.01M8.5 8.5a5 5 0 0 0 0 7M15.5 8.5a5 5 0 0 1 0 7',
    piano: 'M3 5h18v14H3zM8 5v14M13 5v14M18 5v14',
    // Step sequencer: a piano roll of four steps at different pitches above a baseline.
    pattern: 'M3 20h18M3.5 15h3M8.5 10h3M13.5 13h3M18.5 6h2.5',
    play: 'M8 5.5v13l10-6.5z',
    clock: 'M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18zM12 7v5l3 2',
    tie: 'M4 15c3.5-5.5 12.5-5.5 16 0',
    stop: 'M7 7h10v10H7z',
    shuffle: 'M4 7h3.5c4.5 0 4.5 10 9 10H20M4 17h3.5c1.6 0 2.6-1.3 3.4-3M13.1 10c.8-1.7 1.8-3 3.4-3H20M17.5 4.5 20 7l-2.5 2.5M17.5 14.5 20 17l-2.5 2.5',
    left: 'm14.5 6-6 6 6 6',
    right: 'm9.5 6 6 6-6 6',
    eraser: 'M5 19h14M8.5 15.5 4 11l7-7 9 9-4.5 4.5zM7.5 7.5l9 9',
  } as const;

  export type IconName = keyof typeof PATHS;
</script>

<script lang="ts">
  let { name, size = 18 }: { name: IconName; size?: number } = $props();
</script>

<svg
  aria-hidden="true"
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="1.75"
  stroke-linecap="round"
  stroke-linejoin="round"
>
  <path d={PATHS[name]} />
</svg>

<style>
  svg {
    flex: none;
    display: block;
  }
</style>
