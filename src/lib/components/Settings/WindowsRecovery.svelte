<script lang="ts">
	/*
	 * Восстановление Windows.
	 *
	 * Задачи идут скрыто с правами администратора, а ход показывается здесь.
	 * Прежде на этом месте были две команды, открывавшие чёрную консоль поверх
	 * окна: ход был виден, но приложение о нём ничего не знало — ни когда
	 * задача кончилась, ни чем.
	 *
	 * Полный вывод разбирать не пытаемся: `sfc` при перенаправлении пишет в
	 * UTF-16, `DISM` и `chkdsk` — в кодировке консоли, и на разных языках
	 * системы это разные кодировки. Проценты же всегда ASCII. Поэтому здесь
	 * полоса и доля, а весь текст открывается кнопкой в журнале.
	 */
	import { invoke } from '@tauri-apps/api/core';
	import { listen } from '@tauri-apps/api/event';

	import HoldButton from '$lib/components/ui/HoldButton.svelte';
	import { i18n } from '$lib/state/i18n.svelte';
	import { notifyError, notifySuccess } from '$lib/state/notifications.svelte';
	import { formatBytes } from '$lib/state/systemInfo';

	type Progress = { id: string; percent: number | null; elapsed: number };
	type Finished = { id: string; code: number; log: string };
	/**
	 * Ход удаления прошлой установки — отдельно от прочих задач.
	 *
	 * У них доля есть в выводе самой утилиты, а тут её печатать некому: считает
	 * сам обход. Зато он знает больше — сколько объектов убрано, сколько места
	 * освобождено и сколько не поддалось. Полутора миллионам объектов доля без
	 * этих цифр мало что говорит.
	 */
	type Purge = { objects: number; bytes: number; failed: number; elapsed: number };

	/** Ключи те же, что в ядре: набор команд закреплён там. */
	const TASKS = $derived([
		{ id: 'dism_check', label: i18n.t.rec_dism_check, note: i18n.t.rec_dism_check_note },
		{ id: 'dism_scan', label: i18n.t.rec_dism_scan, note: i18n.t.rec_dism_scan_note },
		{ id: 'dism_restore', label: i18n.t.rec_dism_restore, note: i18n.t.rec_dism_restore_note },
		{ id: 'sfc', label: i18n.t.rec_sfc, note: i18n.t.rec_sfc_note },
		{ id: 'chkdsk', label: i18n.t.rec_chkdsk, note: i18n.t.rec_chkdsk_note },
		{ id: 'cleanup', label: i18n.t.rec_cleanup, note: i18n.t.rec_cleanup_note },
		{ id: 'windows_update', label: i18n.t.rec_wu, note: i18n.t.rec_wu_note },
		{ id: 'network', label: i18n.t.rec_net, note: i18n.t.rec_net_note }
	]);

	/** Идущая сейчас задача. Ядро не даёт запустить вторую. */
	let running = $state<string | null>(null);
	let percent = $state<number | null>(null);
	let elapsed = $state(0);

	/** Ключ последней завершённой задачи — у неё показываем кнопку журнала. */
	let lastDone = $state<string | null>(null);

	let oldPresent = $state(false);
	let oldPath = $state('');
	let oldSize = $state<number | null>(null);
	let purge = $state<Purge | null>(null);

	/*
	 * Доля считается по освобождённому месту, а не по числу объектов: файлы в
	 * прошлой установке разного веса, и счёт по штукам врал бы — миллион мелких
	 * в системных папках прошёл бы за минуты и показал бы почти готово.
	 *
	 * Замер объёма делается до удаления и точным не бывает: в него не попадает
	 * то, куда нас не пустили. Поэтому доля прижата к сотне — обогнать её она
	 * может, а вот пятиться на глазах не должна.
	 */
	const purged = $derived(
		purge && oldSize ? Math.min(100, Math.round((purge.bytes / oldSize) * 100)) : null
	);

	$effect(() => {
		checkWindowsOld();
	});

	$effect(() => {
		const progress = listen<Progress>('recovery:progress', (event) => {
			if (event.payload.id !== running) return;
			percent = event.payload.percent;
			elapsed = event.payload.elapsed;
		});

		const purging = listen<Purge>('recovery:purge', (event) => {
			if (running !== 'windows_old') return;
			purge = event.payload;
		});

		const finished = listen<Finished>('recovery:finished', (event) => {
			const { id, code } = event.payload;

			running = null;
			percent = null;
			elapsed = 0;
			lastDone = id;

			const label =
				id === 'windows_old'
					? i18n.t.rec_old
					: (TASKS.find((task) => task.id === id)?.label ?? id);

			if (code === 0) {
				notifySuccess(label, i18n.t.notification_text_1);
			} else if (id === 'windows_old') {
				/*
				 * Итог удаления судим по самой папке, и раз она осталась —
				 * говорим, сколько не поддалось. Голый код возврата тут не
				 * значит ничего: человеку нужно знать, много ли застряло, —
				 * десяток файлов и половина дерева требуют разного.
				 */
				notifyError(label, `${i18n.t.rec_old_stuck}: ${(purge?.failed ?? 0).toLocaleString()}`);
			} else {
				notifyError(label, `${i18n.t.notification_text_4} (${code})`);
			}

			purge = null;

			// Папка могла исчезнуть — перечитываем, а не верим коду возврата
			if (id === 'windows_old') checkWindowsOld();
		});

		return () => {
			progress.then((off) => off());
			purging.then((off) => off());
			finished.then((off) => off());
		};
	});

	async function checkWindowsOld() {
		try {
			const info = await invoke<{ present: boolean; path: string }>('recovery_windows_old');
			oldPresent = info.present;
			oldPath = info.path;

			if (!info.present) {
				oldSize = null;
				return;
			}

			// Размер считается обходом дерева и приходит с задержкой
			oldSize = await invoke<number>('recovery_windows_old_size');
		} catch (e) {
			console.error('не удалось проверить прошлую установку:', e);
		}
	}

	async function start(id: string) {
		if (running) return;

		running = id;
		percent = null;
		elapsed = 0;

		try {
			await invoke('recovery_start', { id });
		} catch (e) {
			running = null;
			notifyError(i18n.t.notification_text_4, String(e));
		}
	}

	async function removeWindowsOld() {
		if (running) return;

		running = 'windows_old';
		purge = { objects: 0, bytes: 0, failed: 0, elapsed: 0 };

		try {
			await invoke('recovery_remove_windows_old');
			notifySuccess(i18n.t.rec_old_started, i18n.t.rec_old_started_note);
		} catch (e) {
			running = null;
			purge = null;
			notifyError(i18n.t.notification_text_4, String(e));
		}
	}

	function openLog(id: string) {
		invoke('recovery_open_log', { id }).catch((e) => notifyError(i18n.t.notification_text_4, String(e)));
	}

	/** «4:12» — сколько идёт задача. */
	function clock(seconds: number): string {
		const m = Math.floor(seconds / 60);
		const s = seconds % 60;
		return `${m}:${String(s).padStart(2, '0')}`;
	}
