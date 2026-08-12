<script lang="ts">
	/* Вкладки консоли. Разметка и классы — из исходного приложения. */
	import type { Tab } from '$lib/components/Console/types';

	type Props = {
		tabs: Tab[];
		activeId: number;
		onadd: () => void;
		onremove: (id: number) => void;
		onselect: (id: number) => void;
	};

	let { tabs, activeId, onadd, onremove, onselect }: Props = $props();

	/** Колесо мыши прокручивает ряд вкладок вбок, а не страницу вниз. */
	function onwheel(event: WheelEvent) {
		event.preventDefault();
		(event.currentTarget as HTMLElement).scrollLeft += event.deltaY;
	}
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="app_console_tab" {onwheel}>
	{#each tabs as tab (tab.id)}
		<button
			class="app_console_tab_cmd {tab.id === activeId ? 'active' : ''}"
			onclick={() => onselect(tab.id)}
		>
			{tab.name}
			<span
				class="close"
				role="button"
				tabindex="0"
				onclick={(event) => {
					event.stopPropagation();
					onremove(tab.id);
				}}
				onkeydown={(event) => {
					if (event.key === 'Enter' || event.key === ' ') {
						event.stopPropagation();
						onremove(tab.id);
					}
				}}
			>
				X
			</span>
		</button>
	{/each}

	<button class="app_console_tab_add" onclick={onadd}>+</button>
</div>
