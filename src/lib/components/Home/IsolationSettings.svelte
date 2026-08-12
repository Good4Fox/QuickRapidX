<script lang="ts">
	/*
	 * Изоляция одной программы.
	 *
	 * Права здесь разные у каждой: браузеру нужен выход наружу, просмотрщику —
	 * библиотека изображений, а большинству не нужно ничего. Общий набор на всех
	 * означал бы либо запрет того, что программе необходимо, либо разрешение
	 * того, что ей ни к чему.
	 *
	 * Но выкладывать восемь переключателей с названиями прав — это разговор не с
	 * тем человеком. Поэтому два уровня, как у Bottles с его средами: сверху
	 * роль («это браузер», «это просмотрщик»), и она сама расставляет права;
	 * ниже, под «Подробнее», — те же права поштучно, ключи запуска, окружение и
	 * открытые папки. Первый уровень достаточен, второй ничего не прячет.
	 *
	 * Настройки сохраняются сразу, а не по кнопке: это не поля записи, а
	 * поведение запуска, и вести себя они должны как остальные настройки.
	 */
	import { invoke } from '@tauri-apps/api/core';
	import { open } from '@tauri-apps/plugin-dialog';

	import { i18n } from '$lib/state/i18n.svelte';
	import { notifyError, notifySuccess } from '$lib/state/notifications.svelte';

	type EnvVar = { name: string; value: string };

	type Profile = {
		target: string;
		preset: string;
		engine: string;
		sandbox: string;
		capabilities: string[];
		folders: string[];
		exe: string;
		args: string;
		env: EnvVar[];
	};

	type Preset = { id: string; capabilities: string[] };
	type Launch = { ok: boolean; message: string };

	type Props = {
		/** Запись каталога, к которой относятся настройки. */
		target: string;
		/** Название программы — им подписывается ярлык. */
		name: string;
		/** Путь, который можно предложить, если файл ещё не выбран. */
		suggested?: string;
		/**
		 * Урезанный вид — для записи внутри набора.
		 *
		 * Там файл, ключи запуска и сама отметка «в изоляции» уже заданы у
		 * записи, и повторять их здесь значило бы держать одно и то же в двух
		 * местах. Остаются только права: чего именно этой программе разрешено.
		 */
		compact?: boolean;
	};

	let { target, name, suggested = '', compact = false }: Props = $props();

	/** Роль «без изоляции» в ядре не хранится: это отсутствие настройки. */
	const OFF = 'off';
	const CUSTOM = 'custom';

	let profile = $state<Profile>({
		target: '',
		preset: OFF,
		engine: '',
		sandbox: '',
		capabilities: [],
		folders: [],
		exe: '',
		args: '',
		env: []
	});

	/** Роли приходят из ядра: там же лежат наборы прав, что и при запуске. */
	let presets = $state<Preset[]>([]);
	let abilities = $state<string[]>([]);

	let open_more = $state(false);
	let busy = $state(false);

	/*
	 * Чем изолировать.
	 *
	 * Наша среда ничего не требует ставить, но обычная настольная программа то
	 * и дело упирается в то, чего AppContainer не пускает, — и виснет. Sandboxie
	 * держит границу драйвером ядра, и внутри программа живёт как в обычной
	 * системе. Отсюда и честные подписи: одно надёжно, другое пока нет.
	 */
	let boxed = $state({ installed: false, path: '' });
	let wsandbox = $state({ installed: false });

	$effect(() => {
		invoke<{ installed: boolean; path: string }>('sandboxie_status')
			.then((status) => (boxed = status))
			.catch(() => undefined);

		invoke<{ installed: boolean }>('wsandbox_status')
			.then((status) => (wsandbox = status))
			.catch(() => undefined);
	});

	$effect(() => {
		invoke<Preset[]>('container_presets')
			.then((list) => (presets = list))
			.catch(() => undefined);

		invoke<string[]>('container_capabilities')
			.then((list) => (abilities = list))
			.catch(() => undefined);
	});

	// Настройки перечитываются при смене записи: иначе форма показывала бы
	// чужие права после перехода к другой программе
	$effect(() => {
		const id = target;

		invoke<Profile>('container_profile', { target: id })
			.then((loaded) => {
				// Дальше читается локальная копия, а не profile: чтение своего же
				// состояния сделало бы эффект зависимым от него и запустило по
				// кругу на каждом сохранении
				const next = { ...loaded, preset: loaded.preset || OFF };

				profile = next;
				open_more = next.preset === CUSTOM;
			})
			.catch(() => undefined);
	});

	const label: Record<string, () => string> = {
		off: () => i18n.t.iso_p_off,
		browser: () => i18n.t.iso_p_browser,
		messenger: () => i18n.t.iso_p_messenger,
		viewer: () => i18n.t.iso_p_viewer,
		office: () => i18n.t.iso_p_office,
		game: () => i18n.t.iso_p_game,
		offline: () => i18n.t.iso_p_offline,
		free: () => i18n.t.iso_p_free,
		custom: () => i18n.t.iso_p_custom
	};

	const hint: Record<string, () => string> = {
		off: () => i18n.t.iso_p_off_note,
		browser: () => i18n.t.iso_p_browser_note,
		messenger: () => i18n.t.iso_p_messenger_note,
		viewer: () => i18n.t.iso_p_viewer_note,
		office: () => i18n.t.iso_p_office_note,
		game: () => i18n.t.iso_p_game_note,
		offline: () => i18n.t.iso_p_offline_note,
		free: () => i18n.t.iso_p_free_note,
		custom: () => i18n.t.iso_p_custom_note
	};

	const ability: Record<string, () => string> = {
		internet: () => i18n.t.iso_c_internet,
		internet_server: () => i18n.t.iso_c_internet_server,
		private_network: () => i18n.t.iso_c_private_network,
		documents: () => i18n.t.iso_c_documents,
		pictures: () => i18n.t.iso_c_pictures,
		videos: () => i18n.t.iso_c_videos,
		music: () => i18n.t.iso_c_music,
		removable: () => i18n.t.iso_c_removable
	};

	const abilityNote: Record<string, () => string> = {
		internet: () => i18n.t.iso_c_internet_note,
		internet_server: () => i18n.t.iso_c_internet_server_note,
		private_network: () => i18n.t.iso_c_private_network_note,
		documents: () => i18n.t.iso_c_documents_note,
		pictures: () => i18n.t.iso_c_pictures_note,
		videos: () => i18n.t.iso_c_videos_note,
		music: () => i18n.t.iso_c_music_note,
		removable: () => i18n.t.iso_c_removable_note
	};

	/** Ряд ролей: «без изоляции» первой, «своё» последним. */
	const roles = $derived(
		compact
			? [...presets.map((preset) => preset.id), CUSTOM]
			: [OFF, ...presets.map((preset) => preset.id), CUSTOM]
	);

	/** Что выдано — словами, а не именами прав. */
	const allowed = $derived(
		profile.capabilities
			.map((id) => ability[id]?.() ?? id)
			.join(', ')
	);

	const isolated = $derived(compact || profile.preset !== OFF);

	async function commit(next: Partial<Profile>) {
		profile = { ...profile, ...next, target };

		try {
			// Роль «без изоляции» уходит в ядро как есть: пустой набор прав и
			// сохранённая пометка, чтобы список знал, что запускать обычно
			await invoke('container_set_profile', { profile: $state.snapshot(profile) });
		} catch (e) {
			notifyError(i18n.t.notification_text_4, String(e));
		}
	}

	function pickRole(id: string) {
		if (id === OFF) {
			open_more = false;
			commit({ preset: OFF, capabilities: [] });
			return;
		}

		if (id === CUSTOM) {
			// Своё ничего не сбрасывает: набор остаётся тот, что был, — иначе
			// переход к ручной настройке стирал бы уже выбранное
			open_more = true;
			commit({ preset: CUSTOM });
			return;
		}

		const found = presets.find((preset) => preset.id === id);

		commit({ preset: id, capabilities: [...(found?.capabilities ?? [])] });
	}

	function toggleAbility(id: string) {
		const next = profile.capabilities.includes(id)
			? profile.capabilities.filter((each) => each !== id)
			: [...profile.capabilities, id];

		// Правка вручную — это уже своя роль, а не та, что была выбрана
		commit({ preset: CUSTOM, capabilities: next });
	}

	async function pickExe() {
		const picked = await open({
			multiple: false,
			title: i18n.t.iso_pick,
			filters: [{ name: i18n.t.grp_kind_app, extensions: ['exe'] }]
		});

		if (typeof picked === 'string') commit({ exe: picked });
	}

	async function addFolder() {
		const picked = await open({ directory: true, multiple: false, title: i18n.t.iso_folder_add });

		if (typeof picked !== 'string' || profile.folders.includes(picked)) return;

		commit({ folders: [...profile.folders, picked] });
	}

	function dropFolder(path: string) {
		commit({ folders: profile.folders.filter((each) => each !== path) });
	}

	function addVar() {
		commit({ env: [...profile.env, { name: '', value: '' }] });
	}

	function dropVar(index: number) {
		commit({ env: profile.env.filter((_, at) => at !== index) });
	}

	/*
	 * Ярлык на рабочий стол. Идея от Bottles: программу из среды кладут ярлыком
	 * наружу, и дальше она запускается как обычная — о самой обёртке вспоминать
	 * не нужно. Ярлык ведёт на нас с ключом, окно при этом не поднимается.
	 */
	async function shortcut() {
		if (!profile.exe && !suggested) {
			notifyError(i18n.t.notification_text_4, i18n.t.iso_file_none);
			return;
		}

		// Ярлык читает настройки с диска, а не спрашивает у окна: подсказанный
		// путь надо сначала записать, иначе там будет пусто
		if (!profile.exe) await commit({ exe: suggested });

		try {
			const path = await invoke<string>('container_shortcut', { target, name });

			notifySuccess(i18n.t.iso_shortcut_done, path);
		} catch (e) {
			notifyError(i18n.t.notification_text_4, String(e));
		}
	}

	async function run() {
		const exe = profile.exe || suggested;

		if (!exe) {
			notifyError(i18n.t.notification_text_4, i18n.t.iso_file_none);
			return;
		}

		busy = true;

		try {
			// Среда заводится при первом запуске: без неё не появится папка данных
			await invoke('container_ensure');

			const result = await invoke<Launch>('container_run', { target, exe });

			if (result.ok) notifySuccess(i18n.t.iso_started, exe);
			else notifyError(i18n.t.notification_text_4, result.message);
		} catch (e) {
			notifyError(i18n.t.notification_text_4, String(e));
		}

		busy = false;
	}