</script>

<!-- Переустановка поверх: запускается только из «Параметров» -->
<section class="qrx_set_row qrx_set_row_stack">
	<div class="qrx_set_row_text">
		<span class="qrx_set_row_title">{i18n.t.rec_reinstall}</span>
		<span class="qrx_set_row_note">{i18n.t.rec_reinstall_note}</span>
	</div>

	<div class="qrx_sys_quick">
		<button class="qrx_sys_link" onclick={() => invoke('recovery_open_settings')}>
			{i18n.t.rec_reinstall_open}
		</button>
	</div>
</section>

<!-- Проверки и починка -->
<section class="qrx_set_row qrx_set_row_stack">
	<div class="qrx_set_row_text">
		<span class="qrx_set_row_title">{i18n.t.rec_tools}</span>
		<span class="qrx_set_row_note">{i18n.t.rec_tools_note}</span>
	</div>

	<p class="qrx_rec_order">{i18n.t.rec_order}</p>

	<div class="qrx_rec_list">
		{#each TASKS as task (task.id)}
			<div class="qrx_rec_task" class:qrx_rec_task_running={running === task.id}>
				<div class="qrx_rec_task_text">
					<span class="qrx_rec_task_label">{task.label}</span>
					<span class="qrx_rec_task_note">{task.note}</span>
				</div>

				{#if running === task.id}
					<div class="qrx_rec_state">
						<span class="qrx_rec_percent">
							{percent === null ? clock(elapsed) : `${percent}%`}
						</span>
						<div class="qrx_rec_bar" class:qrx_rec_bar_idle={percent === null}>
							<div class="qrx_rec_bar_fill" style="width: {percent ?? 0}%"></div>
						</div>
					</div>
				{:else}
					<div class="qrx_rec_actions">
						{#if lastDone === task.id}
							<button class="qrx_rec_log" onclick={() => openLog(task.id)}>
								{i18n.t.notification_text_1}
							</button>
						{/if}
						<button class="qrx_sys_link" disabled={!!running} onclick={() => start(task.id)}>
							{i18n.t.rec_run}
						</button>
					</div>
				{/if}
			</div>
		{/each}
	</div>
</section>

<!-- Прошлая установка Windows -->
<section class="qrx_set_row qrx_set_row_stack">
	<div class="qrx_set_row_text">
		<span class="qrx_set_row_title">{i18n.t.rec_old}</span>
		<span class="qrx_set_row_note">{i18n.t.rec_old_note}</span>
	</div>

	{#if oldPresent}
		<div class="qrx_rec_old">
			<div class="qrx_rec_old_info">
				<span class="qrx_rec_old_size">{oldSize === null ? '…' : formatBytes(oldSize)}</span>
				<span class="qrx_rec_task_note">{oldPath}</span>
				<span class="qrx_rec_warn">{i18n.t.rec_old_warn}</span>
			</div>

			{#if running === 'windows_old'}
				<!-- Цифры, а не одна доля: дерево в полтора миллиона объектов
				     удаляется долго, и живой счётчик убранного — единственное,
				     что отличает работу от зависания -->
				<div class="qrx_rec_state">
					<span class="qrx_rec_percent">
						{purged === null ? clock(purge?.elapsed ?? 0) : `${purged}%`}
					</span>

					<div class="qrx_rec_bar" class:qrx_rec_bar_idle={purged === null}>
						<div class="qrx_rec_bar_fill" style="width: {purged ?? 0}%"></div>
					</div>

					<span class="qrx_rec_task_note">
						{i18n.t.rec_old_gone}: {(purge?.objects ?? 0).toLocaleString()} ·
						{formatBytes(purge?.bytes ?? 0)} · {clock(purge?.elapsed ?? 0)}
					</span>

					<!-- Про общие данные говорим сразу, а не после: иначе цифра
					     обещает столько же свободного места, а его будет меньше -->
					<span class="qrx_rec_task_note">{i18n.t.rec_old_shared}</span>

					{#if purge && purge.failed > 0}
						<span class="qrx_rec_task_note">
							{i18n.t.rec_old_stuck}: {purge.failed.toLocaleString()}
						</span>
					{/if}
				</div>
			{:else}
				<HoldButton
					label={i18n.t.rec_old_delete}
					hint={i18n.t.rec_old_hold}
					ico="qrx_sys_ico_power"
					hold={1400}
					onrun={removeWindowsOld}
				/>
			{/if}
		</div>
	{:else}
		<span class="qrx_rec_task_note">{i18n.t.rec_old_none}</span>
	{/if}
</section>
