<script lang="ts">
	/*
	 * Цвет рамки выделения.
	 *
	 * Что изменилось против прежней версии:
	 *
	 * * цвет подтягивается из системы, а не начинается с пустых полей —
	 *   раньше поля были пусты, а предпросмотр показывал синий «как обычно»,
	 *   даже если у человека стоял свой;
	 * * применяется сразу, без перезахода;
	 * * вместо двух полей, куда надо вписывать «0 102 204» руками, — готовые
	 *   сочетания, своя палитра и обычный выбор цвета;
	 * * в предпросмотре стоит настоящий курсор пользователя.
	 */
	import { invoke } from '@tauri-apps/api/core';

	import { i18n } from '$lib/state/i18n.svelte';
	import { notifyError, notifySuccess } from '$lib/state/notifications.svelte';
	import CursorPreview from '$lib/components/Settings/CursorPreview.svelte';
	import ColorPicker from '$lib/components/ui/ColorPicker.svelte';

	type Rgb = [number, number, number];
	type Swatch = { fill: Rgb; stroke: Rgb };

	/** Подмешивает белого — 0 оставляет цвет, 1 делает его белым. */
	function lighten([r, g, b]: Rgb, amount: number): Rgb {
		const mix = (part: number) => Math.round(part + (255 - part) * amount);
		return [mix(r), mix(g), mix(b)];
	}

	/*
	 * Готовые сочетания.
	 *
	 * Обводка — не тот же цвет чуть светлее, а сильно разбелённый: у рамки
	 * выделения она читается почти белой с лёгким оттенком заливки. Раньше
	 * здесь стояли два близких насыщенных цвета, и рамка выходила сплошь синей,
	 * непохожей на настоящую.
	 */
	const BASES: Rgb[] = [
		[0, 102, 204],
		[0, 150, 136],
		[124, 77, 255],
		[216, 27, 96],
		[245, 124, 0],
		[67, 160, 71],
		[84, 110, 122],
		[221, 170, 29]
	];

	const PRESETS: Swatch[] = BASES.map((base) => ({ fill: base, stroke: lighten(base, 0.72) }));

	let fill = $state<Rgb>([0, 102, 204]);
	let stroke = $state<Rgb>([0, 120, 215]);
	let saved = $state<Swatch[]>([]);

	/*
	 * Крутить протяжку без остановки незачем: посмотрел — и хватит. Выбор
	 * запоминается, чтобы не выключать её при каждом заходе в настройки.
	 */
	const MOTION_KEY = 'SelectionPreviewMotion';

	let animated = $state(true);

	$effect(() => {
		animated = localStorage.getItem(MOTION_KEY) !== 'off';
	});

	function toggleMotion() {
		animated = !animated;
		localStorage.setItem(MOTION_KEY, animated ? 'on' : 'off');
	}

	$effect(() => {
		load();
	});

	async function load() {
		try {
			const current = await invoke<Swatch>('selection_get');
			fill = current.fill;
			stroke = current.stroke;
		} catch (e) {
			console.error('не удалось прочитать цвет выделения:', e);
		}

		try {
			saved = await invoke<Swatch[]>('selection_palette');
		} catch (e) {
			console.error('не удалось прочитать палитру:', e);
		}
	}

	function pick(swatch: Swatch) {
		fill = swatch.fill;
		stroke = swatch.stroke;
		apply();
	}

	/*
	 * Придержка записи.
	 *
	 * Пока ползунок тянут, цвет меняется на каждый кадр — это шесть десятков
	 * записей в реестр в секунду и столько же смен системных цветов. Здесь
	 * применяется не чаще раза в 120 мс, а последнее значение доходит всегда:
	 * отложенный вызов читает состояние заново, а не то, что было при нажатии.
	 */
	const THROTTLE = 120;

	let pending = 0;
	let last = 0;

	function apply() {
		const now = performance.now();
		const wait = Math.max(0, THROTTLE - (now - last));

		clearTimeout(pending);
		pending = window.setTimeout(() => {
			last = performance.now();
			push();
		}, wait);
	}

	async function push() {
		try {
			await invoke('selection_apply', { fill, stroke });
		} catch (e) {
			notifyError(i18n.t.notification_text_4, String(e));
		}
	}

	$effect(() => () => clearTimeout(pending));

	async function save() {
		try {
			saved = await invoke<Swatch[]>('selection_palette_add', { fill, stroke });
			notifySuccess(i18n.t.sel_added, '');
		} catch (e) {
			notifyError(i18n.t.notification_text_4, String(e));
		}
	}

	async function remove(index: number) {
		try {
			saved = await invoke<Swatch[]>('selection_palette_remove', { index });
		} catch (e) {
			notifyError(i18n.t.notification_text_4, String(e));
		}
	}

	const fillCss = $derived(`rgba(${fill.join(', ')}, 0.5)`);
	const strokeCss = $derived(`rgb(${stroke.join(', ')})`);

	/*
	 * Подсветка выделенного значка.
	 *
	 * Не тот же цвет, что у заливки рамки: под значком Windows кладёт заметно
	 * более светлый, почти пастельный тон. Раньше здесь стоял чистый цвет
	 * заливки, и подсветка выходила густой и тёмной — не похожей на настоящую.
	 */
	const tintCss = $derived(`rgba(${lighten(fill, 0.45).join(', ')}, 0.55)`);
