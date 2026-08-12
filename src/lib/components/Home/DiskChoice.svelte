<script lang="ts">
	/*
	 * Выбор диска под Место.
	 *
	 * Выбирают диск, а не папку: имя папки задаём мы сами и одинаковое для всех.
	 * Так Место узнаётся по одному пути и на другой машине, а человеку не надо
	 * решать, куда именно внутри диска класть.
	 *
	 * Отдельная папка тоже доступна — для тех, у кого диск уже занят под свою
	 * раскладку, — но это второй вариант, а не первый.
	 */
	import { invoke } from '@tauri-apps/api/core';
	import { open } from '@tauri-apps/plugin-dialog';

	import { i18n } from '$lib/state/i18n.svelte';
	import { formatBytes } from '$lib/state/systemInfo';

	type Disk = {
		letter: string;
		label: string;
		total: number;
		free: number;
		removable: boolean;
		system: boolean;
		path: string;
	};

	type Props = {
		/** Человек выбрал путь. Создание — на стороне вызывающего. */
		onpick: (path: string) => void;
		disabled?: boolean;
	};

	let { onpick, disabled = false }: Props = $props();

	let disks = $state<Disk[]>([]);
	let suggested = $state('');

	$effect(() => {
		invoke<Disk[]>('library_disks')
			.then((list) => (disks = list))
			.catch((e) => console.error('не удалось получить список дисков:', e));

		invoke<string>('library_suggest')
			.then((path) => (suggested = path))
			.catch(() => undefined);
	});

	async function chooseFolder() {
		const picked = await open({
			directory: true,
			multiple: false,
			title: i18n.t.lib_other,
			defaultPath: suggested || undefined
		});

		if (typeof picked === 'string') onpick(picked);
	}

	/** Доля занятого — по ней рисуется полоса заполнения. */
	function used(disk: Disk): number {
		return disk.total > 0 ? ((disk.total - disk.free) / disk.total) * 100 : 0;
	}
</script>

<div class="qrx_disks">
	{#each disks as disk (disk.letter)}
		<button
			class="qrx_disk"
			class:qrx_disk_suggested={disk.path === suggested}
			{disabled}
			onclick={() => onpick(disk.path)}
		>
			<span class="qrx_disk_top">
				<span class="qrx_disk_letter">{disk.letter}</span>

				{#if disk.system}
					<span class="qrx_disk_tag">{i18n.t.lib_disk_system}</span>
				{:else if disk.removable}
					<span class="qrx_disk_tag">{i18n.t.lib_disk_removable}</span>
				{/if}
			</span>

			{#if disk.label}
				<span class="qrx_disk_label">{disk.label}</span>
			{/if}

			<span class="qrx_disk_free">
				{formatBytes(disk.free)} {i18n.t.lib_free}
			</span>

			<span class="qrx_meter">
				<span class="qrx_meter_fill" style="width: {used(disk)}%"></span>
			</span>

			<span class="qrx_disk_total">{i18n.t.lib_of} {formatBytes(disk.total)}</span>
		</button>
	{/each}
</div>

<button class="qrx_rec_log" {disabled} onclick={chooseFolder}>{i18n.t.lib_other}</button>
