<script lang="ts">
	/*
	 * Выбор установленной программы: список от ядра с поиском по названию.
	 * Разметка и классы — из исходного приложения.
	 */
	import AppShearch from '$lib/components/Home/AppStats/AppShearch.svelte';
	import { blankEntry, createEntry } from '$lib/state/appStore';
	import { inventory, type Finding } from '$lib/state/inventory.svelte';
	import { i18n } from '$lib/state/i18n.svelte';

	type Props = {
		category: string;
		oncreated: () => void;
	};

	let { category, oncreated }: Props = $props();

	let query = $state('');

	const apps = $derived(inventory.findings);
	const loading = $derived(inventory.scanning);

	$effect(() => {
		inventory.ensure();
	});

	const shown = $derived(
		query.trim()
			? apps.filter((app) => app.name.toLowerCase().includes(query.trim().toLowerCase()))
			: apps
	);

	async function add(app: Finding) {
		const entry = blankEntry();
		entry.programm_name = app.name;
		entry.programm_description = app.location;
		entry.programm_ico = app.exe;

		if (await createEntry(category, entry)) oncreated();
	}
</script>

<div class="AppStatsNewPanelCYE">
	<div class="user_home_app_center_panel_search_panel">
		<div class="user_home_app_center_panel_search_panel_get">
			<div class="user_home_app_center_panel_search_panel_get_panel">
				<div class="user_home_app_center_panel_search_panel_get_ico"></div>
				<input
					class="AppStatsNewPanelCYESearchPanelGetText"
					type="text"
					placeholder={i18n.t.appTextUse2}
					bind:value={query}
				/>
			</div>

			<div class="user_home_app_center_panel_search_panel_get_sorting">
				{i18n.t.appTextUse2_1}
				<select class="app_settings_home_right_panel_right_panel_panel_lang_use">
					<option>{i18n.t.appSoon}</option>
				</select>
			</div>

			<div class="user_home_app_center_panel_search_panel_get_info">Search</div>
		</div>

		<div class="user_home_app_center_panel_search_panel_text">
			{i18n.t.appTextUse3}{loading ? '…' : shown.length}
		</div>
	</div>

	<div class="AppStatsNewPanelCYEPanel">
		<AppShearch apps={shown} {loading} onpick={add} />
	</div>
</div>
