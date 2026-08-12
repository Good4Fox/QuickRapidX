<script lang="ts">
	/*
	 * Редактор записи: шесть полей, между которыми переключают либо список
	 * слева, либо стрелки под полем. Разметка и классы — из исходного приложения.
	 */
	import FileDrop from '$lib/components/FileDrop.svelte';
	import type { AppEntry } from '$lib/data/appTypes';
	import { i18n } from '$lib/state/i18n.svelte';

	type Props = {
		/** Редактируемая запись. Меняется на месте. */
		entry: AppEntry;
		oncancel: () => void;
		onsave: (entry: AppEntry) => void;
	};

	let { entry = $bindable(), oncancel, onsave }: Props = $props();

	/** Поля редактора в том же порядке, что и в списке слева. */
	const FIELDS = [
		{ key: 'name', ico: 3, label: () => i18n.t.appTextUse6 },
		{ key: 'description', ico: 3, label: () => i18n.t.appTextUse6_1 },
		{ key: 'icon', ico: 2, label: () => i18n.t.appTextUse6_1_1 },
		{ key: 'site', ico: 2, label: () => i18n.t.appTextUse6_1_2 },
		{ key: 'msi', ico: 2, label: () => i18n.t.appTextUse6_1_3 },
		{ key: 'winget', ico: 1, label: () => i18n.t.appTextUse6_1_4 }
	] as const;

	let index = $state(0);

	const current = $derived(FIELDS[index]);

	/** Стрелки ходят по кругу, как и раньше. */
	function step(direction: -1 | 1) {
		index = (index + direction + FIELDS.length) % FIELDS.length;
	}
</script>

<div class="AppStatsEditor">
	<div class="AppStatsEditorPanel1">
		{#each FIELDS as field, i (field.key)}
			<button class="AppStatsEditorPanel1Element" onclick={() => (index = i)}>
				<div class="AppStatsEditorPanel1ElementICO{field.ico}"></div>
				<div class="AppStatsEditorPanel1ElementText">{field.label()}</div>
			</button>
		{/each}
	</div>

	<div class="AppStatsEditorPanel2">
		<div class="AppStatsEditorPanel2Panel">
			<div class="AppStatsEditorPanel2PanelUse">
				<div class="AppStatsEditorPanel2PanelUseText">{current.label()}</div>

				<div class="AppStatsEditorPanel2PanelUseElement">
					{#if current.key === 'name'}
						<input
							class="AppStatsEditorPanel2PanelUseElementEdit"
							type="text"
							placeholder="Text"
							bind:value={entry.programm_name}
						/>
					{:else if current.key === 'description'}
						<textarea
							class="AppStatsEditorPanel2PanelUseElementEditA"
							placeholder="Text ..."
							bind:value={entry.programm_description}
						></textarea>
					{:else if current.key === 'icon'}
						<FileDrop />
						<input
							class="AppStatsEditorPanel2PanelUseElementEdit"
							type="text"
							placeholder="https://example.com/icon.png"
							bind:value={entry.programm_ico}
						/>
					{:else if current.key === 'site'}
						<input
							class="AppStatsEditorPanel2PanelUseElementEdit"
							type="text"
							placeholder="https://example.com"
							bind:value={entry.site}
						/>
					{:else if current.key === 'msi'}
						<input
							class="AppStatsEditorPanel2PanelUseElementEdit"
							type="text"
							placeholder="https://example.com/setup.msi"
							bind:value={entry.download}
						/>
					{:else}
						<input
							class="AppStatsEditorPanel2PanelUseElementEdit"
							type="text"
							placeholder="winget install -e --id Name.Name"
							bind:value={entry.download_plus}
						/>
					{/if}
				</div>

				<div class="AppStatsEditorPanel2PanelUseGet">
					<button
						class="AppStatsEditorPanel2PanelUseGetICO1"
						aria-label="Предыдущее поле"
						onclick={() => step(-1)}
					></button>
					<button
						class="AppStatsEditorPanel2PanelUseGetICO2"
						aria-label="Следующее поле"
						onclick={() => step(1)}
					></button>
				</div>
			</div>
		</div>
	</div>

	<div class="AppStatsEditorPanel3">
		<button class="AppStatsEditorPanel3Button1" onclick={oncancel}>
			{i18n.t.appTextUse6_2}
		</button>
		<button class="AppStatsEditorPanel3Button2" onclick={() => onsave(entry)}>
			{i18n.t.appTextUse6_2_1}
		</button>
	</div>
</div>
