<script lang="ts">
	/*
	 * Наборы ярлыков.
	 *
	 * Здесь их собирают, а открывают из панели трея — там же, где человек их и
	 * ищет. Поэтому экран устроен как список наборов, а не как один набор с
	 * переключателем: видно сразу всё, что собрано, и сколько в чём лежит.
	 *
	 * Открыть набор можно и отсюда: пробовать собранное, каждый раз лазая в
	 * трей, — лишняя работа.
	 */
	import { flip } from 'svelte/animate';
	import { cubicOut } from 'svelte/easing';
	import { fly } from 'svelte/transition';

	import { invoke } from '@tauri-apps/api/core';
	import { getCurrentWebview } from '@tauri-apps/api/webview';
	import { open } from '@tauri-apps/plugin-dialog';
	import { startDrag } from '@crabnebula/tauri-plugin-drag';

	import AppIcon from '$lib/components/ui/AppIcon.svelte';
	import HoldButton from '$lib/components/ui/HoldButton.svelte';
	import IsolationSettings from '$lib/components/Home/IsolationSettings.svelte';
	import { i18n } from '$lib/state/i18n.svelte';
	import { inventory } from '$lib/state/inventory.svelte';
	import { notifyError, notifySuccess } from '$lib/state/notifications.svelte';
	import {
		blankGroup,
		groups,
		nestedFor,
		shortcutFor,
		type Group,
		type Shortcut
	} from '$lib/state/groups.svelte';


	/** Поиск по найденному на машине — оттуда добавлять быстрее всего. */
	let query = $state('');
	let picking = $state(false);
	let busy = $state(false);

	/** Файлы тащат над окном прямо сейчас. */
	let hovering = $state(false);

	$effect(() => {
		groups.ensure();

		/*
		 * Esc — выход из набора.
		 *
		 * Кнопка «Назад» стоит в заголовке, и в длинном наборе до неё далеко
		 * тянуться. Клавиша под рукой всегда, а привычка выходить ею есть у
		 * всех: так закрывается любое окно поверх.
		 */
		const keys = (event: KeyboardEvent) => {
			if (event.key !== 'Escape' || !groups.open) return;

			// Из поля Esc должен убирать правку, а не уводить с экрана
			const inside = document.activeElement;

			if (inside instanceof HTMLInputElement || inside instanceof HTMLTextAreaElement) {
				inside.blur();
				return;
			}

			go('');
		};

		window.addEventListener('keydown', keys);

		return () => window.removeEventListener('keydown', keys);
	});

	/*
	 * Перетаскивание файлов в окно.
	 *
	 * Самый быстрый способ собрать набор: выделить в проводнике хоть десяток
	 * ярлыков и бросить сюда. Открыт набор — попадут в него; открыт список —
	 * из брошенного соберётся новый набор.
	 *
	 * Событие приходит от окна целиком, а не от элемента: перетаскивание файлов
	 * идёт мимо разметки, на уровне системы.
	 */
	$effect(() => {
		const watching = getCurrentWebview().onDragDropEvent((event) => {
			const payload = event.payload;

			/*
			 * Добавляем только по `drop` — когда кнопку отпустили.
			 *
			 * Событий приходит несколько: `enter` при входе файлов в окно,
			 * `over` при движении, `leave` при выходе. Пути лежат уже в первом
			 * из них, и раньше сюда попадало всё, кроме `over` и `leave`, —
			 * значит, записи добавлялись дважды: один раз на входе курсора,
			 * второй по отпусканию.
			 */
			if (payload.type === 'drop') {
				const onto = dropOn;

				hovering = false;
				dropOn = '';
				accept(payload.paths ?? [], onto);
				return;
			}

			if (payload.type === 'leave') {
				hovering = false;
				dropOn = '';
				return;
			}

			hovering = true;
			dropOn = cardUnder(payload.position);
		});

		return () => {
			watching.then((stop) => stop());
		};
	});

	/** Набор, на карточку которого сейчас метят файлами. */
	let dropOn = $state('');

	/*
	 * Только что добавленные записи — их подсвечивает появление.
	 *
	 * Отмечаются поимённо, а не переходом на весь список: иначе при каждом
	 * открытии набора всё содержимое влетало бы заново, и добавление ничем не
	 * отличалось бы от обычного показа.
	 */
	let fresh = $state(new Set<string>());

	function markFresh(ids: string[]) {
		fresh = new Set([...fresh, ...ids]);

		setTimeout(() => {
			fresh = new Set([...fresh].filter((id) => !ids.includes(id)));
		}, 900);
	}

	/*
	 * Какая карточка под курсором.
	 *
	 * Событие переноса приходит от системы и несёт положение в настоящих
	 * пикселях экрана, а разметка живёт в своих — отсюда деление на плотность.
	 * Дальше спрашиваем у страницы, что в этой точке, и поднимаемся до
	 * карточки: попасть можно и в значок, и в подпись внутри неё.
	 */
	function cardUnder(position?: { x: number; y: number }): string {
		if (!position) return '';

		const ratio = window.devicePixelRatio || 1;
		const node = document.elementFromPoint(position.x / ratio, position.y / ratio);
		const card = node?.closest('[data-group]');

		return card?.getAttribute('data-group') ?? '';
	}

	async function accept(paths: string[], onto = '') {
		const files = paths.filter((path) => path.trim().length > 0);

		if (files.length === 0) return;

		const target = groups.list.find((group) => group.id === (onto || groups.open));

		if (target) {
			// Одно и то же дважды не кладём: бросить ту же папку второй раз —
			// обычная оплошность, а не просьба завести дубль
			const known = new Set(target.items.map((item) => item.path.toLowerCase()));
			const unseen = files.filter((path) => !known.has(path.toLowerCase()));

			if (unseen.length === 0) {
				notifySuccess(target.name, i18n.t.grp_already);
				return;
			}

			const added = unseen.map(shortcutFor);

			await patch(target, { items: [...target.items, ...added] });
			markFresh(added.map((item) => item.id));

			notifySuccess(target.name, `${i18n.t.grp_added}: ${added.length}`);
			return;
		}

		// Списка не было открыто — брошенное само становится набором
		const made = blankGroup(i18n.t.grp_new_name);
		made.items = files.map(shortcutFor);

		await groups.save(made);
		go(made.id);

		notifySuccess(i18n.t.grp_new, `${i18n.t.grp_added}: ${files.length}`);
	}

	const current = $derived(groups.list.find((group) => group.id === groups.open) ?? null);

	const found = $derived.by(() => {
		const needle = query.trim().toLowerCase();
		if (!needle) return [];

		return inventory.findings
			.filter((finding) => finding.exe && finding.name.toLowerCase().includes(needle))
			.slice(0, 8);
	});

	async function create() {
		const group = blankGroup(i18n.t.grp_new_name);

		await groups.save(group);
		go(group.id);
	}

	async function patch(group: Group, next: Partial<Group>) {
		try {
			await groups.save({ ...group, ...next });
		} catch (e) {
			notifyError(i18n.t.notification_text_4, String(e));
		}
	}

	function change(group: Group, id: string, next: Partial<Shortcut>) {
		patch(group, {
			items: group.items.map((item) => (item.id === id ? { ...item, ...next } : item))
		});
	}

	function drop(group: Group, id: string) {
		patch(group, { items: group.items.filter((item) => item.id !== id) });
	}

	/** Перестановка соседей: порядок в наборе — это порядок запуска. */
	function move(group: Group, index: number, step: number) {
		const next = [...group.items];
		const to = index + step;

		if (to < 0 || to >= next.length) return;

		[next[index], next[to]] = [next[to], next[index]];
		patch(group, { items: next });
	}

	/*
	 * Перетаскивание записей внутри набора.
	 *
	 * Не на встроенном перетаскивании разметки, а на событиях указателя.
	 * Причина простая: у окна включён приём файлов извне, и WebView2 забирает
	 * себе всю работу с переносом — свои `dragstart` и `drop` внутри страницы
	 * после этого не приходят вовсе. Указатель же остаётся обычным указателем.
	 *
	 * Тащат за отдельную ручку слева, а не за всю строку: иначе нельзя было бы
	 * ни выделить название в поле, ни просто ткнуть в него.
	 */
	let held = $state(-1);

	/*
	 * Порядок на время перетаскивания.
	 *
	 * Записи меняются местами прямо под курсором, а не после отпускания: так же
	 * ведут себя значки на рабочем столе. Плавность даёт та же анимация
	 * переезда, что и у обычной перестановки, — соседи не прыгают, а
	 * расступаются. Сохраняется всё один раз, по отпусканию: писать файл на
	 * каждое движение мыши незачем.
	 */
	let preview = $state<Shortcut[] | null>(null);

	/*
	 * Переход между списком и правкой набора.
	 *
	 * Приходящее выезжает с той стороны, в которую мы идём, уходящее уходит в
	 * противоположную. Обе стороны живут в одной клетке сетки — так они не
	 * отталкивают друг друга по высоте, пока идёт смена.
	 *
	 * Переход висит на общей обёртке, и это не косметика, а единственное
	 * рабочее место. В Svelte 5 переход по умолчанию местный: он играет,
	 * только если его собственный блок пережил смену, а не был создан вместе с
	 * блоком сверху. Пока переходы стояли на самих сторонах внутри
	 * `{#key groups.open}`, каждая смена ключа создавала их блок заново — и не
	 * играл ни один. Проверено по исходникам Svelte 5.56: в `transitions.js`
	 * местный вход требует признака `REACTION_RAN` у блока над собой, а у
	 * только что созданного его нет.
	 *
	 * На обёртке всё сходится: её блок — сам `{#key}`, он смену переживает.
	 * Заодно переход молчит, когда на раздел заходят снаружи: там блок
	 * создаётся вместе со всем разделом, и своё движение уже играет
	 * переключатель разделов. Пометка `|global` дала бы ровно это движение
	 * вторым слоем.
	 */
	const still =
		typeof window !== 'undefined' &&
		window.matchMedia('(prefers-reduced-motion: reduce)').matches;

	/** Куда идёт переход: 1 — вглубь, в правку; -1 — наружу, к списку. */
	let heading = $state(1);

	/**
	 * Открывает набор или возвращает к списку.
	 *
	 * Направление запоминается до смены, а не выводится из неё: к моменту, когда
	 * переход спрашивает свои настройки, открытый набор уже новый, и по нему
	 * уходящая сторона получила бы направление приходящей — обе поехали бы в
	 * одну сторону.
	 */
	function go(id: string) {
		heading = id ? 1 : -1;
		groups.open = id;
	}

	function enter(node: Element) {
		return fly(node, {
			x: heading * 24,
			duration: still ? 0 : 240,
			easing: cubicOut
		});
	}

	function leave(node: Element) {
		return fly(node, {
			x: heading * -24,
			duration: still ? 0 : 200,
			easing: cubicOut
		});
	}

	/** Что показывать: предпросмотр во время переноса, иначе сам набор. */
	const rows = $derived(preview ?? current?.items ?? []);

	function grab(event: PointerEvent, group: Group, index: number) {
		if (event.button !== 0) return;

		event.preventDefault();

		held = index;
		preview = [...group.items];

		const handle = event.currentTarget as HTMLElement;

		// Захват указателя: иначе движение за пределами ручки перестаёт
		// приходить, и перенос обрывается на полпути
		handle.setPointerCapture?.(event.pointerId);

		let frame = 0;
		let pointerY = event.clientY;

		/*
		 * Куда встанет строка — считается по серединам соседей, а не по тому,
		 * что оказалось под курсором.
		 *
		 * Прежний способ и давал дёрганье: после обмена перетаскиваемая строка
		 * уезжала под курсор, тот снова «попадал» в неё, и она прыгала обратно.
		 * Счёт же по серединам растёт вместе с курсором строго в одну сторону —
		 * качаться тут нечему.
		 */
		const compute = () => {
			frame = 0;

			if (!preview) return;

			const nodes = Array.from(
				document.querySelectorAll<HTMLElement>('.qrx_grp_item[data-row]')
			);

			if (nodes.length === 0) return;

			/*
			 * Место считается среди **остальных** строк, без перетаскиваемой.
			 * Она сама уезжает под курсор, и учитывать её значило бы гоняться
			 * за собственным хвостом.
			 */
			const others = nodes.filter((_, at) => at !== held);

			let to = 0;

			for (const node of others) {
				const box = node.getBoundingClientRect();

				if (pointerY > box.top + box.height / 2) to += 1;
			}

			if (to === held) return;

			const next = [...preview];
			const [taken] = next.splice(held, 1);

			next.splice(to, 0, taken);

			preview = next;
			held = to;
		};

		const move = (moved: PointerEvent) => {
			pointerY = moved.clientY;

			// Считаем раз на кадр: событий указателя приходит куда больше, и
			// пересчитывать раскладку на каждое — лишняя работа
			if (frame === 0) frame = requestAnimationFrame(compute);
		};

		const done = () => {
			cancelAnimationFrame(frame);

			window.removeEventListener('pointermove', move);
			window.removeEventListener('pointerup', done);
			window.removeEventListener('pointercancel', done);

			handle.releasePointerCapture?.(event.pointerId);

			place(group);
		};

		window.addEventListener('pointermove', move);
		window.addEventListener('pointerup', done);
		window.addEventListener('pointercancel', done);
	}

	function place(group: Group) {
		const next = preview;

		held = -1;
		preview = null;

		if (!next) return;

		// Порядок мог и не измениться — тогда и записывать нечего
		const same = next.every((item, at) => item.id === group.items[at]?.id);

		if (!same) patch(group, { items: next });
	}

	async function addFile(group: Group) {
		const picked = await open({
			multiple: true,
			title: i18n.t.grp_add_app,
			filters: [{ name: i18n.t.grp_kind_app, extensions: ['exe', 'lnk', 'bat', 'cmd', 'url'] }]
		});

		const list = Array.isArray(picked) ? picked : typeof picked === 'string' ? [picked] : [];

		if (list.length === 0) return;

		const added = list.map(shortcutFor);

		patch(group, { items: [...group.items, ...added] });
		markFresh(added.map((item) => item.id));
	}

	async function addFolder(group: Group) {
		const picked = await open({ directory: true, multiple: false, title: i18n.t.grp_add_folder });

		if (typeof picked !== 'string') return;

		const added = shortcutFor(picked);

		patch(group, { items: [...group.items, added] });
		markFresh([added.id]);
	}

	/*
	 * Свой значок записи.
	 *
	 * Нужен там, где своего значка нет вовсе: у папок, у ссылок на страницы, у
	 * сценариев. Без него на их месте выводятся две буквы названия, и ряд
	 * значков рассыпается — глаз ищет картинку, а находит текст.
	 *
	 * Берём из любого файла со значком, не только из картинки: выбрать `.exe` и
	 * позаимствовать его значок — самый частый случай.
	 */
	async function pickItemIcon(group: Group, id: string) {
		const picked = await open({
			multiple: false,
			title: i18n.t.grp_item_icon,
			filters: [{ name: i18n.t.bin_icon_kind, extensions: ['ico', 'png', 'exe', 'dll', 'lnk'] }]
		});

		if (typeof picked !== 'string') return;

		change(group, id, { icon: picked });
	}

	async function pickIcon(group: Group) {
		const picked = await open({
			multiple: false,
			title: i18n.t.grp_icon,
			filters: [{ name: i18n.t.grp_kind_app, extensions: ['exe', 'lnk', 'ico'] }]
		});

		if (typeof picked === 'string') patch(group, { icon: picked });
	}

	async function launchAll(group: Group) {
		if (busy) return;
		busy = true;

		const result = await groups.launchAll(group.id);

		busy = false;

		if (result.ok) notifySuccess(group.name, `${i18n.t.grp_opened}: ${result.count}`);
		else notifyError(group.name, result.message);
	}

	async function launchOne(group: Group, item: Shortcut) {
		const result = await groups.launch(group.id, item.id);

		if (!result.ok) notifyError(item.name, result.message);
	}

	/*
	 * Закрепление на панели задач.
	 *
	 * Сделать это за человека нельзя: Windows закрыла глагол `taskbarpin` для
	 * программ, а обходные пути через папку закреплённого и реестр ломаются от
	 * обновления к обновлению. У AppGroup закрепления тоже нет — их
	 * руководство предлагает перетащить значок либо закрепить его из
	 * проводника.
	 *
	 * Второй путь короче, его и берём: ядро готовит ярлык и открывает папку с
	 * ним выделенным. Остаётся правый клик по нему.
	 */
	async function shortcut(group: Group) {
		try {
			await invoke<string>('group_pin', { id: group.id });

			notifySuccess(i18n.t.grp_pin, i18n.t.grp_pinned);
		} catch (e) {
			notifyError(i18n.t.notification_text_4, String(e));
		}
	}

	/*
	 * Вытаскивание набора из окна.
	 *
	 * То, ради чего кнопка «закрепить» вообще не нужна: тащишь плитку набора и
	 * бросаешь на панель задач или рабочий стол — Windows закрепляет сама, как
	 * любой другой ярлык. Файл готовится в этот же миг, до начала переноса:
	 * переносят именно его.
	 *
	 * Перенос начинается по dragstart самой плитки и не всплывает выше — иначе
	 * список перехватил бы жест на перестановку. У AppGroup для этого заведён
	 * отдельный признак, у нас хватает остановки всплытия.
	 */
	/** Готовые файлы ярлыков по наборам: путь к .lnk и картинка к нему. */
	let bundles = $state<Record<string, { file: string; icon: string }>>({});

	// Пути спрашиваются заранее, при показе списка. В момент захвата мышью
	// ждать нечего: перенос должен начаться сразу, иначе система решит, что
	// это обычное нажатие, — и вместо файла уедет картинка из разметки
	$effect(() => {
		for (const group of groups.list) {
			if (bundles[group.id]) continue;

			invoke<{ file: string; icon: string }>('group_shortcut_file', { id: group.id })
				.then((bundle) => (bundles = { ...bundles, [group.id]: bundle }))
				.catch(() => undefined);
		}
	});

	async function dragOut(event: MouseEvent, group: Group) {
		// Левая кнопка и только она: правой вызывают меню
		if (event.button !== 0) return;

		/*
		 * Свой перенос движка отменяем всегда и первым делом.
		 *
		 * Раньше отмена стояла после проверки готовности ярлыка, и стоило ей не
		 * пройти — движок заводил свой перенос картинки. На панель задач уезжал
		 * значок в виде картинки вместо файла, причём молча: со стороны это
		 * выглядело как «перенос просто не работает».
		 */
		event.stopPropagation();
		event.preventDefault();

		dragging = group.id;

		try {
			// Обычно ярлык готов заранее — его просят при показе списка. Но если
			// набор только что создали, ждать неоткуда: спрашиваем на месте
			const bundle =
				bundles[group.id] ??
				(await invoke<{ file: string; icon: string }>('group_shortcut_file', {
					id: group.id
				}));

			bundles = { ...bundles, [group.id]: bundle };

			await startDrag({ item: [bundle.file], icon: bundle.icon }, () => (dragging = ''));
		} catch (e) {
			dragging = '';
			notifyError(i18n.t.notification_text_4, String(e));
		}
	}

	/** Какой набор сейчас тащат. */
	let dragging = $state('');

	/*
	 * Закрепление в меню «Пуск».
	 *
	 * Единственное закрепление, которое система разрешает сделать за человека:
	 * у ярлыка есть такое действие, и его можно вызвать. Закрепления на панели
	 * задач среди действий нет вовсе — Windows убрала его для программ.
	 */
	async function pinStart(group: Group) {
		try {
			await invoke('group_pin_start', { id: group.id });
			notifySuccess(group.name, i18n.t.grp_start_done);
		} catch (e) {
			notifyError(i18n.t.notification_text_4, String(e));
		}
	}

	async function duplicate(group: Group) {
		try {
			// Ядро отдаёт номер копии — по нему её карточку и подсвечиваем.
			// Копия встаёт в конец списка, и без этого её появление легко
			// пропустить: список длинный, а изменилось в нём одно место
			const made = await invoke<string>('group_duplicate', {
				id: group.id,
				suffix: i18n.t.grp_copy_suffix
			});

			await groups.reload();
			markFresh([made]);
		} catch (e) {
			notifyError(i18n.t.notification_text_4, String(e));
		}
	}

	/** Вкладывает в открытый набор другой набор. */
	function nest(host: Group, child: Group) {
		const added = nestedFor(child);

		patch(host, { items: [...host.items, added] });
		markFresh([added.id]);

		nesting = false;
	}

	/** Наборы, которые можно вложить: не сам себя и не уже вложенные. */
	const nestable = $derived.by(() => {
		if (!current) return [];

		const inside = new Set(current.items.map((item) => item.child).filter(Boolean));

		return groups.list.filter((group) => group.id !== current.id && !inside.has(group.id));
	});

	let nesting = $state(false);

	async function remove(group: Group) {
		await groups.remove(group.id);
		go('');
	}

	/** Значок набора: свой, а если не задан — от первой записи. */
	function icon(group: Group): string {
		return group.icon || group.items[0]?.path || '';
	}
