<script lang="ts">
	/*
	 * Корзина загрузок — то самое «Скачать».
	 *
	 * Итоговое место: человек отмечает нужное плюсиком в каталоге и витрине, а
	 * здесь забирает всё разом. Ставить ничего не надо — установщики просто
	 * складываются в «Загрузки/QuickRapidX».
	 *
	 * Позиции качаются по очереди, а не разом: параллельные загрузки делят канал
	 * и мешают друг другу, а отчёт нужен построчно.
	 */
	import { invoke } from '@tauri-apps/api/core';

	import AppIcon from '$lib/components/ui/AppIcon.svelte';
	import { basket } from '$lib/state/basket.svelte';
	import { i18n } from '$lib/state/i18n.svelte';
	import { notifySuccess } from '$lib/state/notifications.svelte';

	let folder = $state('');

	$effect(() => {
		basket.restore();

		invoke<string>('downloads_dir')
			.then((path) => (folder = path))
			.catch(() => undefined);
	});

	async function run() {
		const done = await basket.download();
		const failed = done.filter((outcome) => !outcome.ok).length;

		notifySuccess(
			i18n.t.bag_done,
			failed ? `${failed} ${i18n.t.bag_failed}` : String(done.length)
		);
	}
</script>

<div class="qrx_found">
	<header class="qrx_found_head">
		<div class="qrx_set_row_text">
			<span class="qrx_set_row_title">{i18n.t.bag_title}</span>
			<span class="qrx_set_row_note">{i18n.t.bag_note}</span>
		</div>

		{#if folder}
			<button class="qrx_rec_log" onclick={() => invoke('shell_open', { path: folder })}>{i18n.t.lib_downloads}</button>
		{/if}
	</header>

	{#if basket.items.length === 0}
		<p class="qrx_rec_task_note">{i18n.t.bag_empty}</p>
	{:else}
		<div class="qrx_found_tools">
			<button class="qrx_sys_link" disabled={basket.current !== null} onclick={run}>
				{basket.current ? i18n.t.bag_getting : `${i18n.t.bag_get} (${basket.items.length})`}
			</button>

			<button class="qrx_rec_log" disabled={basket.current !== null} onclick={() => basket.clear()}>
				{i18n.t.bag_clear}
			</button>
		</div>

		<div class="qrx_pkg_list">
			{#each basket.items as item (item.key)}
				<article class="qrx_pkg" class:qrx_pkg_open={basket.current === item.key}>
					<div class="qrx_pkg_top">
						<AppIcon path="" fallback={item.name} size={56} />

						<div class="qrx_found_main">
							<span class="qrx_found_name">{item.name}</span>
							<span class="qrx_rec_task_note">{item.packageId || item.url}</span>
						</div>

						{#if basket.current === item.key}
							<span class="qrx_found_version">{i18n.t.bag_getting}</span>
						{:else}
							<button
								class="qrx_rec_log qrx_lib_drop"
								disabled={basket.current !== null}
								onclick={() => basket.toggle(item)}
							>
								{i18n.t.lib_forget}
							</button>
						{/if}
					</div>
				</article>
			{/each}
		</div>
	{/if}

	{#if basket.report.length > 0}
		<div class="qrx_pkg_list">
			{#each basket.report as outcome (outcome.name)}
				<span class="qrx_rec_task_note">
					{outcome.ok ? '✓' : '×'} {outcome.name} {outcome.message}
				</span>
			{/each}
		</div>
	{/if}
</div>
