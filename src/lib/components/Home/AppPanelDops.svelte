<script lang="ts">
	/*
	 * Сохранённые записи категории: плитка карточек и модальное окно поверх
	 * неё — просмотр, редактирование, удаление.
	 * Разметка и классы — из исходного приложения.
	 */
	import { invoke } from '@tauri-apps/api/core';
	import { openUrl } from '@tauri-apps/plugin-opener';

	import AppStatsDelete from '$lib/components/Home/AppStats/AppStatsDelete.svelte';
	import AppStatsInfo from '$lib/components/Home/AppStats/AppStatsInfo.svelte';
	import AppStatsInfoEditor from '$lib/components/Home/AppStats/AppStatsInfoEditor.svelte';
	import AppStatsNew from '$lib/components/Home/AppStats/AppStatsNew.svelte';
	import type { AppEntry } from '$lib/data/appTypes';
	import { basket } from '$lib/state/basket.svelte';
	import { i18n } from '$lib/state/i18n.svelte';
	import { notifyError, notifySuccess } from '$lib/state/notifications.svelte';

	type Props = {
		/** Записи выбранной категории. */
		entries: AppEntry[];
		/** Имя категории в хранилище: app_dops, app_games_dops и т.д. */
		category: string;
		/** Перечитать списки после изменения. */
		onchanged: () => void;
		/** Открыт ли экран создания новой записи — им управляет кнопка «+». */
		creating: boolean;
		oncreatingdone: () => void;
	};

	let { entries, category, onchanged, creating, oncreatingdone }: Props = $props();

	type Screen = 'info' | 'editor' | 'delete';

	/** Открытая запись: её номер в категории и рабочая копия полей. */
	let openIndex = $state<number | null>(null);
	let draft = $state<AppEntry | null>(null);
	let screen = $state<Screen>('info');

	const selected = $derived(openIndex === null ? null : entries[openIndex]);

	/*
	 * В `download_plus` у записей лежит готовая командная строка вида
	 * `winget install -e --id Publisher.Name`. Корзине нужен сам
	 * идентификатор — вытаскиваем его, а не выполняем строку целиком: строка
	 * из данных не должна уходить в запуск.
	 */
	function wingetId(command: string): string {
		const match = command?.match(/--id\s+([^\s]+)/i);
		return match ? match[1] : '';
	}

	function openInfo(index: number) {
		openIndex = index;
		// Редактор правит копию: отмена не должна менять список на экране.
		draft = { ...entries[index] };
		screen = 'info';
	}

	function close() {
		openIndex = null;
		draft = null;
	}

	async function save(entry: AppEntry) {
		if (openIndex === null) return;

		try {
			await invoke('editor_person_in_json', {
				lineneId: openIndex,
				linenenewSite: entry.site,
				linenenewDownloadPlus: entry.download_plus,
				linenenewDownload: entry.download,
				linenenewProgrammIco: entry.programm_ico,
				linenenewProgrammName: entry.programm_name,
				linenenewProgrammDescription: entry.programm_description,
				linenjname: category
			});

			onchanged();
			close();
			notifySuccess('Запись сохранена', entry.programm_name);
		} catch (e) {
			console.error('не удалось сохранить запись:', e);
			notifyError('Не удалось сохранить запись', String(e));
		}
	}

	async function remove() {
		if (openIndex === null) return;

		try {
			await invoke('remove_person_in_json', {
				linenumber: openIndex,
				structureName: category
			});

			onchanged();
			close();
			notifySuccess('Запись удалена');
		} catch (e) {
			console.error('не удалось удалить запись:', e);
			notifyError('Не удалось удалить запись', String(e));
		}
	}
</script>

