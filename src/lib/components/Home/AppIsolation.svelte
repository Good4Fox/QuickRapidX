<script lang="ts">
	/*
	 * Среда изоляции целиком.
	 *
	 * Knox не подменяет пути: он заводит отдельный профиль, а границу держит
	 * ядро Android. У Windows механизм того же рода встроен — AppContainer,
	 * которым изолированы приложения из Store. Прав администратора и драйвера
	 * не требует: разграничение держит ядро Windows.
	 *
	 * Две вещи, которые здесь названы прямо, а не спрятаны: программа видит,
	 * что работает в изоляции, и работает внутри не всякая. Обещать обратное
	 * значило бы обещать то, чего система не даёт.
	 *
	 * Права по программам настраиваются не здесь, а у записи каталога: они у
	 * каждой свои. Здесь — сама среда и её снимки.
	 *
	 * Снимки — мысль от Bottles. Там у среды есть версии: перед тем как
	 * поставить внутрь что-то сомнительное, делается снимок, а если сломалось —
	 * откат. Откатывать есть что: всё, что программа пишет «в систему», на деле
	 * ложится в папку данных среды.
	 */
	import { invoke } from '@tauri-apps/api/core';
	import { listen } from '@tauri-apps/api/event';
	import { open } from '@tauri-apps/plugin-dialog';

	import { i18n } from '$lib/state/i18n.svelte';
	import { notifyError, notifySuccess } from '$lib/state/notifications.svelte';
	import { formatBytes } from '$lib/state/systemInfo';

	type Launch = { ok: boolean; message: string };
	type Snapshot = { name: string; size: number; made: number };
	type Status = { on: boolean; sid: string; data: string };
	type Check = { id: string; ok: boolean | null; detail: string };

	let sid = $state('');
	let data = $state('');
	let network = $state(false);
	let busy = $state(false);

	/*
	 * Проверка песочницы Windows.
	 *
	 * Она отказывается запускаться чаще, чем хотелось бы, и всегда с невнятным
	 * «Не удалось инициализировать Windows Sandbox». За этим стоит небольшой
	 * набор вполне определённых причин, и почти все чинятся одним нажатием.
	 */
	let checks = $state<Check[]>([]);
	let checking = $state(false);

	const CHECK_LABEL: Record<string, () => string> = {
		feature: () => i18n.t.ws_c_feature,
		compute: () => i18n.t.ws_c_compute,
		hvhost: () => i18n.t.ws_c_hvhost,
		switch: () => i18n.t.ws_c_switch,
		account: () => i18n.t.ws_c_account,
		reboot: () => i18n.t.ws_c_reboot,
		build: () => i18n.t.ws_c_build
	};

	const broken = $derived(checks.some((check) => check.ok === false));

	/*
	 * Тяжёлая починка предлагается в двух случаях: когда сборка компонента
	 * разошлась со сборкой системы — это её прямой повод, — и когда список
	 * чист, а песочница всё равно не работает. Второй случай и привёл сюда:
	 * проверка молчит, потому что каждая мелочь по отдельности в порядке.
	 */
	const stale = $derived(checks.some((check) => check.id === 'build' && check.ok === false));
	const deep = $derived(stale || (checks.length > 0 && !broken));

	/** Ждём подтверждения перезагрузки. Она ничем не отменяется задним числом. */
	let asking = $state(false);

	/*
	 * Ход переустановки компонента.
	 *
	 * Работа идёт минуты, и всё это время наружу должно быть видно, что она
	 * идёт: тишина в ответ на нажатие читается как «сломалось». Шаги называет
	 * ядро числом, а доля есть только у первого — остальные два быстрые и о
	 * себе ничего не сообщают.
	 */
	type Step = { stage: number; percent: number };

	let step = $state<Step | null>(null);

	const STAGE_LABEL: Record<number, () => string> = {
		0: () => i18n.t.ws_s0,
		1: () => i18n.t.ws_s1,
		2: () => i18n.t.ws_s2,
		3: () => i18n.t.ws_s3,
		4: () => i18n.t.ws_s4
	};

	$effect(() => {
		const stepping = listen<Step>('ws:step', (event) => (step = event.payload));

		return () => {
			stepping.then((stop) => stop());
		};
	});

	/*
	 * Время работы.
	 *
	 * Доля есть не у каждого шага, а проверка хранилища идёт минуты — без
	 * растущих цифр даже живая полоса читается как «висит». Часы идут только
	 * пока работа идёт: гонять таймер вхолостую незачем.
	 */
	let ticking = $state(0);

	// Через отдельный признак, а не по самому шагу: он приходит новым объектом
	// на каждое изменение доли, и часы сбрасывались бы вместе с ним
	const working = $derived(step !== null);

	$effect(() => {
		if (!working) return;

		const began = Date.now();
		const beat = setInterval(() => (ticking = Math.floor((Date.now() - began) / 1000)), 1000);

		ticking = 0;

		return () => clearInterval(beat);
	});

	const spent = $derived(
		`${Math.floor(ticking / 60)}:${String(ticking % 60).padStart(2, '0')}`
	);

	async function inspect() {
		checking = true;

		try {
			checks = await invoke<Check[]>('wsandbox_check');
		} catch (e) {
			notifyError(i18n.t.notification_text_4, String(e));
		}

		checking = false;
	}

	async function repair() {
		checking = true;

		try {
			await invoke('wsandbox_repair');
			await inspect();

			notifySuccess(i18n.t.ws_title, i18n.t.ws_repaired);
		} catch (e) {
			notifyError(i18n.t.notification_text_4, String(e));
		}

		checking = false;
	}

	async function reinstall() {
		checking = true;
		// Ноль, а не null: работа началась, просто первый шаг ещё не объявлен —
		// пока человек не согласится дать права, ядру нечего сообщать
		step = { stage: 0, percent: -1 };

		try {
			await invoke('wsandbox_reinstall');
			step = null;

			await inspect();

			// Проверка после переустановки покажет прежнее: сборка компонента
			// сменится только при загрузке. Поэтому говорим прямо про неё
			notifySuccess(i18n.t.ws_title, i18n.t.ws_deep_done);
			asking = true;
		} catch (e) {
			step = null;
			notifyError(i18n.t.notification_text_4, String(e));
		}

		checking = false;
	}

	async function restart() {
		try {
			await invoke('wsandbox_restart', { reason: i18n.t.ws_reboot_reason });
		} catch (e) {
			notifyError(i18n.t.notification_text_4, String(e));
		}

		asking = false;
	}

	let shots = $state<Snapshot[]>([]);
	let title = $state('');
	/** Подтверждение отката: имя снимка, к которому собираются вернуться. */
	let confirming = $state('');

	/*
	 * Среда могла быть заведена в прошлый раз — тогда экран должен открыться уже
	 * включённым. SID выводится из имени и есть всегда, поэтому судим по папке
	 * данных: её создаёт система.
	 */
	/*
	 * Что стало с запущенной программой.
	 *
	 * Программа в изоляции умирает молча: сообщить о нехватке прав ей нечем —
	 * окна ещё нет, а вывод некуда девать. Ядро следит за ней первые секунды и
	 * присылает сюда то, что смогло понять: код выхода или «жива, но окна не
	 * показала». Молчание в ответ на «не открылось» — худшее, что можно выдать.
	 */
	$effect(() => {
		const reported = listen<[string, string]>('iso:report', (event) => {
			const [exe, text] = event.payload;
			// Делим и по обратной косой: пути здесь windows-овские
			const tail = exe.split(/[\\/]/).pop() ?? exe;

			notifyError(tail, text);
		});

		return () => {
			reported.then((stop) => stop());
		};
	});

	$effect(() => {
		invoke<Status>('container_status')
			.then((status) => {
				if (!status.on) return;

				sid = status.sid;
				data = status.data;

				invoke<Snapshot[]>('container_snapshots')
					.then((list) => (shots = list))
					.catch(() => undefined);
			})
			.catch(() => undefined);
	});

	async function turnOff() {
		busy = true;

		try {
			await invoke('container_remove');
			sid = '';
			data = '';
		} catch (e) {
			notifyError(i18n.t.notification_text_4, String(e));
		}

		busy = false;
	}

	async function refresh() {
		data = await invoke<string>('container_data').catch(() => '');
		shots = await invoke<Snapshot[]>('container_snapshots').catch(() => []);
	}

	async function run() {
		const picked = await open({
			multiple: false,
			title: i18n.t.iso_pick,
			filters: [{ name: i18n.t.grp_kind_app, extensions: ['exe'] }]
		});

		if (typeof picked !== 'string') return;

		busy = true;

		/*
		 * Разовый запуск: права минимальные, сеть — только если её попросили.
		 * Постоянные наборы живут у записей каталога.
		 *
		 * Идёт он нашей средой: у Sandboxie и у песочницы Windows свои правила,
		 * и выбирать их надо у самой программы, а не мимоходом.
		 */
		const result = await invoke<Launch>('container_launch', {
			exe: picked,
			capabilities: network ? ['internet'] : [],
			folders: [],
			args: null,
			env: null,
			engine: null,
			sandbox: null
		});

		busy = false;

		if (result.ok) {
			notifySuccess(i18n.t.iso_started, picked);

			// Среда могла завестись прямо сейчас — покажем это без перезахода
			const status = await invoke<Status>('container_status').catch(() => null);

			if (status?.on) {
				sid = status.sid;
				data = status.data;
			}
		} else {
			notifyError(i18n.t.notification_text_4, result.message);
		}
	}

	async function snapshot() {
		const name = title.trim();

		if (!name || busy) return;

		busy = true;

		try {
			await invoke('container_snapshot', { name });
			notifySuccess(i18n.t.iso_snap_made, name);
			title = '';
			await refresh();
		} catch (e) {
			notifyError(i18n.t.notification_text_4, String(e));
		}

		busy = false;
	}

	async function restore(name: string) {
		busy = true;
		confirming = '';

		try {
			await invoke('container_snapshot_restore', { name });
			notifySuccess(i18n.t.iso_snap_done, name);
		} catch (e) {
			notifyError(i18n.t.notification_text_4, String(e));
		}

		busy = false;
	}

	/*
	 * Вынос и внесение — то же копирование, но наружу и обратно. Это и есть
	 * перенос среды на другую машину: там папка вносится и разворачивается.
	 */
	async function exportShot(name: string) {
		const folder = await open({ directory: true, multiple: false, title: i18n.t.iso_snap_export });

		if (typeof folder !== 'string') return;

		busy = true;

		try {
			const path = await invoke<string>('container_snapshot_export', { name, folder });
			notifySuccess(i18n.t.iso_snap_exported, path);
		} catch (e) {
			notifyError(i18n.t.notification_text_4, String(e));
		}

		busy = false;
	}

	async function importShot() {
		const folder = await open({ directory: true, multiple: false, title: i18n.t.iso_snap_import });

		if (typeof folder !== 'string') return;

		busy = true;

		try {
			const added = await invoke<string>('container_snapshot_import', {
				folder,
				fallback: i18n.t.iso_snap_default
			});
			notifySuccess(i18n.t.iso_snap_imported, added);
			await refresh();
		} catch (e) {
			notifyError(i18n.t.notification_text_4, String(e));
		}

		busy = false;
	}

	async function forget(name: string) {
		try {
			await invoke('container_snapshot_forget', { name });
			await refresh();
		} catch (e) {
			notifyError(i18n.t.notification_text_4, String(e));
		}
	}

	/** Дата снимка человеческим видом. В ядре хранятся секунды от эпохи. */
	function when(made: number): string {
		if (!made) return '';

		return new Date(made * 1000).toLocaleString(i18n.current.tag);
	}
