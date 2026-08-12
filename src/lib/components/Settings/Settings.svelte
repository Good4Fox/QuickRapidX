<script lang="ts">
	/*
	 * Экран настроек: узкое левое меню и область раздела справа.
	 *
	 * Меню догнало главный экран: направляющая с бегунком, золотая иконка у
	 * выбранного раздела, подложка при наведении. Раньше не было ни одного из
	 * трёх — активный раздел ничем не отмечался, — зато под тремя рабочими
	 * пунктами стояли шесть пустых плиток на 358px высоты.
	 */
	import SettingsA from '$lib/components/Settings/SettingsA.svelte';
	import SettingsInfo from '$lib/components/Settings/SettingsInfo.svelte';
	import SettingsWindows from '$lib/components/Settings/SettingsWindows.svelte';
	import ViewSwitch from '$lib/components/ViewSwitch.svelte';
	import { APP_TAG, APP_TAG_TOOLTIP } from '$lib/data/info';
	import { i18n } from '$lib/state/i18n.svelte';

	type View = 'settings' | 'windows' | 'info';

	let view = $state<View>('settings');

	/** Разделы идут сверху вниз — по порядку считается и направление перехода. */
	const ORDER: View[] = ['settings', 'windows', 'info'];

	const SECTIONS: { id: View; ico: number; tip: () => string }[] = [
		{ id: 'settings', ico: 1, tip: () => i18n.t.settings_text_5 },
		{ id: 'windows', ico: 2, tip: () => i18n.t.settings_text_5_2 },
		{ id: 'info', ico: 3, tip: () => i18n.t.settings_text_5_3 }
	];

	let direction = $state(1);

	function go(next: View) {
		direction = ORDER.indexOf(next) >= ORDER.indexOf(view) ? 1 : -1;
		view = next;
	}

	const activeIndex = $derived(ORDER.indexOf(view));

	const versions = ` ${APP_TAG_TOOLTIP} ${APP_TAG}`;
</script>

<div class="app_settings">
	<div class="app_settings_home_left">
		<div class="app_settings_home_left_use">
			<!-- Направляющая с бегунком: едет к выбранному разделу -->
			<div class="qrx_rail" style="--rail-count: {SECTIONS.length}" aria-hidden="true">
				<div class="qrx_rail_thumb" style="--rail-index: {activeIndex}"></div>
			</div>

			{#each SECTIONS as section (section.id)}
				<button
					class="app_settings_home_left_link"
					class:app_settings_home_left_link_active={view === section.id}
					aria-label={section.tip()}
					aria-current={view === section.id ? 'page' : undefined}
					onclick={() => go(section.id)}
				>
					<div
						class="app_settings_home_left_link_back"
						data-tooltip={section.tip()}
						data-tooltip-position="right"
					>
						<div class="app_settings_home_left_link_back_ico_{section.ico}"></div>
					</div>
				</button>
			{/each}
		</div>
	</div>

	<div class="app_settings_home_right">
		<div class="app_settings_home_right_panel">
			<ViewSwitch {view} {direction}>
				{#if view === 'settings'}
					<SettingsA />
				{:else if view === 'windows'}
					<SettingsWindows />
				{:else if view === 'info'}
					<SettingsInfo {versions} />
				{/if}
			</ViewSwitch>
		</div>
	</div>
</div>