</script>

<div class="qrx_found">
	<header class="qrx_found_head">
		<div class="qrx_set_row_text">
			<span class="qrx_set_row_title">{i18n.t.grp_title}</span>
			<span class="qrx_set_row_note">{i18n.t.grp_note}</span>
		</div>

		{#if current}
			<button class="qrx_rec_log" onclick={() => go('')}>{i18n.t.grp_back}</button>
		{:else}
			<button class="qrx_sys_link" onclick={create}>{i18n.t.grp_new}</button>
		{/if}
	</header>

	<!-- Подсказка появляется, только когда файлы уже над окном -->
	{#if hovering}
		<p class="qrx_grp_drop">
			{#if dropOn}
				{i18n.t.grp_drop_onto}
				«{groups.list.find((group) => group.id === dropOn)?.name}»
			{:else if current}
				{i18n.t.grp_drop_here}
			{:else}
				{i18n.t.grp_drop_new}
			{/if}
		</p>
	{/if}

	<!-- Обе стороны лежат в одной клетке сетки: пока идёт смена, они не
	     отталкивают друг друга по высоте, а сменяются на месте.

	     Переход висит на обёртке, а не на самих сторонах: только у неё блок
	     переживает смену ключа, и только тогда местный переход играет -->
	<div class="qrx_grp_swap">
	{#key groups.open}
	<div class="qrx_grp_slide" in:enter out:leave>
	{#if !current}
		<div class="qrx_grp_stage">
		{#if groups.list.length === 0}
			<p class="qrx_rec_task_note">{i18n.t.grp_none}</p>
			<p class="qrx_rec_task_note">{i18n.t.grp_drop_hint}</p>
		{/if}

		<!-- Карточки, а не строки: у набора есть лицо — его значок и то, что
		     внутри. Строкой это не показать, а именно по лицу его и узнают -->
		<div class="qrx_grp_cards">
			{#each groups.list as group (group.id)}
				<!-- Перестановка соседей отдельно от появления новичка: сетка
				     раздвигается плавно, а сама копия влетает на освободившееся
				     место. Одним переходом это не показать -->
				<article
					class="qrx_grp_card"
					class:qrx_grp_card_target={dropOn === group.id}
					class:qrx_grp_card_new={fresh.has(group.id)}
					data-group={group.id}
					animate:flip={{ duration: 260 }}
				>
					<header class="qrx_grp_card_head">
						<!-- Значок берётся мышью и тащится на панель задач: закрепление
						     тогда делает сама Windows, кнопка не нужна -->
						<span
							class="qrx_grp_grab"
							class:qrx_grp_grab_busy={dragging === group.id}
							role="button"
							tabindex="0"
							data-tooltip={i18n.t.grp_drag_out}
							data-tooltip-position="right"
							onmousedown={(event) => dragOut(event, group)}
							ondragstart={(event) => event.preventDefault()}
						>
							<AppIcon path={icon(group)} fallback={group.name} size={56} />
						</span>

						<div class="qrx_found_main">
							<span class="qrx_found_name">{group.name}</span>
							<span class="qrx_rec_task_note">
								{group.items.length}
								{i18n.t.grp_count}
							</span>
						</div>

						<div class="qrx_grp_marks">
							{#if group.tray}
								<span class="qrx_found_tag qrx_found_tag_portable">{i18n.t.grp_in_tray}</span>
							{/if}
							{#if group.panel}
								<span class="qrx_found_tag qrx_found_tag_installed">{i18n.t.grp_in_panel}</span>
							{/if}
						</div>
					</header>

					<!-- Полоска содержимого: видно состав, не открывая набор -->
					{#if group.items.length > 0}
						<div class="qrx_grp_strip">
							{#each group.items.slice(0, 8) as item (item.id)}
								<span
									class="qrx_grp_chip"
									data-tooltip={item.name}
									data-tooltip-position="top"
								>
									<AppIcon path={item.icon || item.path} fallback={item.name} size={40} />
								</span>
							{/each}

							{#if group.items.length > 8}
								<span class="qrx_grp_more">+{group.items.length - 8}</span>
							{/if}
						</div>
					{:else}
						<p class="qrx_rec_task_note">{i18n.t.grp_items_none}</p>
					{/if}

					<footer class="qrx_grp_card_foot">
						<!-- Открывает разом все программы набора — нажатие такого веса
						     не должно случаться от промаха мышью -->
						{#if group.items.length > 0}
							<HoldButton
								compact
								label={i18n.t.grp_launch_all}
								ico="qrx_sys_ico_explorer"
								hint={i18n.t.grp_hold}
								onrun={() => launchAll(group)}
							/>
						{/if}

						<button class="qrx_rec_log" onclick={() => go(group.id)}>
							{i18n.t.cat_edit}
						</button>

						<button
							class="qrx_rec_log"
							data-tooltip={i18n.t.grp_copy_note}
							data-tooltip-position="top"
							onclick={() => duplicate(group)}
						>
							{i18n.t.grp_copy}
						</button>
					</footer>
				</article>
			{/each}
		</div>
		</div>
	{:else}
		<!-- Правка набора. Всё сохраняется сразу: это список, а не анкета -->
		<div class="qrx_form qrx_grp_stage">
			<header class="qrx_form_head">
				<span class="qrx_set_row_title">{current.name}</span>

				<div class="qrx_iso_actions">
					{#if current.items.length > 0}
						<HoldButton
							compact
							label={i18n.t.grp_launch_all}
							ico="qrx_sys_ico_explorer"
							hint={i18n.t.grp_hold}
							onrun={() => launchAll(current)}
						/>
					{/if}

					<button class="qrx_rec_log" onclick={() => shortcut(current)}>
						{i18n.t.grp_pin}
					</button>

					<button class="qrx_rec_log qrx_lib_drop" onclick={() => remove(current)}>
						{i18n.t.cat_delete}
					</button>
				</div>
			</header>

			<label class="qrx_field">
				<span class="qrx_field_label">{i18n.t.grp_name}</span>
				<input
					class="qrx_field_input"
					type="text"
					value={current.name}
					onchange={(event) => patch(current, { name: event.currentTarget.value })}
				/>
			</label>

			<div class="qrx_iso_group">
				<span class="qrx_field_label">{i18n.t.grp_icon}</span>

				<div class="qrx_iso_row">
					<AppIcon path={icon(current)} fallback={current.name} size={40} />
					<span class="qrx_iso_path">{current.icon || i18n.t.grp_icon_auto}</span>
					<button class="qrx_rec_log" onclick={() => pickIcon(current)}>
						{i18n.t.iso_file_pick}
					</button>
					{#if current.icon}
						<button class="qrx_rec_log qrx_lib_drop" onclick={() => patch(current, { icon: '' })}>
							{i18n.t.iso_drop}
						</button>
					{/if}
				</div>
			</div>

			<!-- Где набору быть. Три места, и они не спорят друг с другом:
			     свой значок, общая панель, ярлык наружу -->
			<div class="qrx_iso_group">
				<span class="qrx_field_label">{i18n.t.grp_where}</span>

				<label class="qrx_place_switch">
					<input
						class="qrx_toggle"
						type="checkbox"
						checked={current.tray}
						onchange={(event) => patch(current, { tray: event.currentTarget.checked })}
					/>
					<span class="qrx_set_row_text">
						<span class="qrx_set_row_title">{i18n.t.grp_tray}</span>
						<span class="qrx_rec_task_note">{i18n.t.grp_tray_note}</span>
					</span>
				</label>

				<label class="qrx_place_switch">
					<input
						class="qrx_toggle"
						type="checkbox"
						checked={current.panel}
						onchange={(event) => patch(current, { panel: event.currentTarget.checked })}
					/>
					<span class="qrx_set_row_text">
						<span class="qrx_set_row_title">{i18n.t.grp_panel}</span>
						<span class="qrx_rec_task_note">{i18n.t.grp_panel_note}</span>
					</span>
				</label>

				<label class="qrx_place_switch">
					<input
						class="qrx_toggle"
						type="checkbox"
						checked={current.head}
						onchange={(event) => patch(current, { head: event.currentTarget.checked })}
					/>
					<span class="qrx_set_row_text">
						<span class="qrx_set_row_title">{i18n.t.grp_head}</span>
						<span class="qrx_rec_task_note">{i18n.t.grp_head_note}</span>
					</span>
				</label>

				<label class="qrx_place_switch">
					<input
						class="qrx_toggle"
						type="checkbox"
						checked={current.labels}
						onchange={(event) => patch(current, { labels: event.currentTarget.checked })}
					/>
					<span class="qrx_set_row_text">
						<span class="qrx_set_row_title">{i18n.t.grp_labels}</span>
						<span class="qrx_rec_task_note">{i18n.t.grp_labels_note}</span>
					</span>
				</label>

				<!-- Как ложится окно набора. Здесь, а не в общих настройках: это
				     вид этого набора, и у разных наборов он бывает разным -->
				<label class="qrx_place_switch">
					<input
						class="qrx_toggle"
						type="checkbox"
						checked={current.vertical}
						onchange={(event) => patch(current, { vertical: event.currentTarget.checked })}
					/>
					<span class="qrx_set_row_text">
						<span class="qrx_set_row_title">{i18n.t.grp_vertical}</span>
						<span class="qrx_rec_task_note">
							{current.vertical ? i18n.t.grp_vertical_on : i18n.t.grp_vertical_off}
						</span>
					</span>
				</label>

				<div class="qrx_iso_row">
					<span class="qrx_iso_path">{i18n.t.grp_pin_note}</span>

					<button
						class="qrx_rec_log"
						data-tooltip={i18n.t.grp_start_note}
						data-tooltip-position="top"
						onclick={() => pinStart(current)}
					>
						{i18n.t.grp_start}
					</button>

					<button class="qrx_rec_log" onclick={() => shortcut(current)}>
						{i18n.t.grp_pin}
					</button>
				</div>
			</div>

			<div class="qrx_iso_group">
				<span class="qrx_field_label">{i18n.t.grp_items}</span>

				{#if current.items.length === 0}
					<span class="qrx_rec_task_note">{i18n.t.grp_items_none}</span>
				{/if}

				{#each rows as item, index (item.id)}
					{@const inner = item.child ? groups.list.find((each) => each.id === item.child) : null}

					<article
						class="qrx_grp_item"
						class:qrx_grp_item_new={fresh.has(item.id)}
						class:qrx_grp_item_held={held === index && preview !== null}
						data-row={index}
						animate:flip={{ duration: 220 }}
					>
						<!-- Ручка: за неё строку тащат на новое место. Тащить за всю
						     строку нельзя — тогда не выделить название в поле -->
						<span
							class="qrx_grp_handle"
							role="button"
							tabindex="-1"
							aria-label={i18n.t.grp_reorder}
							data-tooltip={i18n.t.grp_reorder}
							data-tooltip-position="right"
							onpointerdown={(event) => grab(event, current, index)}
						></span>

						<!-- Значок записи можно подменить: нажатие по нему выбирает
						     файл, правое нажатие возвращает исходный. Отдельной
						     кнопки в ряду нет — он и без того плотный -->
						<button
							class="qrx_grp_ico"
							class:qrx_grp_ico_own={!!item.icon}
							aria-label={i18n.t.grp_item_icon}
							data-tooltip={item.icon || i18n.t.grp_item_icon}
							data-tooltip-position="top"
							onclick={() => pickItemIcon(current, item.id)}
							oncontextmenu={(event) => {
								if (!item.icon) return;
								event.preventDefault();
								change(current, item.id, { icon: '' });
							}}
						>
							<AppIcon
								path={item.icon || (inner ? icon(inner) : item.path)}
								fallback={item.name}
								size={40}
							/>
						</button>

						<div class="qrx_found_main">
							<input
								class="qrx_field_input qrx_grp_name"
								type="text"
								value={item.name}
								onchange={(event) =>
									change(current, item.id, { name: event.currentTarget.value })}
							/>

							{#if item.child}
								<span class="qrx_rec_task_note">
									{inner
										? `${i18n.t.grp_nested}: ${inner.items.length} ${i18n.t.grp_count}`
										: i18n.t.grp_nested_gone}
								</span>
							{:else}
								<span class="qrx_iso_path">{item.path}</span>
							{/if}
						</div>

						<!-- У вложенного набора нет ни ключей запуска, ни прав: он не
						     программа, в него заходят -->
						{#if !item.child}
							<input
								class="qrx_field_input qrx_iso_key"
								type="text"
								placeholder={i18n.t.iso_args}
								value={item.args}
								spellcheck="false"
								onchange={(event) =>
									change(current, item.id, { args: event.currentTarget.value })}
							/>

							<input
								class="qrx_field_input qrx_iso_key"
								type="text"
								placeholder={i18n.t.grp_tip}
								value={item.tip}
								onchange={(event) => change(current, item.id, { tip: event.currentTarget.value })}
							/>

							<button
								class="qrx_found_chip"
								class:qrx_found_chip_active={item.admin}
								data-tooltip={i18n.t.grp_admin_note}
								data-tooltip-position="top"
								onclick={() => change(current, item.id, { admin: !item.admin })}
							>
								{i18n.t.grp_admin}
							</button>

							<button
								class="qrx_found_chip"
								class:qrx_found_chip_active={item.isolated}
								data-tooltip={i18n.t.grp_iso_note}
								data-tooltip-position="top"
								onclick={() => change(current, item.id, { isolated: !item.isolated })}
							>
								{i18n.t.iso_short}
							</button>
						{:else}
							<span class="qrx_found_tag qrx_found_tag_portable">{i18n.t.grp_nest}</span>
						{/if}

						<!-- Стрелки остаются рядом с ручкой: мышью быстрее, стрелками
						     точнее, и они же доступны с клавиатуры -->
						<div class="qrx_grp_move">
							<button
								class="qrx_grp_step"
								aria-label={i18n.t.grp_up}
								data-tooltip={i18n.t.grp_up}
								data-tooltip-position="top"
								disabled={index === 0}
								onclick={() => move(current, index, -1)}
							>
								<span class="qrx_grp_step_ico qrx_grp_step_up" aria-hidden="true"></span>
							</button>

							<button
								class="qrx_grp_step"
								aria-label={i18n.t.grp_down}
								data-tooltip={i18n.t.grp_down}
								data-tooltip-position="top"
								disabled={index === current.items.length - 1}
								onclick={() => move(current, index, 1)}
							>
								<span class="qrx_grp_step_ico qrx_grp_step_down" aria-hidden="true"></span>
							</button>
						</div>

						{#if item.child}
							<button class="qrx_rec_log" onclick={() => go(item.child)}>
								{i18n.t.grp_go}
							</button>
						{:else}
							<button class="qrx_rec_log" onclick={() => launchOne(current, item)}>
								{i18n.t.grp_open}
							</button>
						{/if}

						<button class="qrx_rec_log qrx_lib_drop" onclick={() => drop(current, item.id)}>
							{i18n.t.iso_drop}
						</button>

						<!-- Права видны только у того, что запускается в изоляции:
						     обычному запуску они ни к чему -->
						{#if item.isolated}
							<div class="qrx_grp_rights">
								<IsolationSettings
									compact
									target={item.id}
									name={item.name}
									suggested={item.path}
								/>
							</div>
						{/if}
					</article>
				{/each}

				<div class="qrx_iso_row">
					<button class="qrx_sys_link" onclick={() => addFile(current)}>
						{i18n.t.grp_add_app}
					</button>
					<button class="qrx_rec_log" onclick={() => addFolder(current)}>
						{i18n.t.grp_add_folder}
					</button>
					<button class="qrx_rec_log" onclick={() => (picking = !picking)}>
						{i18n.t.cat_from_found}
					</button>

					{#if nestable.length > 0}
						<button
							class="qrx_rec_log"
							data-tooltip={i18n.t.grp_nest_note}
							data-tooltip-position="top"
							onclick={() => (nesting = !nesting)}
						>
							{i18n.t.grp_nest}
						</button>
					{/if}
				</div>

				{#if nesting}
					<!-- Вложение: набор внутри набора, как папка внутри папки -->
					<div class="qrx_form_pick">
						{#each nestable as group (group.id)}
							<button class="qrx_form_found" onclick={() => nest(current, group)}>
								{group.name} · {group.items.length} {i18n.t.grp_count}
							</button>
						{/each}
					</div>
				{/if}

				{#if picking}
					<!-- Подстановка с машины: путь к программе уже известен осмотру -->
					<div class="qrx_form_pick">
						<input
							class="qrx_found_search"
							type="text"
							placeholder={i18n.t.found_search}
							bind:value={query}
							spellcheck="false"
							oninput={() => inventory.ensure()}
						/>

						{#each found as finding (finding.key)}
							<button
								class="qrx_form_found"
								onclick={() => {
									patch(current, {
										items: [
											...current.items,
											{ ...shortcutFor(finding.exe), name: finding.name }
										]
									});
									picking = false;
									query = '';
								}}
							>
								{finding.name}
							</button>
						{/each}
					</div>
				{/if}
			</div>
		</div>
	{/if}
	</div>
	{/key}
	</div>
</div>
