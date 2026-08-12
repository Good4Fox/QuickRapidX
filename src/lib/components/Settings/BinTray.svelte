<script lang="ts">
	/*
	 * Корзина: всё, что у неё настраивается.
	 *
	 * Собрано по образцу MiniBin: свой значок в трее, пять ступеней
	 * наполнения, настраиваемое нажатие, три переключателя очистки и свои
	 * значки под каждую ступень. Плюс то, что было раньше в панели трея, —
	 * предел корзины и режим «мимо корзины».
	 *
	 * Живёт только здесь. В панели трея этого не было и не будет: панель
	 * открывают взглянуть и нажать, а не заполнять анкету.
	 *
	 * Оформление — родное, из настроек: строка с названием и пояснением слева,
	 * управление справа, `qrx_toggle` и наш `Select`. Прошлая попытка рисовала
	 * своё — с системным `<select>`, который посреди тёмного окна выглядит
	 * чужой деталью, и без переноса строки: длинный список наезжал на подпись
	 * «Нажатие» и закрывал её целиком.
	 */
	import { invoke } from '@tauri-apps/api/core';
	import { open as pickFile } from '@tauri-apps/plugin-dialog';

	import Select from '$lib/components/ui/Select.svelte';
	import { i18n } from '$lib/state/i18n.svelte';
	import { notifyError } from '$lib/state/notifications.svelte';

	type Bin = {
		tray: boolean;
		steps: boolean;
		click: string;
		confirm: boolean;
		sound: boolean;
		progress: boolean;
		icons: string[];
	};

	/*
	 * Диск и его корзина.
	 *
	 * Корзина в Windows не одна: у каждого тома своя папка `$Recycle.Bin`, и
	 * предел с «мимо корзины» задаются тому, а не системе. Перенести корзину на
	 * соседний диск нельзя — удалённое остаётся там, где лежало. Поэтому
	 * выбирается не место корзины, а диск, чью корзину настраивают.
	 */
	type Volume = {
		root: string;
		label: string;
		total: number;
		free: number;
		limit: number;
		nuke: boolean;
		used: number;
		items: number;
	};

	let bin = $state<Bin | null>(null);

	/** Что оболочка показывает сама: пустая корзина и полная. */
	let shell = $state<string[]>([]);

	/** Диски, у которых есть корзина. */
	let volumes = $state<Volume[]>([]);

	/** Корень выбранного диска: `C:\`. */
	let picked = $state('');

	const disk = $derived(volumes.find((volume) => volume.root === picked) ?? null);

	/** Предел выбранного диска в мегабайтах. Ноль — своего предела нет. */
	const limit = $derived(disk?.limit ?? 0);

	const CLICK_OPTIONS = $derived([
		{ value: 'open', label: i18n.t.bin_click_open },
		{ value: 'empty', label: i18n.t.bin_click_empty }
	]);

	/*
	 * Готовые размеры вместо поля ввода.
	 *
	 * Предел хранится в мегабайтах, и вводить их руками — занятие для того, кто
	 * про мегабайты знает и готов умножать: пять гигабайт это 5120, а не 5000.
	 * Выбирают же люди «пять гигабайт», а не число.
	 */
	const SIZES = [512, 1024, 2048, 5120, 10240, 25600, 51200];

	/*
	 * Доли системного диска.
	 *
	 * Так предел и задумывался у самой Windows: она считает его от размера
	 * диска, а не числом. И для человека это понятнее — «десятая часть диска»
	 * говорит больше, чем «сто гигабайт», особенно когда дисков несколько и
	 * они разные.
	 */
	const SHARES = [1, 3, 5, 10, 15, 20];

	/** Размер выбранного диска в мегабайтах. Ноль — узнать не удалось. */
	const diskMb = $derived(disk ? Math.round(disk.total / 1024 / 1024) : 0);

	/** Как задаётся предел: долей диска или своим числом. */
	let byShare = $state(true);

	/** Своё значение в гигабайтах — гигабайтами его и называют. */
	let ownGb = $state('');

	/** Размер словами: до гигабайта в мегабайтах, дальше в гигабайтах. */
	function size(megabytes: number): string {
		if (megabytes < 1024) return `${megabytes} ${i18n.t.size_mb}`;

		const gigabytes = megabytes / 1024;

		return `${Number.isInteger(gigabytes) ? gigabytes : gigabytes.toFixed(1)} ${i18n.t.size_gb}`;
	}

	/*
	 * Нынешнее значение всегда есть в списке, даже когда оно не из готовых.
	 *
	 * Иначе список показал бы первый пункт вместо того, что стоит на самом
	 * деле, — и человек, ничего не трогая, увидел бы неправду, а первым же
	 * нажатием мимо сменил бы предел, сам того не желая.
	 */
	const SIZE_OPTIONS = $derived.by(() => {
		const known = SIZES.includes(limit) ? SIZES : [...SIZES, limit].sort((a, b) => a - b);

		return [
			...unset(),
			...known
				.filter((value) => value > 0)
				.map((value) => ({ value: String(value), label: size(value) }))
		];
	});

	/*
	 * Пункт «предел не задан», пока его действительно нет.
	 *
	 * У диска, с которого ничего не удаляли, записи в реестре нет. Без такого
	 * пункта список показал бы первый вариант как выбранный — то есть неправду
	 * о том, что стоит на самом деле.
	 */
	function unset(): { value: string; label: string }[] {
		return limit > 0 ? [] : [{ value: '', label: i18n.t.bin_disk_auto }];
	}

	/** Доля диска в мегабайтах. */
	function shareToMb(percent: number): number {
		return Math.max(1, Math.round((diskMb * percent) / 100));
	}

	/** Пункты выбора доли: рядом с процентом сразу видно, сколько это. */
	const SHARE_OPTIONS = $derived([
		...unset(),
		...SHARES.map((percent) => ({
			value: String(percent),
			label: `${percent}%`,
			note: diskMb > 0 ? size(shareToMb(percent)) : ''
		}))
	]);

	/*
	 * Какая доля выбрана сейчас — ближайшая к нынешнему пределу.
	 *
	 * Пустая строка, пока предела нет: у диска, с которого ничего не удаляли,
	 * записи в реестре тоже нет, и подсветить долю значило бы показать выбор,
	 * которого никто не делал.
	 */
	const currentShare = $derived.by(() => {
		if (diskMb <= 0 || limit <= 0) return '';

		const near = SHARES.reduce((best, percent) =>
			Math.abs(shareToMb(percent) - limit) < Math.abs(shareToMb(best) - limit) ? percent : best
		);

		return String(near);
	});

	async function applyShare(percent: string) {
		if (!percent) return;

		await applyLimit(shareToMb(Number(percent)));
	}

	/** Как диск подписан: буква и метка тома, если она есть. */
	function diskName(volume: Volume): string {
		const letter = volume.root.replace(/\\$/, '');

		return volume.label ? `${letter} · ${volume.label}` : letter;
	}

	/*
	 * Пункты выбора диска.
	 *
	 * Рядом с буквой — размер диска и то, сколько лежит в его корзине: выбирать
	 * есть смысл именно по этому, а «C:» и «D:» сами по себе не говорят, где
	 * накопилось.
	 */
	const DISK_OPTIONS = $derived(
		volumes.map((volume) => ({
			value: volume.root,
			label: diskName(volume),
			note: [
				size(Math.round(volume.total / 1024 / 1024)),
				volume.used > 0
					? `${i18n.t.bin_disk_used} ${size(Math.round(volume.used / 1024 / 1024))}`
					: i18n.t.bin_disk_empty
			].join(' · ')
		}))
	);

	/*
	 * Переход на другой диск.
	 *
	 * Способ задания предела и поле ввода пересчитываются здесь же: они
	 * относятся к выбранному диску, и оставить от прежнего его гигабайты
	 * значило бы предложить чужое значение как своё.
	 */
	function focus(root: string) {
		picked = root;
		sync();
	}

	function sync() {
		const chosen = volumes.find((volume) => volume.root === picked);

		ownGb = chosen ? asGb(chosen.limit) : '';

		if (!chosen || chosen.total <= 0 || chosen.limit <= 0) {
			byShare = true;
			return;
		}

		/*
		 * Как показывать предел, решаем по нему самому.
		 *
		 * Если нынешнее значение совпадает с одной из долей — значит его так и
		 * задавали, и открывать надо на долях. Иначе человек вводил своё, и
		 * подсовывать ему доли значило бы прятать то, что он выбрал.
		 */
		const megabytes = Math.round(chosen.total / 1024 / 1024);

		byShare = SHARES.some(
			(share) => Math.abs(Math.round((megabytes * share) / 100) - chosen.limit) <= 64
		);
	}

	/** Мегабайты гигабайтами, как их вводят. */
	function asGb(megabytes: number): string {
		if (megabytes <= 0) return '';

		return (megabytes / 1024).toFixed(megabytes % 1024 === 0 ? 0 : 1);
	}

	/*
	 * Своё значение вводится в гигабайтах, а хранится в мегабайтах.
	 *
	 * В реестре предел лежит в мегабайтах, но вводить их руками — занятие для
	 * того, кто про них знает и готов умножать: пять гигабайт это 5120, а не
	 * 5000. Умножаем сами.
	 */
	async function applyOwn() {
		const asked = Number(ownGb.replace(',', '.'));

		if (!Number.isFinite(asked) || asked <= 0) {
			ownGb = asGb(limit);
			return;
		}

		await applyLimit(Math.max(1, Math.round(asked * 1024)));
	}

	const STEPS = $derived([
		i18n.t.bin_step_0,
		i18n.t.bin_step_1,
		i18n.t.bin_step_2,
		i18n.t.bin_step_3,
		i18n.t.bin_step_4
	]);

	$effect(() => {
		load();
	});

	async function load() {
		try {
			const loaded = await invoke<Bin>('bin_settings');

			// Пять ячеек нужны разметке. Настройки могли прийти из прежней
			// записи, где значков не было вовсе, и добирать их по месту значило
			// бы разбросать эту заботу по всем обращениям
			loaded.icons = [0, 1, 2, 3, 4].map((at) => loaded.icons?.[at] ?? '');
			bin = loaded;

			shell = await invoke<string[]>('bin_shell_icons');
		} catch (e) {
			console.error('не удалось прочитать настройки корзины:', e);
		}

		await loadVolumes();
	}

	/*
	 * Диски и их корзины.
	 *
	 * Выбранный диск сохраняется между обновлениями списка: перечитываем его
	 * после каждой записи, и сбрасывать выбор на первый значило бы уводить
	 * человека с того диска, который он только что настроил.
	 */
	async function loadVolumes() {
		try {
			volumes = await invoke<Volume[]>('bin_volumes');
		} catch (e) {
			console.error('не удалось прочитать диски:', e);
			volumes = [];
		}

		if (!volumes.some((volume) => volume.root === picked)) {
			picked = volumes[0]?.root ?? '';
		}

		sync();
	}

	async function save(next: Partial<Bin>) {
		if (!bin) return;

		bin = { ...bin, ...next };

		try {
			await invoke('bin_settings_save', { bin: $state.snapshot(bin) });
		} catch (e) {
			notifyError(i18n.t.notification_text_4, String(e));
		}
	}

	async function applyLimit(megabytes: number) {
		if (!disk || !Number.isFinite(megabytes) || megabytes <= 0) return;

		try {
			await invoke('bin_volume_limit', { root: disk.root, megabytes: Math.round(megabytes) });
			patch(disk.root, { limit: Math.round(megabytes) });
		} catch (e) {
			notifyError(i18n.t.notification_text_4, String(e));
		}
	}

	async function applyNuke(on: boolean) {
		if (!disk) return;

		try {
			await invoke('bin_volume_nuke', { root: disk.root, on });
			patch(disk.root, { nuke: on });
		} catch (e) {
			notifyError(i18n.t.notification_text_4, String(e));
		}
	}

	/*
	 * Меняем одну запись на месте, а не перечитываем весь список.
	 *
	 * Перечитывание сбросило бы способ задания предела: своё значение в пять
	 * гигабайт совпадает с долей, и после записи поле ввода сменилось бы на
	 * список долей прямо под руками.
	 */
	function patch(root: string, next: Partial<Volume>) {
		volumes = volumes.map((volume) => (volume.root === root ? { ...volume, ...next } : volume));
	}

	async function pickIcon(step: number) {
		if (!bin) return;

		const picked = await pickFile({
			multiple: false,
			title: i18n.t.bin_icon_pick,
			filters: [{ name: i18n.t.bin_icon_kind, extensions: ['ico', 'exe', 'dll', 'png'] }]
		});

		if (typeof picked !== 'string') return;

		const icons = [...bin.icons];
		icons[step] = picked;

		await save({ icons });
	}

	/** Что стоит на ступени сейчас: своё или взятое у оболочки. */
	function iconOf(step: number): string {
		const own = bin?.icons?.[step]?.trim();

		if (own) return own;

		return step === 0 ? (shell[0] ?? '') : (shell[1] ?? '');
	}
