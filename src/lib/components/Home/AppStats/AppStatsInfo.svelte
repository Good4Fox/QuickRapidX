<script lang="ts">
	/* Карточка записи: иконка, название, описание и действия. */
	import { openUrl } from '@tauri-apps/plugin-opener';

	import { i18n } from '$lib/state/i18n.svelte';

	type Props = {
		ico: string;
		name: string;
		description: string;
		download: string;
		onedit: () => void;
		ondelete: () => void;
	};

	let { ico, name, description, download, onedit, ondelete }: Props = $props();

	// Длинные значения режутся так же, как в исходном приложении.
	const shortName = $derived(name.length >= 48 ? `${name.slice(0, 48)} ...` : name);

	const shortDescription = $derived(
		description.length >= 1668 ? `${description.slice(0, 1670)} ...` : description
	);
</script>

<div class="AppStatsInfo">
	<div class="AppStatsInfoPanelA">
		{#if ico}
			<div style="background-image: url({ico});" class="AppStatsInfoPanelAICO"></div>
		{:else}
			<div class="AppStatsInfoPanelAICO"></div>
		{/if}
		<div class="AppStatsInfoPanelAText">
			<div class="AppStatsInfoPanelAText1">{shortName}</div>
		</div>
	</div>

	<div class="AppStatsInfoStats">
		<div class="AppStatsInfoStatsPanel">
			<div class="AppStatsInfoStatsPanelInformation">
				<button
					class="AppStatsInfoStatsPanelInformationDownloadPlus"
					aria-label={i18n.t.app_text_13}
					onclick={() => download && openUrl(download)}
				></button>
				<button
					class="AppStatsInfoStatsPanelInformationDownload"
					aria-label={i18n.t.app_text_13}
					onclick={() => download && openUrl(download)}
				></button>
				<div class="AppStatsInfoStatsPanelInformationText">{i18n.t.app_text_13}</div>
				<button
					class="AppStatsInfoStatsPanelInformationEditor"
					aria-label={i18n.t.appTextUse4_2}
					onclick={onedit}
				></button>
				<button
					class="AppStatsInfoStatsPanelInformationRemove"
					aria-label={i18n.t.appTextUse4_3}
					onclick={ondelete}
				></button>
			</div>
		</div>

		<div class="AppStatsInfoStatsPanelText">
			<div class="AppStatsInfoStatsPanelText1">{shortDescription}</div>
		</div>
	</div>
</div>
