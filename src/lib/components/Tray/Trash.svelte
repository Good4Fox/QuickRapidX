<script lang="ts">
	/*
	 * Корзина в панели трея.
	 *
	 * Переделано целиком. Прежде здесь стояла анкета: галочка подтверждения,
	 * две радиокнопки режима удаления, поле предела с кнопкой применения — всё
	 * то, что настраивают один раз в жизни. Панель трея открывают не для
	 * этого: её открывают взглянуть и нажать. Настройки уехали в окно
	 * программы, целиком.
	 *
	 * Осталось три вещи и ровно в том порядке, в каком они нужны: сколько
	 * занято, открыть, очистить.
	 */
	import { invoke } from '@tauri-apps/api/core';
	import { emit } from '@tauri-apps/api/event';

	import { i18n } from '$lib/state/i18n.svelte';
	import { notifyError, notifySuccess } from '$lib/state/notifications.svelte';
	import { formatBytes } from '$lib/state/systemInfo';


	/** Занято в корзине, байты. */
	let usage = $state(0);

	/** Сколько предметов лежит — не только их объём. */
	let items = $state(0);

	/** Предел корзины в мегабайтах, как его отдаёт реестр. */
	let limit = $state(0);

	let busy = $state(false);

	/*
	 * Доля заполнения от предела, 0–1.
	 *
	 * Прижата к единице: предел складывается по томам, а занятое приходит от
	 * оболочки по всем дискам сразу — на машине с необычной разметкой одно
	 * может обогнать другое, и кольцо ушло бы на второй круг.
	 */
	const share = $derived.by(() => {
		const bytes = limit * 1024 * 1024;

		if (bytes <= 0 || usage <= 0) return 0;

		return Math.min(1, usage / bytes);
	});

	const percent = $derived(Math.round(share * 100));

	/*
	 * Кольцо рисуется штрихом по окружности: длина видимой части и есть доля.
	 * Радиус задан здесь, а не в разметке, чтобы длина окружности считалась от
	 * него же и не разъезжалась при правке.
	 */
	const RADIUS = 52;
	const RING = 2 * Math.PI * RADIUS;

	$effect(() => {
		refresh();

		/*
		 * Пока панель открыта, состояние подтягивается само.
		 *
		 * Прежде размер обновлялся нажатием на него — про такую кнопку надо
		 * знать, а панель живёт на экране секунды. Опрос стоит одного обращения
		 * к сводке оболочки: папку он не обходит.
		 */
		const beat = setInterval(refresh, 2000);

		return () => clearInterval(beat);
	});

	async function refresh() {
		try {
			const state = await invoke<{ bytes: number; items: number }>('tray_basket_state');

			usage = state.bytes;
			items = state.items;
		} catch (e) {
			console.error('не удалось прочитать занятость корзины:', e);
		}

		try {
			limit = await invoke<number>('tray_basket_get_max_capacity');
		} catch (e) {
			console.error('не удалось прочитать предел корзины:', e);
		}
	}

	/*
	 * Размер считает общая на всё приложение функция.
	 *
	 * Здесь была своя, с кириллическими единицами, а рядом на вкладке «Система»
	 * работала другая, с латинскими: в одном окне трея на соседних вкладках
	 * стояли «1,50 ГБ» и «8.0 GB».
	 */
	const human = formatBytes;

	/** Размер из мегабайтов — предел корзины хранится в них. */
	function humanMb(megabytes: number): string {
		return formatBytes(megabytes * 1024 * 1024);
	}

	async function open() {
		await invoke('tray_basket_open');
		await invoke('window_doze');
	}

	async function clear() {
		if (items === 0 || busy) return;

		busy = true;

		try {
			/*
			 * Своей защиты от промаха здесь больше нет, и это не упущение.
			 * Подтверждение спрашивает сама Windows — тем же окном, что и у
			 * проводника, — и включено оно по умолчанию. Прежняя галочка
			 * «требовать подтверждение» изображала то же самое своими силами, а
			 * выключить настоящее всё равно не могла.
			 */
			await invoke('bin_empty');
			await invoke('window_doze');

			notifySuccess(i18n.t.appTrayText_1_6, i18n.t.bin_emptied);
		} catch (e) {
			notifyError(i18n.t.appTrayText_1_6, String(e));
		}

		busy = false;

		// Очистка идёт своим чередом — даём проводнику время до перечёта
		await new Promise((resolve) => setTimeout(resolve, 500));
		await refresh();
	}

	/** Уводит в настройки программы: настроек здесь больше нет. */
	async function settings() {
		await invoke('show_main_window');
		await emit('app:open-settings', 'SettingsBin');
		await invoke('window_doze');
	}
</script>

<div class="tray_bin_tab">
	<!-- Кольцо: доля занятого от предела. Значок внутри гаснет на пустой
	     корзине — пустая не должна выглядеть так же, как полная -->
	<div class="tray_bin_gauge">
		<svg class="tray_bin_ring" viewBox="0 0 120 120" aria-hidden="true">
			<circle class="tray_bin_ring_track" cx="60" cy="60" r={RADIUS} />
			<circle
				class="tray_bin_ring_fill"
				cx="60"
				cy="60"
				r={RADIUS}
				stroke-dasharray={RING}
				stroke-dashoffset={RING * (1 - share)}
			/>
		</svg>

		<div class="tray_bin_core">
			<span class="tray_bin_glyph" class:tray_bin_glyph_idle={items === 0}></span>
			<span class="tray_bin_share">{percent}%</span>
		</div>
	</div>

	<div class="tray_bin_numbers">
		<span class="tray_bin_size">{items === 0 ? i18n.t.bin_empty_hint : human(usage)}</span>

		{#if items > 0}
			<span class="tray_bin_meta">
				{items}
				{i18n.t.bin_items}{limit > 0 ? ` · ${i18n.t.bin_of} ${humanMb(limit)}` : ''}
			</span>
		{/if}
	</div>

	<div class="tray_bin_acts">
		<button class="tray_bin_act" onclick={open}>
			<span class="tray_app_trash_panel_selection_button_ico_1"></span>
			<span class="tray_bin_act_text">{i18n.t.appTrayText_2_1}</span>
		</button>

		<button
			class="tray_bin_act"
			class:tray_bin_act_off={items === 0 || busy}
			disabled={items === 0 || busy}
			onclick={clear}
		>
			<span class="tray_app_trash_panel_selection_button_ico_2"></span>
			<span class="tray_bin_act_text">{i18n.t.appTrayText_2_2}</span>
		</button>
	</div>

	<button class="tray_bin_more" onclick={settings}>{i18n.t.bin_menu_settings}</button>
</div>