</script>

<section class="qrx_iso">
	<header class="qrx_iso_head">
		<div class="qrx_set_row_text">
			<span class="qrx_set_row_title">{i18n.t.iso_role}</span>
			<span class="qrx_set_row_note">{i18n.t.iso_role_note}</span>
		</div>

		<span class="qrx_found_tag" class:qrx_found_tag_portable={isolated}>
			{label[profile.preset]?.() ?? profile.preset}
		</span>
	</header>

	{#if isolated}
		<!--
			Пометка стоит всегда, при любом способе изоляции.

			Пробная здесь не «наша среда», а сама возможность: из Store не
			запускается ничего ни одним способом, обычные настольные программы в
			нашей среде виснут, а песочница Windows хоть и надёжна, но теряет
			всё внутри при закрытии. Обещать «просто работает» было бы неправдой
			ни в одном из трёх случаев.
		-->
		<div class="qrx_iso_beta">
			<span class="qrx_found_tag qrx_found_tag_installed">{i18n.t.iso_beta}</span>
			<span class="qrx_rec_task_note">{i18n.t.iso_beta_note}</span>

			{#if profile.engine !== 'wsandbox'}
				<span class="qrx_rec_task_note">{i18n.t.iso_beta_own}</span>
			{/if}
		</div>

		<!-- Чем изолировать: выбор между надёжным и своим -->
		<div class="qrx_found_kinds">
			<button
				class="qrx_found_chip"
				class:qrx_found_chip_active={profile.engine !== 'sandboxie' &&
					profile.engine !== 'wsandbox'}
				data-tooltip={i18n.t.iso_e_own_note}
				data-tooltip-position="top"
				onclick={() => commit({ engine: 'own' })}
			>
				{i18n.t.iso_e_own}
			</button>

			<button
				class="qrx_found_chip"
				class:qrx_found_chip_active={profile.engine === 'sandboxie'}
				disabled={!boxed.installed}
				data-tooltip={boxed.installed ? i18n.t.iso_e_box_note : i18n.t.iso_e_box_none}
				data-tooltip-position="top"
				onclick={() => commit({ engine: 'sandboxie' })}
			>
				{i18n.t.iso_e_box}
			</button>

			<button
				class="qrx_found_chip"
				class:qrx_found_chip_active={profile.engine === 'wsandbox'}
				disabled={!wsandbox.installed}
				data-tooltip={wsandbox.installed ? i18n.t.iso_e_win_note : i18n.t.iso_e_win_none}
				data-tooltip-position="top"
				onclick={() => commit({ engine: 'wsandbox' })}
			>
				{i18n.t.iso_e_win}
			</button>
		</div>

		{#if profile.engine === 'wsandbox'}
			<span class="qrx_rec_warn">{i18n.t.iso_e_win_warn}</span>
		{:else if !boxed.installed && profile.engine === 'sandboxie'}
			<span class="qrx_rec_task_note">{i18n.t.iso_e_box_none}</span>
		{/if}
	{/if}

	<!--
		Первый уровень: роль. Прав здесь не видно, и это намеренно.

		У песочницы Windows ролей нет вовсе: внутри неё своя система целиком, и
		выдавать ей доступ к папкам бессмысленно — их там просто нет. Остаётся
		один выбор, сеть, и он живёт в самой роли. Показывать ряд, который ни на
		что не влияет, — обманывать.
	-->
	<div class="qrx_found_kinds" class:qrx_iso_roles_off={isolated && profile.engine === 'wsandbox'}>
		{#each roles as role (role)}
			<button
				class="qrx_found_chip"
				class:qrx_found_chip_active={profile.preset === role}
				data-tooltip={hint[role]?.()}
				data-tooltip-position="top"
				onclick={() => pickRole(role)}
			>
				{label[role]?.() ?? role}
			</button>
		{/each}
	</div>

	{#if isolated && profile.engine === 'sandboxie'}
		<p class="qrx_iso_sum">{i18n.t.iso_e_box_rights}</p>
	{:else if isolated && profile.engine === 'wsandbox'}
		<p class="qrx_iso_sum">{i18n.t.iso_e_win_rights}</p>
	{:else if isolated}
		<p class="qrx_iso_sum">
			{#if profile.capabilities.length > 0}
				<span class="qrx_iso_sum_key">{i18n.t.iso_allowed}:</span>
				{allowed}
			{:else}
				{i18n.t.iso_allowed_none}
			{/if}
		</p>

		<div class="qrx_iso_actions">
			{#if !compact}
				<button class="qrx_sys_link" disabled={busy} onclick={run}>{i18n.t.iso_run}</button>

				<button
					class="qrx_rec_log"
					data-tooltip={i18n.t.iso_shortcut_note}
					data-tooltip-position="top"
					onclick={shortcut}
				>
					{i18n.t.iso_shortcut}
				</button>
			{/if}

			<button class="qrx_rec_log" onclick={() => (open_more = !open_more)}>
				{open_more ? i18n.t.iso_less : i18n.t.iso_more}
			</button>
		</div>
	{/if}

	<!-- Второй уровень: то же самое поштучно. Ничего не спрятано, просто свёрнуто -->
	{#if isolated && open_more}
		<div class="qrx_iso_deep">
			<div class="qrx_iso_group">
				<span class="qrx_field_label">{i18n.t.iso_rights}</span>

				<div class="qrx_iso_grid">
					{#each abilities as id (id)}
						<label class="qrx_iso_cap">
							<input
								class="qrx_toggle"
								type="checkbox"
								checked={profile.capabilities.includes(id)}
								onchange={() => toggleAbility(id)}
							/>

							<span class="qrx_set_row_text">
								<span class="qrx_iso_cap_name">{ability[id]?.() ?? id}</span>
								<span class="qrx_rec_task_note">{abilityNote[id]?.() ?? ''}</span>
							</span>
						</label>
					{/each}
				</div>
			</div>

			{#if !compact}
			<div class="qrx_iso_group">
				<span class="qrx_field_label">{i18n.t.iso_file}</span>

				<div class="qrx_iso_row">
					<span class="qrx_iso_path">{profile.exe || suggested || i18n.t.iso_file_none}</span>
					<button class="qrx_rec_log" onclick={pickExe}>{i18n.t.iso_file_pick}</button>
				</div>
			</div>

			<label class="qrx_field">
				<span class="qrx_field_label">{i18n.t.iso_args}</span>
				<input
					class="qrx_field_input"
					type="text"
					placeholder="--profile-directory=Work"
					value={profile.args}
					spellcheck="false"
					onchange={(event) => commit({ args: event.currentTarget.value })}
				/>
				<span class="qrx_rec_task_note">{i18n.t.iso_args_note}</span>
			</label>

			<div class="qrx_iso_group">
				<span class="qrx_field_label">{i18n.t.iso_folders}</span>
				<span class="qrx_rec_task_note">{i18n.t.iso_folders_note}</span>

				{#each profile.folders as folder (folder)}
					<div class="qrx_iso_row">
						<span class="qrx_iso_path">{folder}</span>
						<button class="qrx_rec_log qrx_lib_drop" onclick={() => dropFolder(folder)}>
							{i18n.t.iso_drop}
						</button>
					</div>
				{/each}

				<div class="qrx_iso_row">
					<button class="qrx_rec_log" onclick={addFolder}>{i18n.t.iso_folder_add}</button>
				</div>
			</div>

			{/if}

			<div class="qrx_iso_group">
				<span class="qrx_field_label">{i18n.t.iso_env}</span>
				<span class="qrx_rec_task_note">{i18n.t.iso_env_note}</span>

				{#each profile.env as item, index (index)}
					<div class="qrx_iso_row">
						<input
							class="qrx_field_input qrx_iso_key"
							type="text"
							placeholder={i18n.t.iso_env_name}
							value={item.name}
							spellcheck="false"
							onchange={(event) => {
								profile.env[index].name = event.currentTarget.value;
								commit({});
							}}
						/>

						<input
							class="qrx_field_input"
							type="text"
							placeholder={i18n.t.iso_env_value}
							value={item.value}
							spellcheck="false"
							onchange={(event) => {
								profile.env[index].value = event.currentTarget.value;
								commit({});
							}}
						/>

						<button class="qrx_rec_log qrx_lib_drop" onclick={() => dropVar(index)}>
							{i18n.t.iso_drop}
						</button>
					</div>
				{/each}

				<div class="qrx_iso_row">
					<button class="qrx_rec_log" onclick={addVar}>{i18n.t.iso_env_add}</button>
				</div>
			</div>

			<!-- Границы названы здесь же: их надо видеть до запуска, а не после -->
			<span class="qrx_rec_warn">{i18n.t.iso_partial}</span>
		</div>
	{/if}
</section>
