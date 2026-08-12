<script lang="ts">
	/*
	 * Плитка найденных в системе программ.
	 * Разметка и классы — из исходного приложения.
	 */
	import type { Finding } from '$lib/state/inventory.svelte';

	type Props = {
		apps: Finding[];
		/** Идёт поиск — показываем анимацию вместо плитки. */
		loading: boolean;
		onpick: (app: Finding) => void;
	};

	let { apps, loading, onpick }: Props = $props();

	/**
	 * Режет длинное имя на части по заглавным буквам, цифрам и точкам —
	 * длинные названия иначе не переносятся и вылезают из карточки.
	 * Перенос ставится каждые 16 частей, как и раньше.
	 */
	function parts(name: string): string[] {
		return name.split(/(?=[A-Z])|(?=\d)|\./g);
	}
</script>

{#if loading}
	<div class="AppStatsNewPanelCYEPanelAppLoading">
		<div class="AppStatsNewPanelCYEPanelAppContainer">
			<div class="AppStatsNewPanelCYEPanelAppLoader">
				<div class="AppStatsNewPanelCYEPanelCrystal"></div>
				<div class="AppStatsNewPanelCYEPanelCrystal"></div>
				<div class="AppStatsNewPanelCYEPanelCrystal"></div>
				<div class="AppStatsNewPanelCYEPanelCrystal"></div>
				<div class="AppStatsNewPanelCYEPanelCrystal"></div>
				<div class="AppStatsNewPanelCYEPanelCrystal"></div>
			</div>
		</div>
	</div>
{:else}
	{#each apps as app, i (app.name + app.location + i)}
		<button class="AppStatsNewPanelCYEPanelApp" onclick={() => onpick(app)}>
			<div class="AppStatsNewPanelCYEPanelAppTop">
				<div class="AppStatsNewPanelCYEPanelAppTopLeft"></div>
				<div class="AppStatsNewPanelCYEPanelAppTopCenter">
					<div class="AppStatsNewPanelCYEPanelAppICO"></div>
				</div>
				<div class="AppStatsNewPanelCYEPanelAppTopRight"></div>
			</div>

			<div class="AppStatsNewPanelCYEPanelAppName">
				<div class="AppStatsNewPanelCYEPanelAppNameText">
					{#each parts(app.name) as part, j (j)}
						{#if j > 0 && j % 16 === 0}<br />{/if}{part}
					{/each}
				</div>
			</div>

			<div class="AppStatsNewPanelCYEPanelAppTopBottom"></div>
		</button>
	{/each}
{/if}
