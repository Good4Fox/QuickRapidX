<script lang="ts">
	/*
	 * Новая запись: выбор способа — заполнить руками или взять установленную
	 * программу. Разметка и классы — из исходного приложения.
	 */
	import CreateYourApp from '$lib/components/Home/AppStats/CreateYourApp.svelte';
	import CreateYourEntry from '$lib/components/Home/AppStats/CreateYourEntry.svelte';
	import { i18n } from '$lib/state/i18n.svelte';

	type Props = {
		category: string;
		/** Запись создана — перечитать списки и закрыть окно. */
		oncreated: () => void;
	};

	let { category, oncreated }: Props = $props();

	type Screen = 'choice' | 'entry' | 'search';

	let screen = $state<Screen>('choice');
</script>

<div class="AppStatsNew">
	<div class="AppStatsNewPanel">
		{#if screen === 'choice'}
			<button class="AppStatsNewPanelHomeStart" onclick={() => (screen = 'entry')}>
				<div class="AppStatsNewPanelHomeStartICO"></div>
				<div class="AppStatsNewPanelHomeStartText">{i18n.t.appTextUse5}</div>
			</button>

			<button class="AppStatsNewPanelHomeStart" onclick={() => (screen = 'search')}>
				<div class="AppStatsNewPanelHomeStartICO"></div>
				<div class="AppStatsNewPanelHomeStartText">{i18n.t.appTextUse5_1}</div>
			</button>
		{:else if screen === 'entry'}
			<CreateYourEntry {category} oncancel={() => (screen = 'choice')} {oncreated} />
		{:else}
			<CreateYourApp {category} {oncreated} />
		{/if}
	</div>
</div>