</script>

<section class="qrx_set_row qrx_set_row_stack">
	<div class="qrx_set_row_text">
		<span class="qrx_set_row_title">{i18n.t.sel_title}</span>
		<span class="qrx_set_row_note">{i18n.t.sel_note}</span>
	</div>

	<div class="qrx_sel">
		<!--
			Предпросмотр: кусочек рабочего стола со своим курсором. Рамка и
			курсор идут одной анимацией с общими ключевыми кадрами — иначе они
			разъезжались бы, а выделение должно тянуться ровно за кончиком.
		-->
		<div
			class="qrx_win_box_preview"
			class:qrx_sel_static={!animated}
			style="--sel-tint: {tintCss}"
		>
			<button
				class="qrx_sel_toggle"
				aria-label={animated ? i18n.t.sel_pause : i18n.t.sel_play}
				data-tooltip={animated ? i18n.t.sel_pause : i18n.t.sel_play}
				data-tooltip-position="left"
				onclick={toggleMotion}
			>
				<span
					class="qrx_sel_toggle_mark"
					class:qrx_sel_toggle_pause={animated}
					class:qrx_sel_toggle_play={!animated}
					aria-hidden="true"
				></span>
			</button>

			<div class="qrx_win_box_file qrx_sel_mark">
				<div class="qrx_win_box_file_ico"></div>
				<div class="qrx_win_box_file_text">{i18n.t.settingsWindowsText_3_3_2}</div>
			</div>
			<div
				class="qrx_win_box_rect qrx_sel_drag"
				style="background-color: {fillCss}; border: 1px solid {strokeCss};"
			></div>
			<div class="qrx_sel_pointer">
				<CursorPreview />
			</div>
		</div>

		<div class="qrx_sel_controls">
			<!-- Ровно два цвета, поэтому поля рядом, а не списком -->
			<div class="qrx_sel_inputs">
				<div class="qrx_sel_input">
					<span class="qrx_sel_input_label">{i18n.t.sel_fill}</span>
					<ColorPicker
						value={fill}
						label={i18n.t.sel_fill}
						onpick={(next) => {
							fill = next;
							apply();
						}}
					/>
				</div>

				<div class="qrx_sel_input">
					<span class="qrx_sel_input_label">{i18n.t.sel_stroke}</span>
					<ColorPicker
						value={stroke}
						label={i18n.t.sel_stroke}
						onpick={(next) => {
							stroke = next;
							apply();
						}}
					/>
				</div>

				<button class="qrx_rec_log" onclick={load}>{i18n.t.sel_current}</button>
				<button class="qrx_sys_link" onclick={save}>{i18n.t.sel_save}</button>
			</div>

			<div class="qrx_sel_group">
				<span class="qrx_rec_task_note">{i18n.t.sel_presets}</span>
				<div class="qrx_sel_swatches">
					{#each PRESETS as preset (preset.stroke.join())}
						<button
							class="qrx_sel_swatch"
							style="background-color: rgba({preset.fill.join(', ')}, 0.55); border-color: rgb({preset.stroke.join(
								', '
							)})"
							aria-label={i18n.t.sel_apply}
							onclick={() => pick(preset)}
						></button>
					{/each}
				</div>
			</div>

			<div class="qrx_sel_group">
				<span class="qrx_rec_task_note">{i18n.t.sel_saved}</span>

				{#if saved.length === 0}
					<span class="qrx_rec_task_note">{i18n.t.sel_saved_none}</span>
				{:else}
					<div class="qrx_sel_swatches">
						{#each saved as swatch, index (swatch.fill.join() + swatch.stroke.join())}
							<div class="qrx_sel_saved">
								<button
									class="qrx_sel_swatch"
									style="background-color: rgba({swatch.fill.join(
										', '
									)}, 0.55); border-color: rgb({swatch.stroke.join(', ')})"
									aria-label={i18n.t.sel_apply}
									onclick={() => pick(swatch)}
								></button>
								<button
									class="qrx_sel_drop"
									aria-label={i18n.t.sel_remove}
									onclick={() => remove(index)}
								>
									×
								</button>
							</div>
						{/each}
					</div>
				{/if}
			</div>
		</div>
	</div>
</section>
