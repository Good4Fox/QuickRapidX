<script lang="ts">
	/*
	 * Найденное на машине.
	 *
	 * Осмотр знает четыре источника, и каждый находит своё: реестр удаления —
	 * поставленное установщиком, пакеты Store, средства разработки из PATH и
	 * портативные программы, за которыми нет записи об установке.
	 *
	 * Экран для того, чтобы увидеть весь состав машины разом. Отбор в набор для
	 * восстановления придёт сюда же следующим шагом — поэтому список сразу
	 * умеет фильтр и поиск, а не показывает всё подряд одной кучей.
	 */

	import { invoke } from '@tauri-apps/api/core';

	import AppIcon from '$lib/components/ui/AppIcon.svelte';

	import { i18n } from '$lib/state/i18n.svelte';
	import { inventory, type Kind } from '$lib/state/inventory.svelte';
	import { formatBytes } from '$lib/state/systemInfo';

	let query = $state('');
	let filter = $state<Kind | 'all'>('all');

	$effect(() => {
		inventory.ensure();
	});

	const KINDS: { id: Kind | 'all'; label: () => string }[] = [
		{ id: 'all', label: () => i18n.t.found_all },
		{ id: 'installed', label: () => i18n.t.found_installed },
		{ id: 'portable', label: () => i18n.t.found_portable },
		{ id: 'tool', label: () => i18n.t.found_tool },
		{ id: 'store', label: () => i18n.t.found_store }
	];

	const shown = $derived.by(() => {
		const needle = query.trim().toLowerCase();

		return inventory.findings.filter((finding) => {
			if (filter !== 'all' && finding.kind !== filter) return false;
			if (!needle) return true;

			return (
				finding.name.toLowerCase().includes(needle) ||
				finding.publisher.toLowerCase().includes(needle)
			);
		});
	});

	function count(kind: Kind | 'all'): number {
		return kind === 'all' ? inventory.findings.length : inventory.count(kind);
	}
</script>

<div class="qrx_found">
	<header class="qrx_found_head">
		<div class="qrx_set_row_text">
			<span class="qrx_set_row_title">{i18n.t.found_title}</span>
			<span class="qrx_set_row_note">{i18n.t.found_note}</span>
		</div>

		<button class="qrx_sys_link" disabled={inventory.scanning} onclick={() => inventory.scan()}>
			{inventory.scanning ? i18n.t.found_scanning : i18n.t.found_rescan}
		</button>
	</header>

	<div class="qrx_found_tools">
		<input
			class="qrx_found_search"
			type="text"
			placeholder={i18n.t.found_search}
			bind:value={query}
			spellcheck="false"
		/>

		<div class="qrx_found_kinds">
			{#each KINDS as kind (kind.id)}
				<button
					class="qrx_found_chip"
					class:qrx_found_chip_active={filter === kind.id}
					onclick={() => (filter = kind.id)}
				>
					{kind.label()}
					<span class="qrx_nav_count">{count(kind.id)}</span>
				</button>
			{/each}
		</div>
	</div>

	{#if inventory.scanning && inventory.findings.length === 0}
		<p class="qrx_rec_task_note">{i18n.t.found_scanning}</p>
	{:else if shown.length === 0}
		<p class="qrx_rec_task_note">{i18n.t.found_none}</p>
	{/if}

	<div class="qrx_found_list">
		{#each shown as finding (finding.key)}
			<article class="qrx_found_row">
				<AppIcon path={finding.exe || finding.location} fallback={finding.name} size={56} />

				<div class="qrx_found_main">
					<span class="qrx_found_name">{finding.name}</span>
					<span class="qrx_rec_task_note">
						{finding.publisher || finding.location || finding.exe}
					</span>
				</div>

				<div class="qrx_found_meta">
					{#if finding.version}
						<span class="qrx_found_version">{finding.version}</span>
					{/if}
					{#if finding.size > 0}
						<span class="qrx_rec_task_note">{formatBytes(finding.size)}</span>
					{/if}
				</div>

				<span class="qrx_found_tag qrx_found_tag_{finding.kind}">
					{KINDS.find((kind) => kind.id === finding.kind)?.label() ?? finding.kind}
				</span>

				<!--
					Две кнопки вместо одной подписи «Открыть».

					Она открывала папку, а читалась как «открыть программу» — и
					это разные действия. Значки различают их с одного взгляда:
					треугольник запускает, папка показывает, где лежит.
				-->
				<div class="qrx_found_acts">
					{#if finding.launch}
						<button
							class="qrx_found_act"
							aria-label={i18n.t.found_run}
							data-tooltip="{i18n.t.found_run}: {finding.launch}"
							data-tooltip-position="left"
							onclick={() => invoke('shell_open', { path: finding.launch })}
						>
							<span class="qrx_sys_ico_play"></span>
						</button>
					{/if}

					{#if finding.location}
						<button
							class="qrx_found_act"
							aria-label={i18n.t.found_folder}
							data-tooltip="{i18n.t.found_folder}: {finding.location}"
							data-tooltip-position="left"
							onclick={() => invoke('shell_open', { path: finding.location })}
						>
							<span class="qrx_sys_ico_explorer"></span>
						</button>
					{/if}
				</div>
			</article>
		{/each}
	</div>
</div>
