<script lang="ts">
	/*
	 * Предложение завести Место при первом заходе в раздел.
	 *
	 * Показывается один раз: отказ запоминается, и дальше раздел работает как
	 * обычный каталог. Кнопка «Хранилище» остаётся на месте — вернуться к
	 * этому можно когда угодно.
	 */
	import { invoke } from '@tauri-apps/api/core';

	import DiskChoice from '$lib/components/Home/DiskChoice.svelte';
	import { i18n } from '$lib/state/i18n.svelte';
	import { notifyError, notifySuccess } from '$lib/state/notifications.svelte';

	type Props = {
		/** Место заведено или человек отказался — предложение уходит. */
		ondone: () => void;
	};

	let { ondone }: Props = $props();

	let busy = $state(false);

	async function create(path: string) {
		if (busy) return;
		busy = true;

		try {
			await invoke('library_add', { path, name: '', fallback: i18n.t.lib_default_name });
			notifySuccess(i18n.t.lib_created, path);
			ondone();
		} catch (e) {
			notifyError(i18n.t.notification_text_4, String(e));
		}

		busy = false;
	}
</script>

<div class="qrx_offer">
	<div class="qrx_offer_card">
		<span class="qrx_offer_title">{i18n.t.lib_offer_title}</span>
		<p class="qrx_offer_text">{i18n.t.lib_offer_note}</p>

		<span class="qrx_rec_task_note">{i18n.t.lib_pick_disk}</span>

		<DiskChoice onpick={create} disabled={busy} />

		<div class="qrx_offer_actions">
			<button class="qrx_rec_log" onclick={ondone}>{i18n.t.lib_offer_later}</button>
			<span class="qrx_rec_task_note">{i18n.t.lib_offer_hint}</span>
		</div>
	</div>
</div>
