<script lang="ts">
	/*
	 * Карточка приложения из стартового набора.
	 * Разметка и классы — из исходного приложения.
	 */
	import { openUrl } from '@tauri-apps/plugin-opener';

	import type { AppEntry } from '$lib/data/appTypes';

	type Props = { items: AppEntry[] };

	let { items }: Props = $props();

	function go(url: string) {
		if (url) openUrl(url);
	}
</script>

{#each items as item (item.id)}
	<div class="Home_App_panel_start">
		<div class="Home_App_panel_start_top">
			<div class="Home_App_panel_start_top_left"></div>
			<div class="Home_App_panel_start_top_center">
				{#if item.programm_ico}
					<div
						style="background-image: url({item.programm_ico});"
						class="HomeAppPanelStartIco"
					></div>
				{:else}
					<div class="HomeAppPanelStartIco"></div>
				{/if}
			</div>
			<div class="Home_App_panel_start_top_right"></div>
		</div>

		<div class="Home_App_panel_start_text_A">
			<div class="Home_App_panel_start_text_1">{item.programm_name.slice(0, 10)}</div>
		</div>

		<div class="Home_App_panel_start_text_bottom">
			<div class="Home_App_panel_start_install_panel">
				<button class="HomeAppPanelStartSiteIco" aria-label="Сайт" onclick={() => go(item.site)}
				></button>
				<div class="HomeAppPanelStartInstallPanelDownloadIco"></div>
				<button
					class="HomeAppPanelStartInstallPanelDownloadPlusIco"
					aria-label="Скачать"
					onclick={() => go(item.download)}
				></button>
			</div>
		</div>
	</div>
{/each}
