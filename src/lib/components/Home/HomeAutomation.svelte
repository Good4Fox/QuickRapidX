<script lang="ts">
	/*
	 * Раздел автоматизации. Разметка и классы — из исходного приложения.
	 * Список берётся у ядра: оно смотрит папки automation/* в каталоге данных.
	 */
	import { invoke } from '@tauri-apps/api/core';

	import { i18n } from '$lib/state/i18n.svelte';

	let files = $state<string[]>([]);

	$effect(() => {
		invoke<string[]>('check_automation_folders')
			.then((found) => {
				files = found ?? [];
			})
			.catch((e) => {
				// Команда появится вместе с системным слоем — до тех пор список пуст
				console.warn('check_automation_folders недоступна:', e);
			});
	});
</script>

<div class="Home_automation">
	<div class="Home_automation_left"></div>

	<div class="Home_automation_center">
		<div class="Home_automation_center_panel">
			<div class="Home_automation_center_panel_text_1">{i18n.t.automation_text_1}</div>
			<div class="Home_automation_center_panel_text_2">{i18n.t.automation_text_2}</div>
			<div class="Home_automation_center_panel_text_3">
				{#if files.length > 0}
					{#each files as name (name)}
						{name}
					{/each}
				{:else}
					<br />
					<br />
					{i18n.t.automation_text_3}
				{/if}
			</div>
		</div>
	</div>

	<div class="Home_automation_right">
		<div class="Home_automation_right_panel"></div>
	</div>
</div>
