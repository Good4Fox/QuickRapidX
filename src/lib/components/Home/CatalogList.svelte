<script lang="ts">
	/*
	 * Записи каталога.
	 *
	 * Прежде это была сетка плиток 102×128 с обрезанным до десяти знаков
	 * названием и тремя безымянными кнопками-значками внизу. Понять, что за
	 * программа и что делает каждая кнопка, было нельзя, а описание не
	 * показывалось вовсе.
	 *
	 * Теперь строка: плитка значка, название, описание, источник загрузки и
	 * действия подписью. Та же форма, что у списков найденного и витрины, —
	 * раздел перестаёт выглядеть тремя разными приложениями.
	 */
	import { invoke } from '@tauri-apps/api/core';
	import { openUrl } from '@tauri-apps/plugin-opener';

	import AppIcon from '$lib/components/ui/AppIcon.svelte';
	import type { AppEntry } from '$lib/data/appTypes';
	import { basket } from '$lib/state/basket.svelte';
	import { i18n } from '$lib/state/i18n.svelte';
	import { notifyError, notifySuccess } from '$lib/state/notifications.svelte';

	type Profile = { target: string; preset: string; exe: string };
	type Launch = { ok: boolean; message: string };

	type Props = {
		entries: AppEntry[];
		onedit: (entry: AppEntry) => void;
	};

	let { entries, onedit }: Props = $props();

	/*
	 * Настройки изоляции спрашиваются один раз на весь список: они лежат в одном
	 * файле, и вызов на каждую строку читал бы его же по разу.
	 */
	let profiles = $state<Record<string, Profile>>({});
	let running = $state('');

	$effect(() => {
		basket.restore();

		invoke<Profile[]>('container_profiles')
			.then((list) => {
				profiles = Object.fromEntries(list.map((profile) => [profile.target, profile]));
			})
			.catch(() => undefined);
	});

	/** Изоляция настроена, если выбрана роль, отличная от «без изоляции». */
	function isolated(entry: AppEntry): boolean {
		const profile = profiles[entry.id];

		return Boolean(profile && profile.preset && profile.preset !== 'off');
	}

	async function run(entry: AppEntry) {
		if (running) return;
		running = entry.id;

		try {
			// Среда заводится при первом запуске: без неё не появится папка данных
			await invoke('container_ensure');

			const result = await invoke<Launch>('container_run', {
				target: entry.id,
				exe: profiles[entry.id]?.exe || null
			});

			if (result.ok) notifySuccess(i18n.t.iso_started, entry.programm_name);
			else notifyError(entry.programm_name, result.message);
		} catch (e) {
			notifyError(entry.programm_name, String(e));
		}

		running = '';
	}

	/*
	 * В старых записях в `download_plus` лежала целая командная строка вида
	 * `winget install -e --id Publisher.Name`. Достаём из неё идентификатор,
	 * а не выполняем: строка из данных не должна уходить в запуск.
	 */
	function wingetId(value: string): string {
		if (!value) return '';

		const match = value.match(/--id\s+(\S+)/i);
		if (match) return match[1];

		// Новые записи хранят сам идентификатор — у него есть точка и нет пробелов
		return value.includes(' ') || !value.includes('.') ? '' : value.trim();
	}

	/** Чем эту запись вообще можно скачать. */
	function source(entry: AppEntry): string {
		const id = wingetId(entry.download_plus);
		if (id) return id;

		return entry.download?.startsWith('https://') ? new URL(entry.download).hostname : '';
	}
</script>

{#if entries.length === 0}
	<p class="qrx_rec_task_note">{i18n.t.cat_none}</p>
{/if}

<div class="qrx_pkg_list">
	{#each entries as entry (entry.id)}
		<article class="qrx_pkg">
			<div class="qrx_pkg_top">
				<AppIcon path={entry.programm_ico} fallback={entry.programm_name} size={56} />

				<div class="qrx_found_main">
					<span class="qrx_found_name">{entry.programm_name}</span>
					<span class="qrx_rec_task_note">{entry.programm_description}</span>
				</div>

				{#if source(entry)}
					<span class="qrx_found_tag qrx_found_tag_installed">{source(entry)}</span>
				{:else}
					<span class="qrx_rec_task_note">{i18n.t.cat_no_source}</span>
				{/if}

				{#if entry.site}
					<button
						class="qrx_rec_log"
						data-tooltip={entry.site}
						data-tooltip-position="top"
						onclick={() => openUrl(entry.site)}
					>
						{i18n.t.cat_open_site}
					</button>
				{/if}

				{#if isolated(entry)}
					<!-- Появляется только у настроенных: у остальных запускать нечего -->
					<button
						class="qrx_rec_log"
						disabled={running !== ''}
						data-tooltip={i18n.t.iso_run}
						data-tooltip-position="top"
						onclick={() => run(entry)}
					>
						{running === entry.id ? i18n.t.iso_starting : i18n.t.iso_short}
					</button>
				{/if}

				<button class="qrx_rec_log" onclick={() => onedit(entry)}>{i18n.t.cat_edit}</button>

				<!-- Плюсик кладёт в корзину: скачать пакетно, ничего не ставя -->
				<button
					class="qrx_plus"
					class:qrx_plus_on={basket.has(entry.id)}
					disabled={!source(entry)}
					aria-label={basket.has(entry.id) ? i18n.t.bag_added : i18n.t.bag_add}
					data-tooltip={basket.has(entry.id) ? i18n.t.bag_added : i18n.t.bag_add}
					data-tooltip-position="top"
					onclick={() =>
						basket.toggle({
							key: entry.id,
							name: entry.programm_name,
							packageId: wingetId(entry.download_plus),
							url: entry.download
						})}
				>
					{basket.has(entry.id) ? '✓' : '+'}
				</button>
			</div>
		</article>
	{/each}
</div>
