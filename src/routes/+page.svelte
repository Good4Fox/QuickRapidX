<script lang="ts">
	/*
	 * Главное окно (1280×730) — размеры взяты из исходного приложения.
	 *
	 * Порядок показа тот же, что и раньше: окно-заставка 350×450, затем это
	 * окно с тем же экраном загрузки уже во всю ширину, и только потом
	 * содержимое. Разница в том, что момент перехода задаёт ядро событием
	 * `boot:ready`, а не таймер, поставленный наугад.
	 */
	import { invoke } from '@tauri-apps/api/core';
	import { emit, listen } from '@tauri-apps/api/event';

	import { windowPrefs } from '$lib/state/windowPrefs.svelte';

	import CreatingSettingsInteraction from '$lib/components/CreatingSettingsInteraction.svelte';
	import Home from '$lib/components/Home.svelte';
	import NotificationToasts from '$lib/components/Notifications/NotificationToasts.svelte';
	import Preloader from '$lib/components/Preloader.svelte';
	import Settings from '$lib/components/Settings/Settings.svelte';
	import ViewSwitch from '$lib/components/ViewSwitch.svelte';
	import Titlebar from '$lib/components/Titlebar.svelte';
	import { boot } from '$lib/state/boot.svelte';
	import { notifications } from '$lib/state/notifications.svelte';
	import { i18n } from '$lib/state/i18n.svelte';
	import { welcome } from '$lib/state/welcome.svelte';
	import { setupKeybindings } from '$lib/state/keybindings';
	import { startData } from '$lib/state/startData.svelte';

	/** Сколько держать большой экран загрузки после готовности ядра. */
	const HOLD = 1400;

	type View = 'home' | 'settings';

	let loading = $state(true);
	let view = $state<View>('home');

	$effect(() => {
		welcome.restore();
		i18n.restore();
		const detach = boot.attach();

		// Обычно загрузку запускает окно-заставка. Дублируем вызов отсюда,
		// чтобы её отказ не оставил приложение висеть на экране загрузки:
		// в ядре стоит защита от повторного запуска, лишним не будет.
		invoke('boot_start').catch((e) => console.error('boot_start failed', e));

		const releaseKeys = setupKeybindings();

		return () => {
			detach.then((off) => off());
			releaseKeys();
		};
	});

	// Окно становится видимым как раз по `boot:ready`; отсюда и отсчитываем
	// показ большого экрана загрузки.
	// Сбой загрузки — самое важное, о чём надо сказать сразу
	$effect(() => {
		if (!boot.error) return;

		notifications.push({
			level: 'error',
			title: i18n.t.boot_failed_title,
			text: boot.error.message
		});
	});

	$effect(() => {
		if (!boot.ready) return;

		// Собираем данные стартовой, пока держится экран загрузки: иначе
		// плитки заполняются уже на глазах у пользователя.
		startData.load();

		const timer = setTimeout(() => {
			loading = false;
		}, HOLD);

		return () => clearTimeout(timer);
	});

	/*
	 * Просьба открыть настройки со стороны.
	 *
	 * Приходит из панели трея и из меню значка корзины: настройки живут здесь,
	 * а не там, и вести туда, где их нет, было бы обманом. Событие общее —
	 * его шлёт и другое окно, и ядро.
	 */
	$effect(() => {
		const asked = listen<string | null>('app:open-settings', (event) => {
			view = 'settings';

			const anchor = event.payload;

			if (!anchor) return;

			/*
			 * Просьбу прокрутить пересылаем, а не исполняем здесь.
			 *
			 * Нужного блока в этот миг ещё нет на странице: настройки только
			 * начинают строиться, да ещё и въезжают переходом. Ждём, пока он
			 * кончится, и отдаём просьбу самим настройкам — они к этому времени
			 * уже слушают.
			 */
			setTimeout(() => {
				emit('app:settings-anchor', anchor).catch(() => undefined);
			}, 360);
		});

		return () => {
			asked.then((stop) => stop());
		};
	});

	/*
	 * Просьба открыть наборы — из панели трея, когда их ещё нет.
	 *
	 * Раздел с наборами лежит внутри «Приложений», поэтому сначала уводим сюда,
	 * а дальше просьбу подхватывает сам раздел.
	 */
	$effect(() => {
		const asked = listen('app:open-groups', () => (view = 'home'));

		return () => {
			asked.then((stop) => stop());
		};
	});

	/*
	 * Окно спрятали — возвращаемся на главную.
	 *
	 * Крестиком прячут из любого места, и показывать окно потом на настройках,
	 * куда заходили один раз, — не то, чего ждут. Кому нужно наоборот, тот
	 * включает «возвращаться на тот же раздел».
	 *
	 * Сброс на скрытии, а не на показе: показ бывает и по просьбе открыть
	 * настройки — из панели трея, из меню значка корзины, — и сброс на показе
	 * гасил бы её следом.
	 */
	$effect(() => {
		windowPrefs.load();

		const hidden = listen('app:main-hidden', () => {
			if (!windowPrefs.rememberView) view = 'home';
		});

		return () => {
			hidden.then((stop) => stop());
		};
	});

	/** Повторное нажатие на кнопку раздела возвращает на домашний экран. */
	function toggle(target: View) {
		view = view === target ? 'home' : target;
	}
</script>

<div class="mine_home">
	{#if loading}
		<Preloader />
		{#if boot.error}
			<!-- Загрузка встала: без этого окно молча висело бы на заставке -->
			<div class="boot_failed">{boot.error.message}</div>
		{/if}
	{:else if welcome.show}
		<CreatingSettingsInteraction ondone={() => welcome.done()} />
	{:else}
		<Titlebar
			showTools={view === 'home' || view === 'settings'}
			onsettings={() => toggle('settings')}
		/>

		<NotificationToasts />

		<div class="home">
			<!-- Настройки приезжают снизу: это надстройка над главной, а не
			     соседний раздел, и движение по другой оси это показывает -->
			<ViewSwitch {view} axis="y" shift={22} direction={view === 'home' ? -1 : 1}>
				{#if view === 'home'}
					<Home />
				{:else if view === 'settings'}
					<Settings />
				{/if}
			</ViewSwitch>
		</div>
	{/if}
</div>

<style>
	.boot_failed {
		position: absolute;
		right: 0;
		bottom: 24px;
		left: 0;
		font: 0.8125rem 'Rubik', sans-serif;
		color: rgb(248, 80, 88);
		text-align: center;
	}
</style>
