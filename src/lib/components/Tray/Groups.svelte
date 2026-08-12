<script lang="ts">
	/*
	 * Наборы в панели трея.
	 *
	 * У каждого набора своя иконка в системной панели, и щелчок по ней
	 * открывает сразу её содержимое — ради этого всё и затевалось. Ядро
	 * сообщает событием, чей значок нажали; пусто — нажали основной, и тогда
	 * показывается список всех наборов.
	 *
	 * Список перечитывается при каждом показе: собирают наборы в главном окне,
	 * и панель должна показывать сегодняшнее, а не то, что было при её первом
	 * открытии.
	 *
	 * После запуска панель прячется. Она вызвана ради одного действия, и
	 * оставлять её висеть поверх запущенной программы незачем.
	 */
	import { invoke } from '@tauri-apps/api/core';
	import { emit, listen } from '@tauri-apps/api/event';
	import { getCurrentWindow } from '@tauri-apps/api/window';

	import AppIcon from '$lib/components/ui/AppIcon.svelte';
	import HoldButton from '$lib/components/ui/HoldButton.svelte';
	import { i18n } from '$lib/state/i18n.svelte';
	import { groups, type Group } from '$lib/state/groups.svelte';

	const panel = getCurrentWindow();

	let open_id = $state('');
	let failed = $state('');

	/*
	 * Откуда пришли. Наборы вкладываются друг в друга, и возвращаться надо на
	 * шаг назад, а не сразу к списку: иначе из «Работа → Разработка» выход
	 * выкидывал бы в общий список, минуя «Работу».
	 */
	let trail = $state<string[]>([]);

	function enter(id: string) {
		if (open_id) trail = [...trail, open_id];

		// Сообщение об ошибке относилось к прежнему месту: унести его дальше
		// значило бы обвинять набор, в котором ничего не запускали
		failed = '';
		open_id = id;
	}

	function back() {
		failed = '';

		const previous = trail[trail.length - 1];

		trail = trail.slice(0, -1);
		open_id = previous ?? '';
	}

	$effect(() => {
		groups.reload();

		/*
		 * Спрашиваем, что показывать, а не только ждём события.
		 *
		 * Событие уходит один раз, перед показом окна, и при первом открытии
		 * интерфейс подписаться не успевает — панель показывала общий список
		 * вместо нажатого набора. Окно набора этот вопрос задаёт давно, панель
		 * почему-то нет.
		 */
		invoke<string>('tray_opened_group')
			.then((asked) => {
				if (asked) open_id = asked;
			})
			.catch(() => undefined);

		// Чей значок нажали. Приходит до показа окна, поэтому к моменту, когда
		// панель видна, она уже на нужном наборе
		const opened = listen<string>('tray:group', (event) => {
			failed = '';
			trail = [];
			open_id = event.payload ?? '';
			groups.reload();
		});

		// Панель не закрывается, а прячется: при следующем показе перечитываем
		const focus = panel.onFocusChanged(({ payload }) => {
			if (payload) groups.reload();
		});

		return () => {
			opened.then((stop) => stop());
			focus.then((stop) => stop());
		};
	});

	/*
	 * Что показывать в списке.
	 *
	 * Обычно — только отмеченные для панели. Но отметка снята по умолчанию, и
	 * человек, собравший наборы в главном окне, открывал панель и видел
	 * пустоту: наборы есть, а тут ничего. Пустое окно за это не отвечает —
	 * поэтому, когда не отмечен ни один, показываем все и говорим почему.
	 */
	const marked = $derived(groups.inPanel);
	const shown = $derived(marked.length > 0 ? marked : groups.list);
	const showingAll = $derived(marked.length === 0 && groups.list.length > 0);

	/** Уводит в главное окно, на наборы: заводят их там. */
	async function toMain() {
		await invoke('show_main_window');
		await emit('app:open-groups');
		await invoke('window_doze');
	}

	/*
	 * Открытый набор ищется во всём списке, а не только в отмеченных для трея:
	 * значок могли нажать в тот миг, когда отметку уже сняли в окне, — и тогда
	 * лучше показать набор, чем пустоту.
	 */
	const current = $derived(groups.list.find((group) => group.id === open_id) ?? null);

	function icon(group: Group): string {
		return group.icon || group.items[0]?.path || '';
	}

	async function launchAll(group: Group) {
		const result = await groups.launchAll(group.id);

		if (result.ok) invoke('window_doze');
		else failed = result.message;
	}

	async function launchOne(group: string, item: string) {
		const result = await groups.launch(group, item);

		if (result.ok) invoke('window_doze');
		else failed = result.message;
	}
</script>

<div class="qrx_tray_groups">
	{#if current}
		<header class="qrx_tray_head">
			<button class="qrx_rec_log" onclick={back}>{i18n.t.grp_back}</button>
			<span class="qrx_tray_name">{current.name}</span>
		</header>

		<div class="qrx_tray_grid">
			{#each current.items as item (item.id)}
				{@const inner = item.child ? groups.list.find((each) => each.id === item.child) : null}

				<button
					class="qrx_tray_tile"
					class:qrx_tray_tile_nested={item.child}
					data-tooltip={item.tip || (item.child ? i18n.t.grp_nest : item.path)}
					data-tooltip-position="top"
					onclick={() => (item.child ? enter(item.child) : launchOne(current.id, item.id))}
				>
					<AppIcon
						path={item.icon || (inner ? icon(inner) : item.path)}
						fallback={item.name}
						size={40}
					/>
					<span class="qrx_tray_tile_name">{item.name}</span>
				</button>
			{/each}
		</div>

		{#if current.items.length === 0}
			<p class="qrx_rec_task_note">{i18n.t.grp_items_none}</p>
		{:else}
			<!-- Открывает разом все программы набора: слишком весомо для
			     обычного нажатия, тем более в панели у самого края экрана -->
			<HoldButton
				label={i18n.t.grp_launch_all}
				ico="qrx_sys_ico_explorer"
				hint={i18n.t.grp_hold}
				onrun={() => launchAll(current)}
			/>
		{/if}
	{:else}
		<!-- Ни одного набора вовсе: вместо строчки «пусто» — то, ради чего сюда
		     и пришли. Кнопка ведёт туда, где наборы заводят -->
		{#if shown.length === 0}
			<div class="tray_empty">
				<span class="tray_empty_glyph" aria-hidden="true"></span>
				<span class="tray_empty_title">{i18n.t.tray_sets_none}</span>
				<span class="tray_empty_note">{i18n.t.grp_none_tray}</span>

				<button class="tray_empty_go" onclick={toMain}>{i18n.t.tray_sets_make}</button>
			</div>
		{:else if showingAll}
			<p class="tray_empty_note">{i18n.t.tray_sets_hidden}</p>
		{/if}

		{#if shown.length > 0}
			<div class="qrx_tray_list">
				{#each shown as group (group.id)}
					<button class="qrx_tray_row" onclick={() => enter(group.id)}>
						<AppIcon path={icon(group)} fallback={group.name} size={36} />

						<span class="qrx_tray_row_text">
							<span class="qrx_tray_name">{group.name}</span>
							<span class="qrx_rec_task_note">{group.items.length} {i18n.t.grp_count}</span>
						</span>
					</button>
				{/each}
			</div>
		{/if}
	{/if}

	{#if failed}
		<p class="qrx_rec_warn">{failed}</p>
	{/if}
</div>
