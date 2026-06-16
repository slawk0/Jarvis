<script lang="ts">
  import { LL } from '$lib/i18n/i18n-svelte';
  import worldMap from '@svg-maps/world';
  import { trafficFill, trafficIntensity, type CountryTraffic } from '$lib/geo/countryUtils';

  let {
    countries = [],
    totalRequests = 0,
    hoveredCode = $bindable(null as string | null)
  }: {
    countries?: CountryTraffic[];
    totalRequests?: number;
    hoveredCode?: string | null;
  } = $props();

  const countByCode = $derived(
    Object.fromEntries(
      countries.map(c => [c.code?.toLowerCase(), c.count || 0])
    )
  );

  const maxCount = $derived(Math.max(...countries.map(c => c.count || 0), 1));

  function fillForId(id: string): string {
    const count = countByCode[id] || 0;
    return trafficFill(trafficIntensity(count, maxCount));
  }

  function handleEnter(id: string) {
    hoveredCode = id.toUpperCase();
  }

  function handleLeave() {
    hoveredCode = null;
  }
</script>

<div class="world-map-container">
  <svg viewBox={worldMap.viewBox} class="world-map-svg" role="img" aria-label={$LL.pangolinWorldMap.ariaLabel()}>
    {#each worldMap.locations as location}
      {@const isHovered = hoveredCode?.toLowerCase() === location.id}
      <path
        d={location.path}
        fill={fillForId(location.id)}
        stroke={isHovered ? 'rgba(251, 146, 60, 0.9)' : 'rgba(255, 255, 255, 0.06)'}
        stroke-width={isHovered ? 1.2 : 0.4}
        class="country-path"
        class:hovered={isHovered}
        onmouseenter={() => handleEnter(location.id)}
        onmouseleave={handleLeave}
        onfocus={() => handleEnter(location.id)}
        onblur={handleLeave}
        tabindex="0"
        role="button"
        aria-label={$LL.pangolinWorldMap.countryAriaLabel({ name: location.name })}
      />
    {/each}
  </svg>
  {#if countries.length === 0}
    <div class="map-empty">{$LL.pangolinWorldMap.empty()}</div>
  {/if}
</div>

<style>
  .world-map-container {
    position: relative;
    width: 100%;
    aspect-ratio: 1010 / 666;
    background: rgba(0, 0, 0, 0.25);
    border-radius: var(--radius-sm);
    overflow: hidden;
    outline: 1px solid rgba(255, 255, 255, 0.06);
  }

  .world-map-svg {
    width: 100%;
    height: 100%;
    display: block;
  }

  .country-path {
    cursor: pointer;
    transition: fill 0.15s ease, stroke 0.15s ease;
  }

  .country-path:hover,
  .country-path.hovered,
  .country-path:focus-visible {
    filter: brightness(1.15);
  }

  .map-empty {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-muted);
    font-size: 0.85rem;
    pointer-events: none;
  }
</style>