</script>

{#if bin}
	<div class="qrx_bin">
		<!-- Значок в трее и всё, что от него зависит -->
		<label class="qrx_bin_line">
			<span class="qrx_set_row_text">
				<span class="qrx_set_row_title">{i18n.t.bin_tray}</span>
				<span class="qrx_set_row_note">{i18n.t.bin_tray_note}</span>
			</span>

			<input
				class="qrx_toggle"
				type="checkbox"
				checked={bin.tray}
				onchange={(event) => save({ tray: event.currentTarget.checked })}
			/>
		</label>

		{#if bin.tray}
			<div class="qrx_bin_line">
				<span class="qrx_set_row_text">
					<span class="qrx_set_row_title">{i18n.t.bin_click}</span>
					<span class="qrx_set_row_note">{i18n.t.bin_click_note}</span>
				</span>

				<Select
					options={CLICK_OPTIONS}
					value={bin.click}
					onpick={(value) => save({ click: value })}
					label={i18n.t.bin_click}
				/>
			</div>

			<label class="qrx_bin_line">
				<span class="qrx_set_row_text">
					<span class="qrx_set_row_title">{i18n.t.bin_steps}</span>
					<span class="qrx_set_row_note">{i18n.t.bin_steps_note}</span>
				</span>

				<input
					class="qrx_toggle"
					type="checkbox"
					checked={bin.steps}
					onchange={(event) => save({ steps: event.currentTarget.checked })}
				/>
			</label>

			<!-- Ступени. Показываем и те, что взяты у оболочки: иначе непонятно,
			     что стоит, пока своего не поставили -->
			<div class="qrx_bin_slots">
				{#each bin.steps ? [0, 1, 2, 3, 4] : [0, 4] as step (step)}
					<button
						class="qrx_bin_slot"
						class:qrx_bin_slot_own={!!bin.icons[step]?.trim()}
						data-tooltip={iconOf(step)}
						data-tooltip-position="top"
						onclick={() => pickIcon(step)}
					>
						<span class="qrx_bin_slot_name">{STEPS[step]}</span>
						<span class="qrx_bin_slot_note">
							{bin.icons[step]?.trim() ? i18n.t.bin_icon_own : i18n.t.bin_icon_shell}
						</span>
					</button>
				{/each}
			</div>

			<button class="qrx_bin_reset" onclick={() => save({ icons: ['', '', '', '', ''] })}>
				{i18n.t.bin_icon_reset}
			</button>
		{/if}

		<!-- Три переключателя очистки. Это собственные флаги оболочки, и
		     действуют они везде: и на значок в трее, и на кнопку в панели -->
		<label class="qrx_bin_line">
			<span class="qrx_set_row_text">
				<span class="qrx_set_row_title">{i18n.t.bin_confirm}</span>
			</span>

			<input
				class="qrx_toggle"
				type="checkbox"
				checked={bin.confirm}
				onchange={(event) => save({ confirm: event.currentTarget.checked })}
			/>
		</label>

		<label class="qrx_bin_line">
			<span class="qrx_set_row_text">
				<span class="qrx_set_row_title">{i18n.t.bin_sound}</span>
			</span>

			<input
				class="qrx_toggle"
				type="checkbox"
				checked={bin.sound}
				onchange={(event) => save({ sound: event.currentTarget.checked })}
			/>
		</label>

		<label class="qrx_bin_line">
			<span class="qrx_set_row_text">
				<span class="qrx_set_row_title">{i18n.t.bin_progress}</span>
			</span>

			<input
				class="qrx_toggle"
				type="checkbox"
				checked={bin.progress}
				onchange={(event) => save({ progress: event.currentTarget.checked })}
			/>
		</label>

		<!--
			Настройки самой корзины Windows. Переехали сюда из панели трея.

			Всё, что ниже, относится к выбранному диску: корзина у каждого тома
			своя, и предел с «мимо корзины» задаются тому, а не системе.
		-->
		<div class="qrx_bin_line">
			<span class="qrx_set_row_text">
				<span class="qrx_set_row_title">{i18n.t.bin_disk}</span>
				<span class="qrx_set_row_note">{i18n.t.bin_disk_note}</span>
			</span>

			{#if volumes.length > 1}
				<Select
					options={DISK_OPTIONS}
					value={picked}
					onpick={focus}
					label={i18n.t.bin_disk}
					up
				/>
			{:else if disk}
				<!-- Диск один — выбирать не из чего, но назвать его стоит -->
				<span class="qrx_set_row_note">{diskName(disk)}</span>
			{:else}
				<span class="qrx_set_row_note">{i18n.t.bin_disk_none}</span>
			{/if}
		</div>

		<div class="qrx_bin_line">
			<span class="qrx_set_row_text">
				<span class="qrx_set_row_title">{i18n.t.appTrayText_3_2}</span>
				<span class="qrx_set_row_note">{i18n.t.bin_limit_note}</span>
			</span>

			<span class="qrx_bin_limit">
				<!-- Два способа задать одно и то же: долей диска и числом -->
				<button
					class="qrx_found_chip"
					class:qrx_found_chip_active={byShare}
					onclick={() => (byShare = true)}
				>
					{i18n.t.bin_limit_share}
				</button>

				<button
					class="qrx_found_chip"
					class:qrx_found_chip_active={!byShare}
					onclick={() => (byShare = false)}
				>
					{i18n.t.bin_limit_own}
				</button>
			</span>
		</div>

		<div class="qrx_bin_line qrx_bin_line_child">
			{#if byShare && diskMb > 0}
				<span class="qrx_set_row_text">
					<span class="qrx_set_row_note">
						{i18n.t.bin_limit_of_disk} {disk ? diskName(disk) : ''}: {size(diskMb)}
					</span>
				</span>

				<Select
					options={SHARE_OPTIONS}
					value={currentShare}
					onpick={applyShare}
					label={i18n.t.bin_limit_share}
					up
				/>
			{:else if byShare}
				<!-- Размер диска не узнали — доля посчиталась бы от неизвестного -->
				<span class="qrx_set_row_note">{i18n.t.bin_limit_no_disk}</span>

				<Select
					options={SIZE_OPTIONS}
					value={String(limit)}
					onpick={(value) => applyLimit(Number(value))}
					label={i18n.t.appTrayText_3_2}
					up
				/>
			{:else}
				<span class="qrx_set_row_text">
					<span class="qrx_set_row_note">
						{i18n.t.bin_limit_now}: {limit > 0 ? size(limit) : i18n.t.bin_disk_auto}
					</span>
				</span>

				<span class="qrx_bin_limit">
					<input
						class="qrx_field_input qrx_bin_limit_field"
						type="text"
						inputmode="decimal"
						bind:value={ownGb}
						onblur={applyOwn}
						onkeydown={(event) => {
							if (event.key !== 'Enter') return;
							event.preventDefault();
							applyOwn();
						}}
					/>
					<span class="qrx_set_row_note">{i18n.t.size_gb}</span>
				</span>
			{/if}
		</div>

		<label class="qrx_bin_line">
			<span class="qrx_set_row_text">
				<span class="qrx_set_row_title">{i18n.t.appTrayText_3_3}</span>
				<span class="qrx_set_row_note">{i18n.t.bin_nuke_note}</span>
			</span>

			<input
				class="qrx_toggle"
				type="checkbox"
				disabled={!disk}
				checked={disk?.nuke ?? false}
				onchange={(event) => applyNuke(event.currentTarget.checked)}
			/>
		</label>
	</div>
{/if}
