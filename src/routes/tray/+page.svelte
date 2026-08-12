<script lang="ts">
	/*
	 * Окно трея (390×490) — размеры взяты из исходного приложения.
	 * Разметка и классы перенесены как есть, оформление лежит в tray.css.
	 */
	import { invoke } from '@tauri-apps/api/core';
	import { emit, listen } from '@tauri-apps/api/event';

	import Groups from '$lib/components/Tray/Groups.svelte';
	import Trash from '$lib/components/Tray/Trash.svelte';
	import System from '$lib/components/Tray/System.svelte';
	import NotificationToasts from '$lib/components/Notifications/NotificationToasts.svelte';
	import HoldIcon from '$lib/components/ui/HoldIcon.svelte';
	import { i18n } from '$lib/state/i18n.svelte';


	type View = 'home' | 'system' | 'trash';

	let view = $state<View>('home');

	/*
	 * Выход взводится нажатием, а не только удержанием.
	 *
	 * Раньше короткое нажатие не делало ничего — и с клавиатуры кнопка была
	 * недостижима вовсе, хотя заведена была именно ради этого. Теперь первое
	 * нажатие взводит, второе выходит; удержание по-прежнему выходит сразу.
	 * Взвод сам спадает через три секунды, чтобы не остаться заряженным.
	 */
	let armed = $state(false);
	let disarm: ReturnType<typeof setTimeout> | null = null;

	function askQuit() {
		if (armed) {
			quit();
			return;
		}

		armed = true;

		if (disarm) clearTimeout(disarm);
		disarm = setTimeout(() => (armed = false), 3000);
	}

	function quit() {
		if (disarm) clearTimeout(disarm);
		armed = false;
		invoke('app_quit');
	}

	$effect(() => {
		i18n.restore();

		/*
		 * Щелчок по значку набора должен показать набор, а не то, на чём панель
		 * закрылась в прошлый раз. Иначе человек, оставивший её на корзине,
		 * увидел бы корзину.
		 */
		const opened = listen('tray:group', () => (view = 'home'));

		return () => {
			opened.then((stop) => stop());
		};
	});

	/** Разделы панели. Порядок — по частоте: наборы, система, корзина. */
	const TABS = $derived([
		{ id: 'home' as View, label: i18n.t.tray_sets, ico: 'tray_app_bottom_link_back_ico_1' },
		{ id: 'system' as View, label: i18n.t.tray_system, ico: 'tray_app_bottom_link_back_ico_sys' },
		{ id: 'trash' as View, label: i18n.t.appTrayText_1_6, ico: 'tray_app_bottom_link_back_ico_6' }
	]);

	/*
	 * Знак приложения открывает главное окно — именно открывает.
	 *
	 * Стояло `app_tray_popen`, а это переключатель: если окно уже видно, он его
	 * **прячет**. Панель при этом закрывалась следом, и нажатие на знак давало
	 * ровно ничего — оба окна исчезали. Со стороны это выглядело как поломка,
	 * хотя ядро делало ровно то, что его просили.
	 */
	function openMain() {
		invoke('show_main_window');
		invoke('window_doze');
	}

	/** Настройки программы: они в главном окне, панель за ними не ходит. */
	async function openSettings() {
		await invoke('show_main_window');
		await emit('app:open-settings', '');
		await invoke('window_doze');
	}
</script>

<!-- Окошки уведомлений: без них сообщения панели уходили в никуда —
     показывать их было некому, а запись оседала только в списке главного окна -->
<NotificationToasts />

<div class="mine_home">
	<div class="titlebar_tray">
		<div class="titlebar_tray_left">
			<button class="titlebar_tray_left_logo" onclick={openMain}>
				<div class="titlebar_tray_left_logo_ico"></div>
				<div class="titlebar_tray_left_logo_text">{i18n.t.home_app_text_1}</div>
			</button>
		</div>

		<div class="titlebar_tray_right">
			<div class="titlebar_tray_right_use">
				<!--
					Кнопка «скрыть» убрана: она делала ровно то же, что крестик
					теперь, и две одинаковые по смыслу кнопки рядом только
					сбивали с толку.

					Крестик здесь ведёт себя так же, как в главном окне:
					нажатие прячет, удержание выходит. Правило одно на оба окна —
					его достаточно узнать один раз.
				-->
				<HoldIcon
					button="titlebar_tray_right_button_close"
					ico="titlebar_tray_right_button_close_img"
					label={i18n.t.win_x_panel}
					holdLabel={i18n.t.win_x_quit}
					onclick={() => invoke('window_doze')}
					onhold={() => invoke('app_quit')}
				/>
			</div>
		</div>
	</div>

	<div class="home">
		<div class="tray_app">
			<div class="tray_app_top">
				<div class="tray_app_top_panel">
					{#if view === 'home'}
						<Groups />
					{:else if view === 'system'}
						<System />
					{:else}
						<Trash />
					{/if}
				</div>
			</div>

			<!--
				Нижний ряд: слева разделы, справа действия над самим окном.

				Было четыре заглушки «?» и две кнопки. Заглушки убраны: место в
				панели дорогое, а знак вопроса не делает ничего и обещает, что
				когда-нибудь будет делать. Вместо них — то, что ядро уже умеет.
			-->
			<div class="tray_app_bottom">
				<div class="tray_app_bottom_use">
					{#each TABS as tab (tab.id)}
						<!-- Подсказка на самой кнопке, а не на коробке внутри: та была
						     уже кнопки, и по её краю подсказка не появлялась -->
						<button
							class="tray_app_bottom_link"
							class:tray_app_bottom_link_on={view === tab.id}
							aria-label={tab.label}
							aria-current={view === tab.id ? 'page' : undefined}
							data-tooltip={tab.label}
							data-tooltip-position="top"
							onclick={() => (view = tab.id)}
						>
							<div class="tray_app_bottom_link_back">
								<div class={tab.ico}></div>
							</div>
						</button>
					{/each}
				</div>

				<div class="tray_app_bottom_use">
					<button
						class="tray_app_bottom_link"
						aria-label={i18n.t.tray_settings}
						data-tooltip={i18n.t.tray_settings}
						data-tooltip-position="top"
						onclick={openSettings}
					>
						<div class="tray_app_bottom_link_back">
							<div class="tray_app_bottom_link_back_ico_cog"></div>
						</div>
					</button>

					<!--
						Выход удержанием: короткое нажатие ничего не делает, и об
						этом говорит подсказка. Панель у самого края экрана, мимо
						часов и значков, — промахнуться сюда легче обычного.
					-->
					<HoldIcon
						button={armed ? 'tray_app_bottom_link tray_app_bottom_link_armed' : 'tray_app_bottom_link'}
						ico="tray_app_bottom_link_back_ico_quit"
						label={armed ? i18n.t.win_quit_again : i18n.t.win_quit}
						holdLabel={i18n.t.win_quit_hold}
						onclick={askQuit}
						onhold={quit}
					/>
				</div>
			</div>
		</div>
	</div>
</div>
