<script lang="ts">
	/* Консоль: ряд вкладок сверху, панель активной вкладки снизу. */
	import ConsolePanel from '$lib/components/Console/ConsolePanel.svelte';
	import Tabs from '$lib/components/Console/Tabs.svelte';
	import type { Tab } from '$lib/components/Console/types';

	let tabs = $state<Tab[]>([{ id: 1, name: 'Command Line 1' }]);
	let activeId = $state(1);
	let output = $state<Record<number, string>>({ 1: '' });

	function add() {
		const id = tabs.length > 0 ? tabs[tabs.length - 1].id + 1 : 1;
		tabs.push({ id, name: `Command Line ${id}` });
		output[id] = '';
		activeId = id;
	}

	function remove(id: number) {
		tabs = tabs.filter((tab) => tab.id !== id);
		delete output[id];

		// Закрыли активную — переходим на первую оставшуюся
		if (activeId === id && tabs.length > 0) activeId = tabs[0].id;
	}
</script>

<div class="app_console">
	<div class="app_console_top">
		<div class="app_console_top_panel">
			<div class="app_console_top_panel_tab">
				<Tabs
					{tabs}
					{activeId}
					onadd={add}
					onremove={remove}
					onselect={(id) => (activeId = id)}
				/>
			</div>
		</div>
	</div>

	<div class="app_console_down">
		<div class="app_console_down_panel">
			{#if tabs.length > 0}
				<ConsolePanel {activeId} bind:output />
			{/if}
		</div>
	</div>
</div>
