<script lang="ts">
	/*
	 * Специальные возможности и питание.
	 *
	 * Прежде здесь были две строки — залипание клавиш и гибернация, — и обе
	 * работали наполовину: залипание писалось в реестр числом «510» и
	 * требовало перезахода, а состояние гибернации читалось, но о том, что без
	 * прав администратора её не переключить, узнавалось только по отказу.
	 *
	 * Теперь состояние приходит из ядра целиком и перечитывается после каждой
	 * правки: значение мог поменять кто-то ещё, а показывать нажатое вместо
	 * настоящего — врать.
	 */
	import { invoke } from '@tauri-apps/api/core';

	import { i18n } from '$lib/state/i18n.svelte';
	import { notifyError } from '$lib/state/notifications.svelte';
	import { formatBytes } from '$lib/state/systemInfo';

	type Tweaks = {
		sticky_hotkey: boolean;
		filter_hotkey: boolean;
		toggle_hotkey: boolean;
		hibernate: boolean;
		fast_startup: boolean;
		hiberfil: number;
		admin: boolean;
	};

	let tweaks = $state<Tweaks>({
		sticky_hotkey: false,
		filter_hotkey: false,
		toggle_hotkey: false,
		hibernate: false,
		fast_startup: false,
		hiberfil: 0,
		admin: false
	});

	let busy = $state(false);

	$effect(() => {
		load();
	});

	async function load() {
		try {
			tweaks = await invoke<Tweaks>('tweaks_get');
		} catch (e) {
			console.error('не удалось прочитать настройки системы:', e);
		}
	}

	async function change(command: string, on: boolean) {
		if (busy) return;
		busy = true;

		try {
			await invoke(command, { on });
		} catch (e) {
			notifyError(i18n.t.notification_text_4, String(e));
		}

		await load();
		busy = false;
	}

	/** Приглашения по горячим сочетаниям — три однотипные строки. */
	const PROMPTS = $derived([
		{
			on: tweaks.sticky_hotkey,
			label: i18n.t.tw_sticky,
			note: i18n.t.tw_sticky_note,
			command: 'tweaks_set_sticky'
		},
		{
			on: tweaks.filter_hotkey,
			label: i18n.t.tw_filter,
			note: i18n.t.tw_filter_note,
			command: 'tweaks_set_filter'
		},
		{
			on: tweaks.toggle_hotkey,
			label: i18n.t.tw_toggle,
			note: i18n.t.tw_toggle_note,
			command: 'tweaks_set_toggle'
		}
	]);
</script>

<!-- Специальные возможности -->
<section class="qrx_set_row qrx_set_row_stack">
	<div class="qrx_set_row_text">
		<span class="qrx_set_row_title">{i18n.t.tw_access}</span>
		<span class="qrx_set_row_note">{i18n.t.tw_access_note}</span>
	</div>

	<div class="qrx_rec_list">
		{#each PROMPTS as prompt (prompt.command)}
			<div class="qrx_rec_task">
				<div class="qrx_rec_task_text">
					<span class="qrx_rec_task_label">{prompt.label}</span>
					<span class="qrx_rec_task_note">{prompt.note}</span>
				</div>
				<input
					class="qrx_toggle"
					type="checkbox"
					checked={prompt.on}
					disabled={busy}
					onchange={(event) => change(prompt.command, event.currentTarget.checked)}
				/>
			</div>
		{/each}
	</div>
</section>

<!-- Питание -->
<section class="qrx_set_row qrx_set_row_stack">
	<div class="qrx_set_row_head">
		<div class="qrx_set_row_text">
			<span class="qrx_set_row_title">{i18n.t.tw_power}</span>
			{#if !tweaks.admin}
				<span class="qrx_rec_warn">{i18n.t.tw_admin}</span>
			{/if}
		</div>

		{#if tweaks.hiberfil > 0}
			<div class="qrx_tw_size">
				<span class="qrx_rec_task_note">{i18n.t.tw_hiberfil}</span>
				<span class="qrx_sel_input_label qrx_tw_size_value">{formatBytes(tweaks.hiberfil)}</span>
			</div>
		{/if}
	</div>

	<div class="qrx_rec_list">
		<div class="qrx_rec_task">
			<div class="qrx_rec_task_text">
				<span class="qrx_rec_task_label">{i18n.t.tw_hibernate}</span>
				<span class="qrx_rec_task_note">{i18n.t.tw_hibernate_note}</span>
			</div>
			<input
				class="qrx_toggle"
				type="checkbox"
				checked={tweaks.hibernate}
				disabled={busy || !tweaks.admin}
				onchange={(event) => change('tweaks_set_hibernate', event.currentTarget.checked)}
			/>
		</div>

		<!-- Быстрый запуск держится на гибернации, поэтому вложен под неё -->
		<div class="qrx_rec_task qrx_tw_child" class:disabled={!tweaks.hibernate}>
			<div class="qrx_rec_task_text">
				<span class="qrx_rec_task_label">{i18n.t.tw_fast}</span>
				<span class="qrx_rec_task_note">{i18n.t.tw_fast_note}</span>
			</div>
			<input
				class="qrx_toggle"
				type="checkbox"
				checked={tweaks.fast_startup}
				disabled={busy || !tweaks.admin}
				onchange={(event) => change('tweaks_set_fast_startup', event.currentTarget.checked)}
			/>
		</div>
	</div>
</section>