{#if creating}
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<div
		class="HomeAppPanelStartInfo"
		onclick={oncreatingdone}
		onkeydown={(event) => {
			if (event.key === 'Escape') oncreatingdone();
		}}
	>
		<!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
		<div class="HomeAppPanelStartInfo_motion" onclick={(event) => event.stopPropagation()}>
			<div class="HomeAppPanelStartInfo_motion_tab">
				<div class="HomeAppPanelStartInfo_motion_tab_use"></div>
				<div class="HomeAppPanelStartInfo_motion_tab_left">
					<div class="HomeAppPanelStartInfo_motion_tab_left_text">{i18n.t.appTextUse4}</div>
				</div>
				<div class="HomeAppPanelStartInfo_motion_tab_right">
					<div class="HomeAppPanelStartInfo_motion_tab_right">
						<button
							class="HomeAppPanelStartInfoMotionTabButtonClose"
							aria-label="Закрыть"
							onclick={oncreatingdone}
						>
							<div class="HomeAppPanelStartInfoMotionTabButtonCloseIco"></div>
						</button>
					</div>
				</div>
			</div>

			<div class="HomeAppPanelStartInfo_motion_panel">
				<AppStatsNew
					{category}
					oncreated={() => {
						onchanged();
						oncreatingdone();
					}}
				/>
			</div>
		</div>
	</div>
{/if}

{#if selected && draft}
	<!-- Клик по затемнению закрывает окно, клик внутри — нет. Escape тоже закрывает. -->
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<div
		class="HomeAppPanelStartInfo"
		onclick={close}
		onkeydown={(event) => {
			if (event.key === 'Escape') close();
		}}
	>
		<!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
		<div class="HomeAppPanelStartInfo_motion" onclick={(event) => event.stopPropagation()}>
			<div class="HomeAppPanelStartInfo_motion_tab">
				<div class="HomeAppPanelStartInfo_motion_tab_use"></div>

				<div class="HomeAppPanelStartInfo_motion_tab_left">
					<div class="HomeAppPanelStartInfo_motion_tab_left_text">
						{#if screen === 'info'}
							{i18n.t.appTextUse4_1}
						{:else if screen === 'editor'}
							{i18n.t.appTextUse4_2}
							{selected.programm_name}
						{:else}
							{i18n.t.appTextUse4_3}
						{/if}
					</div>
				</div>

				<div class="HomeAppPanelStartInfo_motion_tab_right">
					<div class="HomeAppPanelStartInfo_motion_tab_right">
						{#if screen === 'info'}
							<button
								class="HomeAppPanelStartInfoMotionTabButtonClose"
								aria-label="Закрыть"
								onclick={close}
							>
								<div class="HomeAppPanelStartInfoMotionTabButtonCloseIco"></div>
							</button>
						{:else}
							<button
								class="HomeAppPanelStartInfoMotionTabButtonClose"
								aria-label="Назад"
								onclick={() => (screen = 'info')}
							>
								<div class="HomeAppPanelStartInfoMotionTabButtonBackIco"></div>
							</button>
						{/if}
					</div>
				</div>
			</div>

			<div class="HomeAppPanelStartInfo_motion_panel">
				{#if screen === 'info'}
					<AppStatsInfo
						ico={selected.programm_ico}
						name={selected.programm_name}
						description={selected.programm_description}
						download={selected.download}
						onedit={() => {
							draft = { ...selected };
							screen = 'editor';
						}}
						ondelete={() => (screen = 'delete')}
					/>
				{:else if screen === 'editor'}
					<AppStatsInfoEditor
						bind:entry={draft}
						oncancel={() => (screen = 'info')}
						onsave={save}
					/>
				{:else}
					<AppStatsDelete
						name={selected.programm_name}
						oncancel={() => (screen = 'info')}
						onconfirm={remove}
					/>
				{/if}
			</div>
		</div>
	</div>
{/if}

{#each entries as entry, i (entry.id + i)}
	<div class="Home_App_panel_start_2">
		<div class="Home_App_panel_start_top">
			<div class="Home_App_panel_start_top_left"></div>
			<div class="Home_App_panel_start_top_center">
				{#if entry.programm_ico}
					<div style="background-image: url({entry.programm_ico});" class="HomeAppPanelStartIco"></div>
				{:else}
					<div class="HomeAppPanelStartIco"></div>
				{/if}
			</div>
			<div class="Home_App_panel_start_top_right"></div>
		</div>

		<div class="Home_App_panel_start_text_A">
			<div class="Home_App_panel_start_text_1">{entry.programm_name.slice(0, 10)}</div>
		</div>

		<div class="Home_App_panel_start_text_bottom">
			<div class="Home_App_panel_start_install_panel">
				<button
					class="HomeAppPanelStartSiteIco"
					aria-label={entry.site}
					onclick={() => entry.site && openUrl(entry.site)}
				></button>
				<button
					class="HomeAppPanelStartInstallPanelDownloadIco"
					aria-label={i18n.t.appTextUse4_1}
					onclick={() => openInfo(i)}
				></button>
				<!--
					Плюсик кладёт запись в корзину загрузок — ровно то, для чего он
					и задумывался. Раньше он просто открывал ссылку в браузере.
				-->
				<button
					class="HomeAppPanelStartInstallPanelDownloadPlusIco"
					class:qrx_plus_on={basket.has(entry.id)}
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
				></button>
			</div>
		</div>
	</div>
{/each}
