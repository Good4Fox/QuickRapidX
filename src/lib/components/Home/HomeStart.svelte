<script lang="ts">
	/*
	 * Стартовая страница.
	 *
	 * Раньше здесь была одна строка «Стартовая страница». Теперь — сводка о
	 * машине и быстрые действия: то, ради чего утилиту открывают чаще всего.
	 */
	import { invoke } from '@tauri-apps/api/core';

	import { i18n } from '$lib/state/i18n.svelte';
	import { startData } from '$lib/state/startData.svelte';
	import { formatBytes, formatUptime } from '$lib/state/systemInfo';

	type Props = {
		/** Перейти в раздел левого меню. */
		ongo: (view: 'app' | 'automation') => void;
	};

	let { ongo }: Props = $props();

	/*
	 * Данные собраны заранее, пока держался экран загрузки. Здесь их только
	 * читаем: запрос в момент показа страницы был заметен — плитки успевали
	 * мигнуть прочерками.
	 */
	const info = $derived(startData.info);
	const version = $derived(startData.version);
	const binUsage = $derived(startData.binUsage);
	const entries = $derived(startData.entries);

	// На случай, если страницу открыли раньше, чем сбор закончился
	$effect(() => {
		startData.load();
	});

	const memoryPercent = $derived(
		info && info.memory_total > 0 ? (info.memory_used / info.memory_total) * 100 : 0
	);

	/** Диски без размера — сетевые и пустые приводы, их не показываем. */
	const disks = $derived(info?.disks.filter((d) => d.total > 0) ?? []);
</script>

<div class="qrx_start">
	<header class="qrx_start_head">
		<div class="qrx_start_hello">
			<span class="qrx_start_title">{i18n.t.home_app_text_1}</span>
			<span class="qrx_start_sub">
				{info?.host || '—'}{version ? ` · v${version}` : ''}
			</span>
		</div>

		{#if info && !info.admin}
			<span class="qrx_start_badge" data-tooltip={i18n.t.start_admin_tip} data-tooltip-position="bottom">
				{i18n.t.start_admin}
			</span>
		{/if}
	</header>

	<div class="qrx_start_grid">
		<!-- Система -->
		<section class="qrx_tile">
			<span class="qrx_tile_label">{i18n.t.start_system}</span>
			<span class="qrx_tile_value">
				{info ? `${info.os} ${info.os_version}` : '…'}
			</span>
			<span class="qrx_tile_note">
				{info?.kernel ? `${i18n.t.start_build} ${info.kernel}` : ''}
			</span>
		</section>

		<!-- Время работы -->
		<section class="qrx_tile">
			<span class="qrx_tile_label">{i18n.t.start_uptime}</span>
			<span class="qrx_tile_value">{info ? formatUptime(info.uptime) : '…'}</span>
			<span class="qrx_tile_note">{info ? `${info.cpu_cores} ${i18n.t.start_threads}` : ''}</span>
		</section>

		<!-- Память -->
		<section class="qrx_tile">
			<span class="qrx_tile_label">{i18n.t.start_memory}</span>
			<span class="qrx_tile_value">
				{info ? `${formatBytes(info.memory_used)} / ${formatBytes(info.memory_total)}` : '…'}
			</span>
			<div class="qrx_meter">
				<div class="qrx_meter_fill" style="width: {memoryPercent}%"></div>
			</div>
		</section>

		<!-- Процессор -->
		<section class="qrx_tile qrx_tile_wide">
			<span class="qrx_tile_label">{i18n.t.start_cpu}</span>
			<span class="qrx_tile_value qrx_tile_value_small">{info?.cpu || '…'}</span>
		</section>

		<!-- Диски -->
		{#each disks as disk (disk.mount)}
			<section class="qrx_tile">
				<span class="qrx_tile_label">{i18n.t.start_disk} {disk.mount.replace(/\\$/, '')}</span>
				<span class="qrx_tile_value">{formatBytes(disk.free)} {i18n.t.start_free}</span>
				<div class="qrx_meter">
					<div
						class="qrx_meter_fill"
						style="width: {((disk.total - disk.free) / disk.total) * 100}%"
					></div>
				</div>
				<span class="qrx_tile_note">/ {formatBytes(disk.total)}</span>
			</section>
		{/each}
	</div>

	<div class="qrx_start_actions">
		<button class="qrx_action" onclick={() => ongo('app')}>
			<span class="qrx_action_value">{entries ?? '—'}</span>
			<span class="qrx_action_label">{i18n.t.start_entries}</span>
		</button>

		<button class="qrx_action" onclick={() => invoke('tray_basket_open')}>
			<span class="qrx_action_value">
				{binUsage === null ? '—' : formatBytes(binUsage)}
			</span>
			<span class="qrx_action_label">{i18n.t.start_bin}</span>
		</button>

		<button class="qrx_action" onclick={() => invoke('windows_open_settings')}>
			<span class="qrx_action_value">Windows</span>
			<span class="qrx_action_label">{i18n.t.start_windows}</span>
		</button>
	</div>
</div>