</script>

<section class="qrx_set_row qrx_set_row_stack">
	<div class="qrx_set_row_head">
		<div class="qrx_set_row_text">
			<span class="qrx_set_row_title">{i18n.t.iso_title}</span>
			<span class="qrx_set_row_note">{i18n.t.iso_note}</span>
		</div>

		<span class="qrx_found_tag" class:qrx_found_tag_portable={sid} class:qrx_found_tag_installed={!sid}>
			{sid ? i18n.t.iso_on : i18n.t.iso_off}
		</span>
	</div>

	<!--
		Пометка стоит у самого входа, а не только у настроек отдельной
		программы. Это дверь во всю возможность целиком, и узнать, что она
		пробная, человек должен здесь — до того, как настроит десяток программ и
		обнаружит, что половина не запускается.
	-->
	<div class="qrx_iso_beta">
		<span class="qrx_found_tag qrx_found_tag_installed">{i18n.t.iso_beta}</span>
		<span class="qrx_rec_task_note">{i18n.t.iso_beta_note}</span>
		<span class="qrx_rec_task_note">{i18n.t.iso_beta_own}</span>
	</div>

	{#if sid}
		<span class="qrx_rec_task_note">{sid}</span>

		{#if data}
			<div class="qrx_iso_row">
				<span class="qrx_iso_path">{data}</span>
				<button class="qrx_rec_log" onclick={() => invoke('shell_open', { path: data })}>{i18n.t.lib_open}</button>
			</div>
		{/if}

		<label class="qrx_place_switch">
			<input class="qrx_toggle" type="checkbox" bind:checked={network} />
			<span class="qrx_set_row_text">
				<span class="qrx_set_row_title">{i18n.t.iso_net}</span>
				<span class="qrx_rec_task_note">{i18n.t.iso_net_note}</span>
			</span>
		</label>

		<div class="qrx_lib_actions">
			<button class="qrx_sys_link" disabled={busy} onclick={run}>{i18n.t.iso_run}</button>

			<!-- Не «выключить», а «сбросить»: среда заводится сама при первом
			     запуске, и включать её больше не нужно. Убрать её имеет смысл
			     только чтобы стереть всё, что программы в ней накопили -->
			<button class="qrx_rec_log qrx_lib_drop" disabled={busy} onclick={turnOff}>
				{i18n.t.iso_reset}
			</button>
		</div>

		<!-- Снимки среды: откат к состоянию до того, как что-то сломалось -->
		<div class="qrx_iso_group">
			<span class="qrx_field_label">{i18n.t.iso_snap_title}</span>
			<span class="qrx_rec_task_note">{i18n.t.iso_snap_note}</span>

			<div class="qrx_iso_row">
				<input
					class="qrx_field_input"
					type="text"
					placeholder={i18n.t.iso_snap_name}
					bind:value={title}
					spellcheck="false"
				/>

				<button class="qrx_sys_link" disabled={busy || !title.trim()} onclick={snapshot}>
					{busy ? i18n.t.iso_snap_busy : i18n.t.iso_snap_make}
				</button>

				<button
					class="qrx_rec_log"
					disabled={busy}
					data-tooltip={i18n.t.iso_snap_export_note}
					data-tooltip-position="top"
					onclick={importShot}
				>
					{i18n.t.iso_snap_import}
				</button>
			</div>

			{#if shots.length === 0}
				<span class="qrx_rec_task_note">{i18n.t.iso_snap_none}</span>
			{/if}

			{#each shots as shot (shot.name)}
				<div class="qrx_iso_shot">
					<div class="qrx_found_main">
						<span class="qrx_found_name">{shot.name}</span>
						<span class="qrx_rec_task_note">{when(shot.made)} · {formatBytes(shot.size)}</span>
					</div>

					{#if confirming === shot.name}
						<!-- Откат стирает нынешнее содержимое, поэтому спрашивается -->
						<span class="qrx_rec_warn">{i18n.t.iso_snap_warn}</span>

						<button class="qrx_sys_link" disabled={busy} onclick={() => restore(shot.name)}>
							{i18n.t.iso_snap_yes}
						</button>

						<button class="qrx_rec_log" onclick={() => (confirming = '')}>
							{i18n.t.cat_cancel}
						</button>
					{:else}
						<button
							class="qrx_rec_log"
							disabled={busy}
							onclick={() => (confirming = shot.name)}
						>
							{i18n.t.iso_snap_restore}
						</button>

						<button
							class="qrx_rec_log"
							disabled={busy}
							data-tooltip={i18n.t.iso_snap_export_note}
							data-tooltip-position="top"
							onclick={() => exportShot(shot.name)}
						>
							{i18n.t.iso_snap_export}
						</button>

						<button class="qrx_rec_log qrx_lib_drop" onclick={() => forget(shot.name)}>
							{i18n.t.iso_drop}
						</button>
					{/if}
				</div>
			{/each}
		</div>
	{:else}
		<!-- Среда ещё не заведена. Кнопки «включить» нет намеренно: она появится
		     сама при первом запуске в изоляции — хоть отсюда, хоть из набора -->
		<span class="qrx_rec_task_note">{i18n.t.iso_auto}</span>

		<div class="qrx_lib_actions">
			<button class="qrx_sys_link" disabled={busy} onclick={run}>{i18n.t.iso_run}</button>
		</div>
	{/if}

	<!-- Песочница Windows: проверка и починка -->
	<div class="qrx_iso_group">
		<span class="qrx_field_label">{i18n.t.ws_title}</span>
		<span class="qrx_rec_task_note">{i18n.t.ws_note}</span>

		{#each checks as check (check.id)}
			<div class="qrx_iso_row">
				<span
					class="qrx_found_tag"
					class:qrx_found_tag_portable={check.ok === true}
					class:qrx_found_tag_installed={check.ok !== true}
				>
					{check.ok === true ? i18n.t.ws_ok : check.ok === false ? i18n.t.ws_bad : '?'}
				</span>

				<span class="qrx_set_row_text">
					<span class="qrx_iso_cap_name">{CHECK_LABEL[check.id]?.() ?? check.id}</span>
					<span class="qrx_rec_task_note">{check.detail}</span>
				</span>
			</div>
		{/each}

		<div class="qrx_iso_row">
			<button class="qrx_rec_log" disabled={checking} onclick={inspect}>
				{checking ? i18n.t.ws_checking : i18n.t.ws_check}
			</button>

			{#if broken}
				<button class="qrx_sys_link" disabled={checking} onclick={repair}>
					{i18n.t.ws_repair}
				</button>
			{/if}
		</div>

		{#if broken}
			<span class="qrx_rec_task_note">{i18n.t.ws_repair_note}</span>
			<span class="qrx_rec_warn">{i18n.t.ws_reboot_note}</span>
		{/if}

		<!-- Переустановка компонента: последнее средство, и оно с перезагрузкой -->
		{#if deep}
			<div class="qrx_iso_row">
				<button class="qrx_sys_link" disabled={checking} onclick={reinstall}>
					{i18n.t.ws_deep}
				</button>

				{#if asking}
					<button class="qrx_rec_log" onclick={restart}>{i18n.t.ws_restart_ask}</button>
				{:else}
					<button class="qrx_rec_log" onclick={() => (asking = true)}>
						{i18n.t.ws_restart}
					</button>
				{/if}
			</div>

			<!-- Ход работы: пока он идёт, объяснять кнопку уже поздно -->
			{#if step}
				<div class="qrx_ws_step">
					<span class="qrx_iso_cap_name">{STAGE_LABEL[step.stage]?.() ?? ''}</span>

					<span class="qrx_ws_share">
						{step.percent >= 0 ? `${step.percent.toFixed(0)}% · ${spent}` : spent}
					</span>
				</div>

				<!-- Доля известна не всегда: у шагов без неё полоса просто ходит
				     туда-сюда — она показывает, что работа идёт, а не сколько её -->
				<div class="qrx_ws_bar" class:qrx_ws_bar_vague={step.percent < 0}>
					<div
						class="qrx_ws_fill"
						style={step.percent >= 0 ? `width: ${step.percent}%` : ''}
					></div>
				</div>
			{:else}
				<span class="qrx_rec_task_note">{i18n.t.ws_deep_note}</span>
				<span class="qrx_rec_warn">{i18n.t.ws_deep_reboot}</span>
			{/if}

			{#if asking}
				<span class="qrx_rec_task_note">{i18n.t.ws_restart_note}</span>
			{/if}
		{/if}
	</div>

	<!-- Границы названы здесь, а не в справке: их надо видеть до включения -->
	<span class="qrx_rec_warn">{i18n.t.iso_visible}</span>
	<span class="qrx_rec_task_note">{i18n.t.iso_partial}</span>
	<span class="qrx_rec_task_note">{i18n.t.iso_profile}</span>
</section>
