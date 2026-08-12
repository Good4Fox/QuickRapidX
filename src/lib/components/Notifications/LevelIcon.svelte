<script lang="ts">
	/*
	 * Значок уровня уведомления.
	 *
	 * Рисуем контуром, а не берём файлом: цвет должен наследоваться от уровня,
	 * а четыре значка в двух темах — это восемь файлов ради четырёх фигур.
	 */
	import type { Level } from '$lib/state/notifications.svelte';

	type Props = { level: Level; size?: number };

	let { level, size = 18 }: Props = $props();
</script>

<svg
	width={size}
	height={size}
	viewBox="0 0 24 24"
	fill="none"
	stroke="currentColor"
	stroke-width="1.7"
	stroke-linecap="round"
	stroke-linejoin="round"
	aria-hidden="true"
>
	{#if level === 'success'}
		<circle cx="12" cy="12" r="9" />
		<path d="m8.5 12 2.5 2.5 4.5-5" />
	{:else if level === 'warning'}
		<path d="M12 3.8 2.8 19.2a1 1 0 0 0 .87 1.5h16.66a1 1 0 0 0 .87-1.5z" />
		<path d="M12 9.5v4.5" />
		<path d="M12 17.3h.01" />
	{:else if level === 'error'}
		<circle cx="12" cy="12" r="9" />
		<path d="M15 9l-6 6M9 9l6 6" />
	{:else}
		<circle cx="12" cy="12" r="9" />
		<path d="M12 11v5.5" />
		<path d="M12 7.6h.01" />
	{/if}
</svg>
