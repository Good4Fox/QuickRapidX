<script lang="ts">
	/*
	 * Титлбар главного окна. Разметка и классы перенесены из исходного
	 * приложения — оформление целиком в default.css и файлах тем.
	 */
	import { invoke } from '@tauri-apps/api/core';
	import { getCurrentWindow } from '@tauri-apps/api/window';

	import DynamicIsland from '$lib/components/DynamicIsland.svelte';
	import HoldIcon from '$lib/components/ui/HoldIcon.svelte';
	import NotificationPanel from '$lib/components/Notifications/NotificationPanel.svelte';
	import { APP_TAG, APP_TAG_TOOLTIP } from '$lib/data/info';
	import { setAppFrame } from '$lib/state/appFrame';
	import { i18n } from '$lib/state/i18n.svelte';
	import { notifications } from '$lib/state/notifications.svelte';
	import { windowPrefs } from '$lib/state/windowPrefs.svelte';

	type Props = {
		/** Открыть/закрыть настройки. */
		onsettings: () => void;
		/** Открыть/закрыть консоль. */
		/** Показывать ли блок доп. кнопок — только на домашней и в настройках. */
		showTools: boolean;
	};

	let { onsettings, showTools }: Props = $props();

	const appWindow = getCurrentWindow();

	let maximized = $state(false);
	let panelOpen = $state(false);

	$effect(() => {
		let alive = true;

		notifications.restore();

		const sync = async () => {
			const value = await appWindow.isMaximized();
			if (!alive) return;
			maximized = value;
			setAppFrame(!value);
		};

		sync();
		const off = appWindow.onResized(sync);

		/*
		 * Настройка закрытия нужна самой подписи крестика.
		 *
		 * При выключенном «сворачивать в трей» нажатие выходит сразу, и обещать
		 * «свернуть в трей» было бы прямой неправдой. Спрашиваем у ядра, а не
		 * держим догадку: реестр могли поправить и снаружи.
		 */
		windowPrefs.load();

		return () => {
			alive = false;
			off.then((unlisten) => unlisten());
		};
	});

	function minimize() {
		appWindow.minimize();
	}

	function toggleMaximize() {
		if (maximized) appWindow.unmaximize();
		else appWindow.maximize();
	}

	/**
	 * Крестик закрывает окно, а решает ядро — прятать или выходить.
	 *
	 * Раньше здесь стояла своя развилка по ключу `AppTrayOnioUser` в
	 * localStorage. Ключ остался от прошлой версии: его читало это место и
	 * больше никто, а **писать** его перестали вовсе — настройка пишет
	 * `close_to_tray` в настройки ядра. Проверка всегда давала ложь, и крестик
	 * выходил из программы, что бы ни стояло в настройках.
	 *
	 * Теперь решение одно и в одном месте: обработчик закрытия окна в ядре. Он
	 * же перед уходом сохраняет положение окна — чего развилка не делала.
	 */
	function close() {
		appWindow.close();
	}

	/** Удержание крестика — выход насовсем, минуя трей. */
	function quit() {
		invoke('app_quit');
	}
</script>

{#if panelOpen}
	<NotificationPanel onclose={() => (panelOpen = false)} />
{/if}

<div data-tauri-drag-region class="titlebar">
	<div data-tauri-drag-region class="titlebar_left">
		<div class="titlebar_left_logo">
			<div class="titlebar_left_logo_ico"></div>
			<div class="titlebar_left_logo_text">{i18n.t.home_app_text_1}</div>
			<div class="titlebar_left_logo_beta">
				<div
					class="titlebar_left_logo_beta_text"
					data-tooltip={APP_TAG_TOOLTIP}
					data-tooltip-position="right"
				>
					{i18n.t.app_stage || APP_TAG}
				</div>
			</div>
		</div>
	</div>

	<div data-tauri-drag-region class="titlebar_center">
		<DynamicIsland />
	</div>

	<div data-tauri-drag-region class="titlebar_right">
		{#if showTools}
			<div data-tauri-drag-region class="titlebar_right_dops">
				<button
					class="titlebar_right_dops_notification"
					aria-label={i18n.t.note_mode}
					onclick={(event) => {
						panelOpen = !panelOpen;
						event.stopPropagation();
					}}
				>
					<div class="titlebar_right_dops_notification_ico"></div>
					{#if notifications.unread > 0}
						<span class="titlebar_notification_badge">
							{notifications.unread > 9 ? '9+' : notifications.unread}
						</span>
					{/if}
				</button>

				<button
					class="titlebar_right_dops_settings"
					onclick={onsettings}
					aria-label={i18n.t.win_settings}
				>
					<div class="titlebar_right_dops_settings_img"></div>
				</button>
			</div>
		{/if}

		<div data-tauri-drag-region class="titlebar_right_use">
			<button
				class="titlebar_right_button_minimize"
				onclick={minimize}
				aria-label={i18n.t.win_x_min}
			>
				<div class="titlebar_right_button_minimize_img"></div>
			</button>
			<!-- Подпись меняется вместе со значком: развёрнутое окно кнопка
			     возвращает к прежнему размеру, а не разворачивает ещё раз -->
			<button
				class="titlebar_right_button_maximize"
				onclick={toggleMaximize}
				aria-label={maximized ? i18n.t.win_x_restore : i18n.t.win_x_max}
			>
				<div
					class={maximized
						? 'titlebar_right_button_maximize_img_min'
						: 'titlebar_right_button_maximize_img'}
				></div>
			</button>
			<!-- Нажатие прячет в трей, удержание выходит: одна кнопка на два
			     действия, потому что двух одинаковых крестиков рядом человек
			     не различит -->
			<HoldIcon
				button="titlebar_right_button_close"
				ico="titlebar_right_button_close_img"
				label={windowPrefs.closeToTray ? i18n.t.win_x_hide : i18n.t.win_x_close}
				holdLabel={i18n.t.win_x_quit}
				onclick={close}
				onhold={quit}
			/>
		</div>
	</div>
</div>
