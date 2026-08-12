<script lang="ts">
	/*
	 * Хранилище: места, куда приложение кладёт программы.
	 *
	 * Смысл в одном: забрать программу, которую установила Windows, нельзя —
	 * её файлы лишь малая часть, остальное в реестре, службах и профиле. А
	 * унести ту, что мы сами положили, можно целиком. Поэтому Место — первое,
	 * что появляется в разделе, и всё остальное будет опираться на него.
	 *
	 * Мест может быть несколько, на разных дисках. Одно основное — туда идёт
	 * то, для чего не выбрали другое.
	 */
	import { invoke } from '@tauri-apps/api/core';

	import AppIsolation from '$lib/components/Home/AppIsolation.svelte';
	import DiskChoice from '$lib/components/Home/DiskChoice.svelte';
	import { i18n } from '$lib/state/i18n.svelte';
	import { notifyError, notifySuccess } from '$lib/state/notifications.svelte';
	import { formatBytes } from '$lib/state/systemInfo';

	type Library = { id: string; name: string; root: string };
	type LibraryState = Library & {
		available: boolean;
		apps: number;
		size: number;
		primary: boolean;
	};

	let places = $state<LibraryState[]>([]);
	let downloads = $state('');
	let busy = $state(false);

	/** Место, которое переносят: пока выбран диск, показывается его выбор. */
	let moving = $state<string | null>(null);

	$effect(() => {
		load();
	});

	async function load() {
		try {
			places = await invoke<LibraryState[]>('library_list');
		} catch (e) {
			console.error('не удалось прочитать места:', e);
		}

		try {
			downloads = await invoke<string>('downloads_dir');
		} catch (e) {
			console.error('не удалось определить папку загрузок:', e);
		}
	}

	async function add(path: string) {
		if (busy) return;
		busy = true;

		try {
			await invoke('library_add', { path, name: '', fallback: i18n.t.lib_default_name });
			notifySuccess(i18n.t.lib_created, path);
			await load();
		} catch (e) {
			notifyError(i18n.t.notification_text_4, String(e));
		}

		busy = false;
	}

	async function move(id: string, path: string) {
		moving = null;
		await act('library_move', { id, path, fallback: i18n.t.lib_default_name }, i18n.t.lib_moved);
	}

	async function act(command: string, args: Record<string, unknown>, done?: string) {
		if (busy) return;
		busy = true;

		try {
			await invoke(command, args);
			if (done) notifySuccess(done, '');
		} catch (e) {
			notifyError(i18n.t.notification_text_4, String(e));
		}

		await load();
		busy = false;
	}
</script>

<div class="qrx_lib">
	<header class="qrx_lib_head">
		<div class="qrx_set_row_text">
			<span class="qrx_set_row_title">{i18n.t.lib_title}</span>
			<span class="qrx_set_row_note">{i18n.t.lib_note}</span>
		</div>

		{#if downloads}
			<button class="qrx_rec_log" onclick={() => invoke('shell_open', { path: downloads })}>
				{i18n.t.lib_downloads}
			</button>
		{/if}
	</header>

	{#if places.length === 0}
		<p class="qrx_rec_task_note">{i18n.t.lib_empty}</p>
	{/if}

	<div class="qrx_lib_list">
		{#each places as place (place.id)}
			<article class="qrx_set_row qrx_set_row_stack" class:qrx_lib_gone={!place.available}>
				<div class="qrx_set_row_head">
					<div class="qrx_set_row_text">
						<span class="qrx_set_row_title">
							{place.name}
							{#if place.primary}
								<span class="qrx_lib_badge">{i18n.t.lib_primary}</span>
							{/if}
						</span>
						<span class="qrx_rec_task_note">{place.root}</span>
					</div>

					<div class="qrx_lib_figures">
						{#if place.available}
							<span class="qrx_lib_size">{formatBytes(place.size)}</span>
							<span class="qrx_rec_task_note">{place.apps} {i18n.t.lib_count}</span>
						{:else}
							<span class="qrx_rec_warn">{i18n.t.lib_offline}</span>
						{/if}
					</div>
				</div>

				<div class="qrx_lib_actions">
					{#if place.available}
						<button class="qrx_rec_log" onclick={() => invoke('shell_open', { path: place.root })}>
							{i18n.t.lib_open}
						</button>
					{/if}

					{#if !place.primary}
						<button
							class="qrx_rec_log"
							disabled={busy}
							onclick={() => act('library_set_primary', { id: place.id })}
						>
							{i18n.t.lib_make_primary}
						</button>
					{/if}

					<button
						class="qrx_rec_log"
						disabled={busy}
						onclick={() => (moving = moving === place.id ? null : place.id)}
					>
						{moving === place.id ? i18n.t.lib_cancel : i18n.t.lib_move}
					</button>

					<button
						class="qrx_rec_log"
						disabled={busy}
						data-tooltip={i18n.t.lib_forget_note}
						data-tooltip-position="top"
						onclick={() => act('library_forget', { id: place.id })}
					>
						{i18n.t.lib_forget}
					</button>

					<button
						class="qrx_rec_log qrx_lib_drop"
						disabled={busy}
						data-tooltip={i18n.t.lib_delete_note}
						data-tooltip-position="top"
						onclick={() => act('library_delete', { id: place.id }, i18n.t.lib_deleted)}
					>
						{i18n.t.lib_delete}
					</button>
				</div>

				{#if moving === place.id}
					<div class="qrx_lib_move">
						<span class="qrx_rec_task_note">{i18n.t.lib_move_where}</span>
						<DiskChoice onpick={(path) => move(place.id, path)} disabled={busy} />
					</div>
				{/if}
			</article>
		{/each}
	</div>

	<!-- Добавление: выбирается диск, папку внутри него заводим сами -->
	<section class="qrx_set_row qrx_set_row_stack">
		<div class="qrx_set_row_text">
			<span class="qrx_set_row_title">{i18n.t.lib_add}</span>
		</div>

		<DiskChoice onpick={add} disabled={busy} />
	</section>

	<AppIsolation />
</div>
