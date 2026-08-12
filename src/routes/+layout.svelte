<script lang="ts">
	/*
	 * Общая обвязка всех трёх окон.
	 *
	 * Стили — исходные файлы оформления, подключённые статическими импортами:
	 * Vite собирает их в один бандл. Никаких динамических import() и подмены
	 * <link> в рантайме — именно на этом ломались темы в прошлой сборке.
	 */
	// Первым — сброс браузерного оформления кнопок, чтобы классы оформления
	// перебивали его, а не наоборот.
	import '$lib/App/style/main/controls.css';

	// Значения темы — до правил, которые на них ссылаются.
	import '$lib/App/style/main/tokens.css';

	import '$lib/App/style/main/main.css';
	import '$lib/App/style/main/default.css';
	import '$lib/App/style/main/titlebar.css';
	import '$lib/App/style/main/notifications.css';
	import '$lib/App/style/main/island.css';
	import '$lib/App/style/main/view-switch.css';
	import '$lib/App/style/main/media.css';
	import '$lib/App/style/main/window-fx.css';
	import '$lib/App/style/main/shell.css';
	import '$lib/App/style/main/preloader.css';
	import '$lib/App/style/main/select.css';
	import '$lib/App/style/custom/home.css';
	import '$lib/App/style/custom/welcome.css';
	import '$lib/App/style/custom/start.css';
	import '$lib/App/style/custom/tray.css';

	// Иконки: форма из SVG маской, цвет из токена — одно правило на обе темы.
	import '$lib/App/style/main/icons.css';

	// Две многоцветные иконки, которые маской не покрасить.
	import '$lib/App/style/themes/icons-theme.css';

	import { invoke } from '@tauri-apps/api/core';
	import { listen } from '@tauri-apps/api/event';

	import Tooltip from '$lib/components/ui/Tooltip.svelte';
	import { appIcon } from '$lib/state/appIcon.svelte';
	import { i18n } from '$lib/state/i18n.svelte';
	import { theme } from '$lib/state/theme.svelte';
	import { setupWindowFx } from '$lib/state/windowFx';

	let { children } = $props();

	$effect(() => theme.restore());

	$effect(() => setupWindowFx());




	// Знак приложения нужен всем окнам: он стоит в полосе заголовка и на
	// экране загрузки, а меняется из настроек главного окна
	$effect(() => {
		appIcon.load();
		return appIcon.attach();
	});

	/*
	 * Настройки меняются в главном окне, а тема и язык нужны всем трём.
	 * Ядро рассылает `app:labels-updated`, и каждое окно перечитывает выбор.
	 * Раньше на это же место приходил `location.reload()` для каждого окна —
	 * они перезагружались целиком и теряли своё состояние.
	 */
	$effect(() => {
		const sub = listen('app:labels-updated', () => {
			theme.restore();
			i18n.restore();
		});

		return () => {
			sub.then((unlisten) => unlisten());
		};
	});

	/*
	 * Слова для значка корзины в трее.
	 *
	 * Их рисует система, а не разметка, поэтому до словаря они сами не
	 * дотягиваются — приходится отдавать их ядру строками. Но берутся они
	 * отсюда же, откуда все прочие: словарь один, второй копии у переводов
	 * быть не должно.
	 *
	 * Отправка привязана к самому словарю: `i18n.t` меняется при смене языка,
	 * и обработчик повторяется сам. Ядро сравнивает присланное с тем, что уже
	 * стоит, и на одинаковое не отвечает ничем — поэтому повторы от трёх окон
	 * ничего не стоят.
	 */
	$effect(() => {
		const labels = {
			empty: i18n.t.bin_empty_hint,
			title: i18n.t.appTrayText_1_6,
			items: i18n.t.bin_items,
			open: i18n.t.bin_click_open,
			clear: i18n.t.bin_click_empty,
			settings: i18n.t.bin_menu_settings,
			// Обозначения размера: подсказку значка собирает ядро, и без них
			// она говорила «1,5 ГБ» на любом языке
			units: [i18n.t.size_b, i18n.t.size_kb, i18n.t.size_mb, i18n.t.size_gb, i18n.t.size_tb]
		};

		invoke('bin_set_labels', { labels }).catch(() => undefined);
	});
</script>

{@render children()}

<!-- Последней и вне содержимого окна: подсказка не должна попадать ни в один
     прокручиваемый список — он обрезал бы её по своему краю -->
<Tooltip />
